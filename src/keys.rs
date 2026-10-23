//! Parsing and formatting of AutoHotkey-style hotkey strings, e.g. `~]`, `+^``, `LShift`.
//!
//! Key names are mapped to Linux evdev key codes by *physical position* on a
//! US keyboard (so `]` means "the key right of P"), which is what AutoHotkey
//! does too on a US layout. Raw evdev names such as `KEY_LEFTSHIFT` are also
//! accepted, which covers anything AutoHotkey has no name for.

use anyhow::{Result, anyhow, bail};
use evdev::KeyCode;
use std::str::FromStr;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Mods {
    /// True if every modifier required by `self` is held in `held`.
    pub fn satisfied_by(self, held: Mods) -> bool {
        (!self.shift || held.shift) && (!self.ctrl || held.ctrl) && (!self.alt || held.alt) && (!self.meta || held.meta)
    }

    pub fn is_empty(self) -> bool {
        self == Mods::default()
    }

    pub fn count(self) -> usize {
        [self.shift, self.ctrl, self.alt, self.meta].iter().filter(|&&m| m).count()
    }

    pub fn from_held(is_down: impl Fn(KeyCode) -> bool) -> Mods {
        Mods {
            shift: is_down(KeyCode::KEY_LEFTSHIFT) || is_down(KeyCode::KEY_RIGHTSHIFT),
            ctrl: is_down(KeyCode::KEY_LEFTCTRL) || is_down(KeyCode::KEY_RIGHTCTRL),
            alt: is_down(KeyCode::KEY_LEFTALT) || is_down(KeyCode::KEY_RIGHTALT),
            meta: is_down(KeyCode::KEY_LEFTMETA) || is_down(KeyCode::KEY_RIGHTMETA),
        }
    }
}

pub fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
    )
}

/// A parsed hotkey such as `~+^``.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hotkey {
    /// Any of these keys triggers the hotkey (e.g. `Shift` is LShift or RShift).
    pub keys: Vec<KeyCode>,
    pub mods: Mods,
    /// `~` prefix: let the key through to other applications as well.
    pub passthrough: bool,
}

impl Hotkey {
    pub fn parse(s: &str) -> Result<Hotkey> {
        let s = s.trim();
        let mut mods = Mods::default();
        let mut passthrough = false;
        let mut rest = s;
        // Consume prefix symbols, but never the final character (it may be the key itself).
        while rest.len() > 1 {
            let c = rest.chars().next().unwrap();
            match c {
                '~' => passthrough = true,
                '+' => mods.shift = true,
                '^' => mods.ctrl = true,
                '!' => mods.alt = true,
                '#' => mods.meta = true,
                // wildcard / hook / left-right modifier qualifiers: no-ops here
                '*' | '$' | '<' | '>' => {}
                _ => break,
            }
            rest = &rest[c.len_utf8()..];
        }
        if rest.contains(" & ") {
            bail!("custom combinations (\"a & b\") are not supported: {s:?}");
        }
        let keys = key_codes(rest).ok_or_else(|| anyhow!("{rest:?} is not a valid key name"))?;
        Ok(Hotkey { keys, mods, passthrough })
    }

    pub fn format(&self) -> String {
        let mut s = String::new();
        if self.passthrough {
            s.push('~');
        }
        if self.mods.meta {
            s.push('#');
        }
        if self.mods.shift {
            s.push('+');
        }
        if self.mods.ctrl {
            s.push('^');
        }
        if self.mods.alt {
            s.push('!');
        }
        match self.keys.as_slice() {
            [KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT] => s.push_str("Shift"),
            [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL] => s.push_str("Ctrl"),
            [KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT] => s.push_str("Alt"),
            [key] => s.push_str(&key_name(*key)),
            keys => s.push_str(&keys.iter().map(|k| key_name(*k)).collect::<Vec<_>>().join("|")),
        }
        s
    }
}

