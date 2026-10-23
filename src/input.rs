//! Reading physical keyboards through evdev.
//!
//! Each keyboard gets a reader thread that forwards key events over a channel.
//! A scanner thread picks up keyboards that are plugged in later.
//!
//! Keyboards are only *grabbed* (made exclusive to this program) when some
//! hotkey asks to "prevent default behaviour"; in that case the keys that
//! should still reach other applications are re-emitted through a virtual
//! keyboard (see `Forwarder`).

use crate::gamepad;
use anyhow::{Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, EventSummary, EventType, InputEvent, InputId, KeyCode};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const FORWARDER_NAME: &str = "b0xx-linux Passthrough Keyboard";

pub type DeviceId = usize;

pub enum Msg {
    Key { dev: DeviceId, key: KeyCode, value: i32, grabbed: bool },
    Gone { dev: DeviceId },
}

pub struct ReaderOptions {
    /// Grab devices that can produce any of these keys.
    pub grab_keys: HashSet<KeyCode>,
    /// Only read these device paths (all keyboards if empty).
    pub only: Vec<PathBuf>,
    /// Keys that make a device worth reading, besides letters.
    pub interesting_keys: HashSet<KeyCode>,
    /// Print devices as they're opened.
    pub verbose: bool,
}

/// Starts reading all current and future keyboards. Returns immediately.
pub fn start(opts: ReaderOptions, tx: Sender<Msg>) {
    let opts = Arc::new(opts);
    let open = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
    let mut next_id = 0;
    let mut scan = move || {
        let mut found = 0;
        for (path, device) in evdev::enumerate() {
            if open.lock().unwrap().contains(&path) || !should_read(&path, &device, &opts) {
                continue;
            }
            found += 1;
            let grab = should_grab(&device, &opts.grab_keys);
            if opts.verbose {
                let note = if grab { " (grabbed)" } else { "" };
                eprintln!("Reading {} \"{}\"{note}", path.display(), device.name().unwrap_or("?"));
            }
            open.lock().unwrap().insert(path.clone());
            let (tx, open, dev) = (tx.clone(), open.clone(), next_id);
            next_id += 1;
            thread::spawn(move || {
                if let Err(e) = read_device(device, dev, grab, &tx) {
                    eprintln!("Stopped reading {}: {e:#}", path.display());
                }
                open.lock().unwrap().remove(&path);
                let _ = tx.send(Msg::Gone { dev });
            });
        }
        found
    };
    if scan() == 0 {
        eprintln!("No keyboards found yet. Are you in the 'input' group? (see README)");
    }
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(2));
            scan();
        }
    });
}

fn is_own_device(device: &Device) -> bool {
    matches!(device.name(), Some(gamepad::DEVICE_NAME | FORWARDER_NAME))
}

fn should_read(path: &Path, device: &Device, opts: &ReaderOptions) -> bool {
    if is_own_device(device) {
        return false;
    }
    if !opts.only.is_empty() {
        return opts.only.iter().any(|p| same_file(p, path));
    }
    let Some(keys) = device.supported_keys() else { return false };
    keys.contains(KeyCode::KEY_A) || opts.interesting_keys.iter().any(|&k| keys.contains(k))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn should_grab(device: &Device, grab_keys: &HashSet<KeyCode>) -> bool {
    let Some(keys) = device.supported_keys() else { return false };
    if !grab_keys.iter().any(|&k| keys.contains(k)) {
        return false;
    }
    // Never grab something that's also a mouse: we only forward keys.
    if device.supported_relative_axes().is_some_and(|axes| axes.iter().next().is_some()) {
        eprintln!(
            "Not grabbing \"{}\": it is also a pointing device. Prevent-default won't apply to it.",
            device.name().unwrap_or("?")
        );
        return false;
    }
    true
}

fn read_device(mut device: Device, dev: DeviceId, grab: bool, tx: &Sender<Msg>) -> Result<()> {
    if grab {
        // Grabbing while a key is down would leave that key stuck for everyone
        // else (they'd never see the release), so wait for the keyboard to be idle.
        let mut warned = false;
        while device.get_key_state()?.iter().next().is_some() {
            if !warned {
                eprintln!("Waiting for all keys to be released on \"{}\"...", device.name().unwrap_or("?"));
                warned = true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        device.grab().context("grabbing device")?;
    }
    loop {
        for event in device.fetch_events()? {
            if let EventSummary::Key(_, key, value) = event.destructure()
                && tx.send(Msg::Key { dev, key, value, grabbed: grab }).is_err()
            {
                return Ok(());
            }
        }
    }
}

/// Virtual keyboard that re-emits keys from grabbed devices that should pass through.
pub struct Forwarder {
    device: Option<VirtualDevice>,
}

impl Forwarder {
    pub fn new() -> Self {
        Forwarder { device: None }
    }

    pub fn send(&mut self, key: KeyCode, value: i32) -> Result<()> {
        if self.device.is_none() {
            let mut keys = AttributeSet::<KeyCode>::new();
            // Keyboard keys only (skip the mouse/joystick BTN_ ranges).
            for code in (1..0x100).chain(0x160..0x2c0) {
                keys.insert(KeyCode::new(code));
            }
            let device = VirtualDevice::builder()?
                .name(FORWARDER_NAME)
                .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x1209, 0xb0c6, 1))
                .with_keys(&keys)?
                .build()
                .context("creating passthrough keyboard")?;
            self.device = Some(device);
        }
        let event = InputEvent::new(EventType::KEY.0, key.code(), value);
        self.device.as_mut().unwrap().emit(&[event])?;
        Ok(())
    }
}

/// Prints every input device and whether it would be read.
pub fn list_devices() {
    let mut devices: Vec<_> = evdev::enumerate().collect();
    devices.sort_by(|a, b| a.0.cmp(&b.0));
    if devices.is_empty() {
        println!("No readable input devices. Are you in the 'input' group? (see README)");
    }
    for (path, device) in devices {
        let is_keyboard = device.supported_keys().is_some_and(|k| k.contains(KeyCode::KEY_A));
        let kind = if is_own_device(&device) {
            "this program"
        } else if is_keyboard {
            "keyboard"
        } else {
            "-"
        };
        println!("{:<22} {:<13} {}", path.display(), kind, device.name().unwrap_or("?"));
    }
    println!("\nPaths in /dev/input/by-id/ are stable across reboots, if you want to use --device.");
}
