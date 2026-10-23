//! The main loop: keyboard events in, controller state out.

use crate::config::{Binding, Config};
use crate::gamepad::{DolphinFlavor, Gamepad};
use crate::input::{self, DeviceId, Forwarder, Msg, ReaderOptions};
use crate::keys::Mods;
use crate::logic::{B0xx, Button};
use anyhow::Result;
use evdev::KeyCode;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;

pub struct RunOptions {
    pub config_path: PathBuf,
    pub devices: Vec<PathBuf>,
    pub flavor: DolphinFlavor,
    pub verbose: bool,
}

struct App {
    config: Config,
    bindings: Vec<Binding>,
    /// Parallel to `bindings`: whether that hotkey is currently held.
    active: Vec<bool>,
    b0xx: B0xx,
    gamepad: Gamepad,
    forwarder: Forwarder,
    suspended: bool,
    verbose: bool,
    /// How many devices are holding each key.
    held: HashMap<KeyCode, u32>,
    held_by_device: HashMap<DeviceId, HashSet<KeyCode>>,
    /// Keys from grabbed devices whose press was swallowed, so their release must be too.
    swallowed: HashSet<(DeviceId, KeyCode)>,
}

pub fn run(opts: RunOptions) -> Result<()> {
    let config = Config::load_or_create(&opts.config_path)?;
    let bindings = config.bindings()?;
    eprintln!("Using hotkeys from {}", config.path.display());

    let gamepad = Gamepad::new(opts.flavor)?;
    eprintln!(
        "Created virtual controller \"{}\" (Dolphin device: evdev/0/{})",
        crate::gamepad::DEVICE_NAME,
        crate::gamepad::DEVICE_NAME
    );

    let (tx, rx) = mpsc::channel();
    input::start(
        ReaderOptions {
            grab_keys: grab_keys(&bindings),
            only: opts.devices,
            interesting_keys: bindings.iter().flat_map(|b| b.hotkey.keys.iter().copied()).collect(),
            verbose: true,
        },
        tx,
    );
    eprintln!("Running. Ctrl+Alt+S or Pause toggles hotkeys, Ctrl+Alt+R reloads hotkeys.ini, Ctrl+C quits.");
    notify("Script Started");

    let mut app = App {
        active: vec![false; bindings.len()],
        config,
        bindings,
        b0xx: B0xx::new(),
        gamepad,
        forwarder: Forwarder::new(),
        suspended: false,
        verbose: opts.verbose,
        held: HashMap::new(),
        held_by_device: HashMap::new(),
        swallowed: HashSet::new(),
    };

    for msg in rx {
        match msg {
            Msg::Key { dev, key, value, grabbed } => app.on_key(dev, key, value, grabbed)?,
            Msg::Gone { dev } => app.on_device_gone(dev)?,
        }
        let out = app.b0xx.output();
        app.gamepad.update(&out)?;
    }
    Ok(())
}

/// Keys that some hotkey wants to keep from other applications.
fn grab_keys(bindings: &[Binding]) -> HashSet<KeyCode> {
    bindings.iter().filter(|b| !b.hotkey.passthrough).flat_map(|b| b.hotkey.keys.iter().copied()).collect()
}

impl App {
    fn mods(&self) -> Mods {
        Mods::from_held(|k| self.held.contains_key(&k))
    }

