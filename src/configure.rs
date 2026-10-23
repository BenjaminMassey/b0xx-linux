//! Interactive terminal replacement for the AHK version's "Edit Controls" GUI.

use crate::config::Config;
use crate::input::{self, Msg, ReaderOptions};
use crate::keys::{Hotkey, Mods, is_modifier};
use crate::logic::Button;
use anyhow::{Result, bail};
use evdev::KeyCode;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

/// Keys whose normal function would probably get in the way while playing.
fn has_system_function(key: KeyCode) -> bool {
    is_modifier(key)
        || matches!(
            key,
            KeyCode::KEY_TAB | KeyCode::KEY_ENTER | KeyCode::KEY_CAPSLOCK | KeyCode::KEY_COMPOSE | KeyCode::KEY_BACKSPACE
        )
}

struct Keys {
    rx: Receiver<Msg>,
    held: HashSet<KeyCode>,
}

enum Captured {
    Keep,
    Clear,
    Hotkey(Hotkey),
}

impl Keys {
    /// Next press (value 1) or release (value 0), tracking held keys.
    fn next(&mut self) -> Result<(KeyCode, bool)> {
        loop {
            let Ok(msg) = self.rx.recv() else { bail!("keyboard input stopped") };
            let Msg::Key { key, value, .. } = msg else { continue };
            match value {
                1 if self.held.insert(key) => return Ok((key, true)),
                0 if self.held.remove(&key) => return Ok((key, false)),
                _ => {}
            }
        }
    }

    fn mods(&self) -> Mods {
        Mods::from_held(|k| self.held.contains(&k))
    }

    fn wait_all_released(&mut self) -> Result<()> {
        while !self.held.is_empty() {
            self.next()?;
        }
        Ok(())
    }

    /// Waits for a key, or a modifier pressed and released on its own.
    fn capture(&mut self) -> Result<Captured> {
        let mut lone_modifier = None;
        loop {
            let (key, down) = self.next()?;
            let mods = self.mods();
            if !down {
                if lone_modifier == Some(key) {
                    return Ok(Captured::Hotkey(Hotkey { keys: vec![key], mods: Mods::default(), passthrough: true }));
                }
                continue;
            }
            if is_modifier(key) {
                lone_modifier = Some(key);
                continue;
            }
            match key {
                KeyCode::KEY_C if mods.ctrl => bail!("aborted, nothing saved"),
                KeyCode::KEY_ESC if mods.is_empty() => return Ok(Captured::Keep),
                KeyCode::KEY_BACKSPACE if mods.is_empty() => return Ok(Captured::Clear),
                _ => return Ok(Captured::Hotkey(Hotkey { keys: vec![key], mods, passthrough: true })),
            }
        }
    }

    fn yes_no(&mut self) -> Result<bool> {
        loop {
            match self.next()? {
                (KeyCode::KEY_Y, true) => return Ok(true),
                (KeyCode::KEY_N | KeyCode::KEY_ENTER, true) => return Ok(false),
                (KeyCode::KEY_C, true) if self.mods().ctrl => bail!("aborted, nothing saved"),
                _ => {}
            }
        }
    }
}

pub fn run(config_path: &Path, devices: Vec<PathBuf>) -> Result<()> {
    let mut config = Config::load_or_create(config_path)?;

    println!("Configuring {}", config.path.display());
    println!("For each button, press the key to bind (hold Shift/Ctrl/Alt for a combination).");
    println!("To bind a modifier on its own, press and release it. Esc keeps the current key,");
    println!("Backspace unbinds, Ctrl+C aborts without saving.\n");
    println!("Keyboards are grabbed while configuring, so typing won't reach other windows.");

    let (tx, rx) = mpsc::channel();
    input::start(
        ReaderOptions {
            grab_keys: HashSet::from([KeyCode::KEY_A]),
            only: devices,
            interesting_keys: HashSet::new(),
            verbose: false,
        },
        tx,
    );
    let mut keys = Keys { rx, held: HashSet::new() };

    for button in Button::ALL {
        let i = button.slot() - 1;
        let current = &config.raw[i];
        let shown = if current.is_empty() { "(none)" } else { current.as_str() };
        print!("{:>2}. {:<14} [{shown}]: ", i + 1, button.label());
        std::io::stdout().flush()?;

        let new = loop {
            let captured = keys.capture()?;
            keys.wait_all_released()?;
            let mut hotkey = match captured {
                Captured::Keep => break current.clone(),
                Captured::Clear => break String::new(),
                Captured::Hotkey(h) => h,
            };
            let taken = config.raw.iter().enumerate().find(|&(j, raw)| {
                j != i && Hotkey::parse(raw).is_ok_and(|h| h.keys == hotkey.keys && h.mods == hotkey.mods)
            });
            if let Some((j, _)) = taken {
                print!("{} is already used by {}, try another: ", hotkey.format(), Button::ALL[j].label());
                std::io::stdout().flush()?;
                continue;
            }
            // Keep the existing prevent-default setting unless it's worth asking about.
            hotkey.passthrough = Hotkey::parse(current).map_or(true, |h| h.passthrough);
            if hotkey.keys.iter().any(|&k| has_system_function(k)) || !hotkey.mods.is_empty() {
                print!("{}  prevent default behaviour? [y/N] ", hotkey.format().trim_start_matches('~'));
                std::io::stdout().flush()?;
                hotkey.passthrough = !keys.yes_no()?;
                keys.wait_all_released()?;
            }
            break hotkey.format();
        };
        println!("{}", if new.is_empty() { "(none)" } else { new.as_str() });
        config.raw[i] = new;
    }

    config.save()?;
    println!("\nSaved {}.", config.path.display());
    if config.bindings()?.iter().any(|b| !b.hotkey.passthrough) {
        println!("Some keys prevent default behaviour, so keyboards with those keys will be grabbed while running.");
    }
    println!("If b0xx-linux is already running, press Ctrl+Alt+R to reload.");
    Ok(())
}