/// AutoHotkey key names (lowercase) and their evdev key codes.
const NAMES: &[(&str, KeyCode)] = &[
    ("`", KeyCode::KEY_GRAVE),
    ("-", KeyCode::KEY_MINUS),
    ("=", KeyCode::KEY_EQUAL),
    ("[", KeyCode::KEY_LEFTBRACE),
    ("]", KeyCode::KEY_RIGHTBRACE),
    ("\\", KeyCode::KEY_BACKSLASH),
    (";", KeyCode::KEY_SEMICOLON),
    ("'", KeyCode::KEY_APOSTROPHE),
    (",", KeyCode::KEY_COMMA),
    (".", KeyCode::KEY_DOT),
    ("/", KeyCode::KEY_SLASH),
    ("space", KeyCode::KEY_SPACE),
    ("tab", KeyCode::KEY_TAB),
    ("enter", KeyCode::KEY_ENTER),
    ("return", KeyCode::KEY_ENTER),
    ("escape", KeyCode::KEY_ESC),
    ("esc", KeyCode::KEY_ESC),
    ("backspace", KeyCode::KEY_BACKSPACE),
    ("bs", KeyCode::KEY_BACKSPACE),
    ("delete", KeyCode::KEY_DELETE),
    ("del", KeyCode::KEY_DELETE),
    ("insert", KeyCode::KEY_INSERT),
    ("ins", KeyCode::KEY_INSERT),
    ("home", KeyCode::KEY_HOME),
    ("end", KeyCode::KEY_END),
    ("pgup", KeyCode::KEY_PAGEUP),
    ("pgdn", KeyCode::KEY_PAGEDOWN),
    ("up", KeyCode::KEY_UP),
    ("down", KeyCode::KEY_DOWN),
    ("left", KeyCode::KEY_LEFT),
    ("right", KeyCode::KEY_RIGHT),
    ("capslock", KeyCode::KEY_CAPSLOCK),
    ("scrolllock", KeyCode::KEY_SCROLLLOCK),
    ("numlock", KeyCode::KEY_NUMLOCK),
    ("printscreen", KeyCode::KEY_SYSRQ),
    ("pause", KeyCode::KEY_PAUSE),
    ("appskey", KeyCode::KEY_COMPOSE),
    ("lshift", KeyCode::KEY_LEFTSHIFT),
    ("rshift", KeyCode::KEY_RIGHTSHIFT),
    ("lctrl", KeyCode::KEY_LEFTCTRL),
    ("lcontrol", KeyCode::KEY_LEFTCTRL),
    ("rctrl", KeyCode::KEY_RIGHTCTRL),
    ("rcontrol", KeyCode::KEY_RIGHTCTRL),
    ("lalt", KeyCode::KEY_LEFTALT),
    ("ralt", KeyCode::KEY_RIGHTALT),
    ("lwin", KeyCode::KEY_LEFTMETA),
    ("rwin", KeyCode::KEY_RIGHTMETA),
    ("numpad0", KeyCode::KEY_KP0),
    ("numpad1", KeyCode::KEY_KP1),
    ("numpad2", KeyCode::KEY_KP2),
    ("numpad3", KeyCode::KEY_KP3),
    ("numpad4", KeyCode::KEY_KP4),
    ("numpad5", KeyCode::KEY_KP5),
    ("numpad6", KeyCode::KEY_KP6),
    ("numpad7", KeyCode::KEY_KP7),
    ("numpad8", KeyCode::KEY_KP8),
    ("numpad9", KeyCode::KEY_KP9),
    ("numpaddot", KeyCode::KEY_KPDOT),
    ("numpaddiv", KeyCode::KEY_KPSLASH),
    ("numpadmult", KeyCode::KEY_KPASTERISK),
    ("numpadadd", KeyCode::KEY_KPPLUS),
    ("numpadsub", KeyCode::KEY_KPMINUS),
    ("numpadenter", KeyCode::KEY_KPENTER),
    // NumLock-off names for the same physical keys
    ("numpadins", KeyCode::KEY_KP0),
    ("numpadend", KeyCode::KEY_KP1),
    ("numpaddown", KeyCode::KEY_KP2),
    ("numpadpgdn", KeyCode::KEY_KP3),
    ("numpadleft", KeyCode::KEY_KP4),
    ("numpadclear", KeyCode::KEY_KP5),
    ("numpadright", KeyCode::KEY_KP6),
    ("numpadhome", KeyCode::KEY_KP7),
    ("numpadup", KeyCode::KEY_KP8),
    ("numpadpgup", KeyCode::KEY_KP9),
    ("numpaddel", KeyCode::KEY_KPDOT),
];