    fn on_key(&mut self, dev: DeviceId, key: KeyCode, value: i32, grabbed: bool) -> Result<()> {
        match value {
            // Autorepeat: never affects the controller, but pass it on if the press was.
            2 => {
                if grabbed && !self.swallowed.contains(&(dev, key)) {
                    self.forwarder.send(key, value)?;
                }
            }
            1 => {
                if !self.held_by_device.entry(dev).or_default().insert(key) {
                    return Ok(());
                }
                let count = self.held.entry(key).or_insert(0);
                *count += 1;
                let swallow = if *count == 1 { self.on_press(key)? } else { false };
                if grabbed {
                    if swallow {
                        self.swallowed.insert((dev, key));
                    } else {
                        self.forwarder.send(key, value)?;
                    }
                }
            }
            0 => {
                if !self.held_by_device.entry(dev).or_default().remove(&key) {
                    return Ok(());
                }
                self.key_up(key);
                if grabbed && !self.swallowed.remove(&(dev, key)) {
                    self.forwarder.send(key, value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handles the first press of a key. Returns whether to keep it from other applications.
    fn on_press(&mut self, key: KeyCode) -> Result<bool> {
        let mods = self.mods();
        if (mods.ctrl && mods.alt && key == KeyCode::KEY_S) || key == KeyCode::KEY_PAUSE {
            self.toggle_suspend();
            return Ok(true);
        }
        if mods.ctrl && mods.alt && key == KeyCode::KEY_R {
            self.reload();
            return Ok(true);
        }
        if self.suspended {
            return Ok(false);
        }
        // Hotkeys fire whatever other modifiers are held (so e.g. a Shift-bound ModX
        // doesn't block other buttons), but if both `p` and `^p` are bound, Ctrl+P
        // only fires the more specific one.
        let matches: Vec<usize> = (0..self.bindings.len())
            .filter(|&i| {
                let hotkey = &self.bindings[i].hotkey;
                !self.active[i] && hotkey.keys.contains(&key) && hotkey.mods.satisfied_by(mods)
            })
            .collect();
        let most_specific = matches.iter().map(|&i| self.bindings[i].hotkey.mods.count()).max().unwrap_or(0);
        let mut swallow = false;
        for i in matches {
            let hotkey = &self.bindings[i].hotkey;
            if hotkey.mods.count() < most_specific {
                continue;
            }
            swallow |= !hotkey.passthrough;
            self.active[i] = true;
            let button = self.bindings[i].button;
            self.b0xx.press(button);
            if self.verbose {
                eprintln!("+ {}", button.label());
            }
            if button == Button::Debug {
                notify(&self.b0xx.debug_report());
            }
        }
        Ok(swallow)
    }

    fn key_up(&mut self, key: KeyCode) {
        let Some(count) = self.held.get_mut(&key) else { return };
        *count -= 1;
        if *count > 0 {
            return;
        }
        self.held.remove(&key);
        for i in 0..self.bindings.len() {
            if self.active[i] && self.bindings[i].hotkey.keys.contains(&key) {
                self.active[i] = false;
                self.b0xx.release(self.bindings[i].button);
                if self.verbose {
                    eprintln!("- {}", self.bindings[i].button.label());
                }
            }
        }
    }

    fn on_device_gone(&mut self, dev: DeviceId) -> Result<()> {
        for key in self.held_by_device.remove(&dev).unwrap_or_default() {
            self.key_up(key);
            // If the press was forwarded, the passthrough keyboard is still holding it.
            if !self.swallowed.remove(&(dev, key)) {
                let _ = self.forwarder.send(key, 0);
            }
        }
        Ok(())
    }

    fn release_all(&mut self) {
        for i in 0..self.bindings.len() {
            if self.active[i] {
                self.active[i] = false;
                self.b0xx.release(self.bindings[i].button);
            }
        }
    }

    fn toggle_suspend(&mut self) {
        self.suspended = !self.suspended;
        if self.suspended {
            // Leave the controller in a neutral state rather than stuck mid-input.
            self.release_all();
            notify("Hotkeys Disabled");
        } else {
            notify("Hotkeys Enabled");
        }
    }

    fn reload(&mut self) {
        let loaded = Config::load_or_create(&self.config.path).and_then(|c| c.bindings().map(|b| (c, b)));
        match loaded {
            Ok((config, bindings)) => {
                if grab_keys(&bindings) != grab_keys(&self.bindings) {
                    eprintln!("Note: \"prevent default\" changes only take effect after restarting.");
                }
                self.release_all();
                self.active = vec![false; bindings.len()];
                self.config = config;
                self.bindings = bindings;
                notify("Hotkeys Reloaded");
            }
            Err(e) => {
                eprintln!("Reload failed, keeping previous hotkeys: {e:#}");
                notify("Reload failed, see terminal");
            }
        }
    }
}

/// Prints a message and shows a desktop notification if possible.
pub fn notify(msg: &str) {
    eprintln!("[B0XX] {msg}");
    let _ = std::process::Command::new("notify-send")
        .args(["--app-name=b0xx-linux", "--expire-time=3000", "B0XX", msg])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|mut child| std::thread::spawn(move || child.wait()));
}
