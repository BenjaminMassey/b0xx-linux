//! Reading and writing hotkeys.ini, in the same format as the AutoHotkey version.

use crate::keys::Hotkey;
use crate::logic::Button;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

/// Same layout as the AHK default, except Debug passes through (so the default
/// config never needs to grab the keyboard).
pub const DEFAULT_HOTKEYS: &str = "\
[Hotkeys]
1=~]
2=~3
3=~2
4=~4
5=~v
6=~b
7=~m
8=~o
9=~q
10=~9
11=~p
12=~0
13=~[
14=~k
15=~Space
16=~n
17=~,
18=~-
19=~=
20=~7
21=~Up
22=~Down
23=~Left
24=~Right
25=~+^`
";

pub struct Binding {
    pub button: Button,
    pub hotkey: Hotkey,
}

/// The raw hotkey strings, indexed by slot - 1. Empty string = unbound.
pub struct Config {
    pub path: PathBuf,
    pub raw: [String; 25],
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("b0xx-linux").join("hotkeys.ini")
}

impl Config {
    /// Loads the config, creating it from the defaults if it doesn't exist.
    pub fn load_or_create(path: &Path) -> Result<Config> {
        if !path.exists() {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            }
            fs::write(path, DEFAULT_HOTKEYS).with_context(|| format!("writing {}", path.display()))?;
            eprintln!("Created default config at {}", path.display());
        }
        let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Config { path: path.to_owned(), raw: parse_ini(&text) })
    }

    pub fn bindings(&self) -> Result<Vec<Binding>> {
        let mut bindings = Vec::new();
        let mut errors = Vec::new();
        for (i, raw) in self.raw.iter().enumerate() {
            if raw.is_empty() {
                continue;
            }
            let button = Button::from_slot(i + 1).unwrap();
            match Hotkey::parse(raw) {
                Ok(hotkey) => bindings.push(Binding { button, hotkey }),
                Err(e) => errors.push(format!("  line {} ({}): {e}", i + 1, button.label())),
            }
        }
        if !errors.is_empty() {
            bail!("invalid entries in {}:\n{}", self.path.display(), errors.join("\n"));
        }
        Ok(bindings)
    }

    pub fn save(&self) -> Result<()> {
        let mut text = String::from("[Hotkeys]\n");
        for (i, raw) in self.raw.iter().enumerate() {
            text.push_str(&format!("{}={}\n", i + 1, raw));
        }
        fs::write(&self.path, text).with_context(|| format!("writing {}", self.path.display()))
    }
}

fn parse_ini(text: &str) -> [String; 25] {
    let mut raw: [String; 25] = Default::default();
    let mut in_hotkeys = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_hotkeys = line.eq_ignore_ascii_case("[hotkeys]");
            continue;
        }
        if !in_hotkeys {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if let Ok(slot @ 1..=25) = key.trim().parse::<usize>() {
            raw[slot - 1] = value.trim().to_string();
        }
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let config = Config { path: "test".into(), raw: parse_ini(DEFAULT_HOTKEYS) };
        assert_eq!(config.bindings().unwrap().len(), 25);
    }

    #[test]
    fn original_ahk_config_parses() {
        let ahk = include_str!("../../hotkeys.ini");
        let config = Config { path: "test".into(), raw: parse_ini(ahk) };
        let bindings = config.bindings().unwrap();
        assert_eq!(bindings.len(), 25);
        assert!(!bindings[24].hotkey.passthrough);
    }
}