/// Extended (0xE0-prefixed) Windows scan codes, written by AHK as sc1XX.
const EXTENDED_SCANCODES: &[(u16, KeyCode)] = &[
    (0x11C, KeyCode::KEY_KPENTER),
    (0x11D, KeyCode::KEY_RIGHTCTRL),
    (0x135, KeyCode::KEY_KPSLASH),
    (0x137, KeyCode::KEY_SYSRQ),
    (0x138, KeyCode::KEY_RIGHTALT),
    (0x145, KeyCode::KEY_NUMLOCK),
    (0x147, KeyCode::KEY_HOME),
    (0x148, KeyCode::KEY_UP),
    (0x149, KeyCode::KEY_PAGEUP),
    (0x14B, KeyCode::KEY_LEFT),
    (0x14D, KeyCode::KEY_RIGHT),
    (0x14F, KeyCode::KEY_END),
    (0x150, KeyCode::KEY_DOWN),
    (0x151, KeyCode::KEY_PAGEDOWN),
    (0x152, KeyCode::KEY_INSERT),
    (0x153, KeyCode::KEY_DELETE),
    (0x15B, KeyCode::KEY_LEFTMETA),
    (0x15C, KeyCode::KEY_RIGHTMETA),
    (0x15D, KeyCode::KEY_COMPOSE),
];

const LETTERS: [KeyCode; 26] = [
    KeyCode::KEY_A,
    KeyCode::KEY_B,
    KeyCode::KEY_C,
    KeyCode::KEY_D,
    KeyCode::KEY_E,
    KeyCode::KEY_F,
    KeyCode::KEY_G,
    KeyCode::KEY_H,
    KeyCode::KEY_I,
    KeyCode::KEY_J,
    KeyCode::KEY_K,
    KeyCode::KEY_L,
    KeyCode::KEY_M,
    KeyCode::KEY_N,
    KeyCode::KEY_O,
    KeyCode::KEY_P,
    KeyCode::KEY_Q,
    KeyCode::KEY_R,
    KeyCode::KEY_S,
    KeyCode::KEY_T,
    KeyCode::KEY_U,
    KeyCode::KEY_V,
    KeyCode::KEY_W,
    KeyCode::KEY_X,
    KeyCode::KEY_Y,
    KeyCode::KEY_Z,
];

const DIGITS: [KeyCode; 10] = [
    KeyCode::KEY_0,
    KeyCode::KEY_1,
    KeyCode::KEY_2,
    KeyCode::KEY_3,
    KeyCode::KEY_4,
    KeyCode::KEY_5,
    KeyCode::KEY_6,
    KeyCode::KEY_7,
    KeyCode::KEY_8,
    KeyCode::KEY_9,
];

/// evdev codes of F1..F24.
fn function_key(n: u16) -> Option<KeyCode> {
    let code = match n {
        1..=10 => KeyCode::KEY_F1.code() + n - 1,
        11 => KeyCode::KEY_F11.code(),
        12 => KeyCode::KEY_F12.code(),
        13..=24 => KeyCode::KEY_F13.code() + n - 13,
        _ => return None,
    };
    Some(KeyCode::new(code))
}

/// Resolves a single key name (no prefixes) to one or more evdev codes.
pub fn key_codes(name: &str) -> Option<Vec<KeyCode>> {
    if name.starts_with("KEY_") || name.starts_with("BTN_") {
        return KeyCode::from_str(name).ok().map(|k| vec![k]);
    }
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "shift" => return Some(vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT]),
        "ctrl" | "control" => return Some(vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL]),
        "alt" => return Some(vec![KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT]),
        _ => {}
    }
    if let [c] = lower.as_bytes() {
        match c {
            b'a'..=b'z' => return Some(vec![LETTERS[(c - b'a') as usize]]),
            b'0'..=b'9' => return Some(vec![DIGITS[(c - b'0') as usize]]),
            _ => {}
        }
    }
    if let Some(&(_, key)) = NAMES.iter().find(|(n, _)| *n == lower) {
        return Some(vec![key]);
    }
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse().ok()) {
        return function_key(n).map(|k| vec![k]);
    }
    if let Some(sc) = lower.strip_prefix("sc").and_then(|n| u16::from_str_radix(n, 16).ok()) {
        // Basic set-1 scan codes coincide with evdev key codes.
        if (1..0x80).contains(&sc) {
            return Some(vec![KeyCode::new(sc)]);
        }
        return EXTENDED_SCANCODES.iter().find(|(s, _)| *s == sc).map(|&(_, k)| vec![k]);
    }
    None
}

/// The AutoHotkey name for a key if there is one, otherwise its evdev name.
pub fn key_name(key: KeyCode) -> String {
    if let Some(i) = LETTERS.iter().position(|&k| k == key) {
        return ((b'a' + i as u8) as char).to_string();
    }
    if let Some(i) = DIGITS.iter().position(|&k| k == key) {
        return ((b'0' + i as u8) as char).to_string();
    }
    for n in 1..=24 {
        if function_key(n) == Some(key) {
            return format!("F{n}");
        }
    }
    if let Some((name, _)) = NAMES.iter().find(|(_, k)| *k == key) {
        return pretty_name(name);
    }
    format!("{key:?}")
}

/// "numpadenter" => "NumpadEnter", "pgup" => "PgUp", etc.
fn pretty_name(lower: &str) -> String {
    const CANONICAL: &[&str] = &[
        "Space", "Tab", "Enter", "Escape", "Backspace", "Delete", "Insert", "Home", "End", "PgUp", "PgDn",
        "Up", "Down", "Left", "Right", "CapsLock", "ScrollLock", "NumLock", "PrintScreen", "Pause", "AppsKey",
        "LShift", "RShift", "LCtrl", "RCtrl", "LAlt", "RAlt", "LWin", "RWin", "NumpadDot", "NumpadDiv",
        "NumpadMult", "NumpadAdd", "NumpadSub", "NumpadEnter",
    ];
    if let Some(n) = lower.strip_prefix("numpad").filter(|n| n.len() == 1) {
        return format!("Numpad{n}");
    }
    CANONICAL
        .iter()
        .find(|c| c.eq_ignore_ascii_case(lower))
        .map(|c| c.to_string())
        .unwrap_or_else(|| lower.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_hotkeys() {
        let h = Hotkey::parse("~]").unwrap();
        assert_eq!(h.keys, vec![KeyCode::KEY_RIGHTBRACE]);
        assert!(h.passthrough);
        assert!(h.mods.is_empty());

        let h = Hotkey::parse("+^`").unwrap();
        assert_eq!(h.keys, vec![KeyCode::KEY_GRAVE]);
        assert!(!h.passthrough);
        assert!(h.mods.shift && h.mods.ctrl && !h.mods.alt);

        assert_eq!(Hotkey::parse("~Space").unwrap().keys, vec![KeyCode::KEY_SPACE]);
        assert_eq!(Hotkey::parse("~,").unwrap().keys, vec![KeyCode::KEY_COMMA]);
        assert_eq!(Hotkey::parse("~-").unwrap().keys, vec![KeyCode::KEY_MINUS]);
        assert_eq!(Hotkey::parse("~=").unwrap().keys, vec![KeyCode::KEY_EQUAL]);
        assert_eq!(Hotkey::parse("Shift").unwrap().keys.len(), 2);
        assert_eq!(Hotkey::parse("F13").unwrap().keys, vec![KeyCode::KEY_F13]);
        assert_eq!(Hotkey::parse("sc01E").unwrap().keys, vec![KeyCode::KEY_A]);
        assert_eq!(Hotkey::parse("~KEY_LEFTMETA").unwrap().keys, vec![KeyCode::KEY_LEFTMETA]);
        assert!(Hotkey::parse("NotAKey").is_err());
    }

    #[test]
    fn format_round_trips() {
        for s in ["~]", "+^`", "~Space", "Shift", "~LShift", "!F5", "~NumpadEnter", "~Numpad5", "PgDn", "~KEY_MUTE"] {
            assert_eq!(Hotkey::parse(s).unwrap().format(), s);
        }
    }
}
