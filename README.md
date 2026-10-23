# b0xx-linux

A Linux port of [b0xx-ahk](https://github.com/agirardeau/b0xx-ahk): it emulates B0XX controller behaviour from a keyboard, for Melee in Dolphin/Slippi.

The AutoHotkey version uses vJoy. This version reads your keyboard through **evdev** and creates a virtual controller through **uinput**, which Dolphin reads with its built-in `evdev` backend. It works the same under X11 and Wayland, and Dolphin does not need to be focused.

The B0XX logic is a direct port of `b0xx.ahk`, with the same coordinates, modifier lockouts, firefox angles, turnaround side-B nerf, airdodge angles, lightshield values, and C-buttons acting as the D-pad while ModX+ModY are held. The same features are still missing (pivot utilt/angled ftilt nerf, SDI nerf).

## Requirements

- Read access to `/dev/input/event*` and write access to `/dev/uinput`. On most distros, that means:
  ```sh
  sudo usermod -aG input $USER     # then log out and back in
  ```
  Many desktops already give the logged-in user access to `/dev/uinput`. Check with `ls -l /dev/uinput` and `getfacl /dev/uinput`. If you don't have access, install the bundled udev rule:
  ```sh
  sudo cp 60-b0xx-linux.rules /etc/udev/rules.d/
  sudo udevadm control --reload && sudo udevadm trigger
  ```
- A keyboard with good n-key rollover (see the main README).
- Rust, to build it: `cargo build --release`. The binary is `target/release/b0xx-linux`.

## Setup

1. Install the Dolphin controller profile:
   ```sh
   b0xx-linux install-profile
   ```
   This writes `b0xx-linux.ini` into Slippi's (`~/.config/SlippiOnline/Config/Profiles/GCPad/`) and/or Dolphin's profile directory, depending on which exist. You can also copy `dolphin/b0xx-linux.ini` there yourself.
2. Start `b0xx-linux` **before** opening Dolphin's controller settings. Dolphin only detects new devices on startup or when you click *Refresh*.
3. In Dolphin, go to *Controllers*, set Port 1 to *Standard Controller*, click *Configure*, load the `b0xx-linux` profile, and confirm that the device is `evdev/0/b0xx-linux Virtual Controller`.

## Usage

```sh
b0xx-linux                      # run (reads every keyboard, including ones plugged in later)
b0xx-linux configure            # bind keys interactively (replaces the AHK "Edit Controls" GUI)
b0xx-linux list-devices         # show input devices
b0xx-linux --device /dev/input/by-id/usb-...-event-kbd   # only read one keyboard
b0xx-linux --verbose            # log every button press/release
b0xx-linux --dolphin mainline   # see "Dolphin versions" below
```

While running:

| Keys | Action |
|---|---|
| Ctrl+Alt+S or Pause | Suspend/resume hotkeys (the controller returns to neutral) |
| Ctrl+Alt+R | Reload `hotkeys.ini` |
| Ctrl+C (in the terminal) | Quit |

Messages show up in the terminal, and as desktop notifications if `notify-send` is installed. The Debug hotkey (Ctrl+Shift+\` by default) prints the same report as the AHK version.

## Configuration

Hotkeys live in `~/.config/b0xx-linux/hotkeys.ini`, or the file given with `--config`. The file is created on first run. It uses **the same format as the AHK version's `hotkeys.ini`**, so you can point `--config` at your existing file. The numbered lines are explained in the [main README](../README.md#hotkeysini-index).

- Key names are AutoHotkey's: `a`, `]`, `Space`, `LShift`, `Shift`, `Numpad5`, `F13`, `sc01E`, and so on. They mean the key in that position on a US layout. Linux key names such as `KEY_LEFTMETA` also work, for keys AutoHotkey has no name for.
- The `+`, `^`, `!`, and `#` prefixes require Shift, Ctrl, Alt, or Super. A plain key fires whatever modifiers are held. If both `p` and `^p` are bound, Ctrl+P fires only `^p`.
- A `~` prefix means the key **also** does its normal thing. Without `~`, its normal behaviour is prevented ("Prevent Default Behavior" in the AHK GUI). To do that, b0xx-linux *grabs* every keyboard that has such a key and re-sends all of that keyboard's other keys through a virtual "b0xx-linux Passthrough Keyboard". Because of that, the default config uses `~` everywhere, including on Debug, unlike the AHK default. Devices that are also mice are never grabbed.

## Dolphin versions

Dolphin turns the virtual stick position into the GameCube's 0–255 values differently depending on the version. b0xx-linux encodes positions so that Melee receives the exact B0XX coordinates:

- `--dolphin ishiiruka` (default): Slippi Online and other Ishiiruka-based builds (Faster Melee, etc.). The mapping is linear.
- `--dolphin mainline`: current mainline Dolphin, which reshapes stick input onto an octagonal gate. This mode undoes that shaping. It relies on the profile's `Gate Size = 100` settings, so don't change the stick calibration or gate in Dolphin. This mode is untested against a real mainline build.

To verify in game, use 20XX's debug mode (see "Troubleshooting" in the main README) to display the stick coordinates, and compare them against the Debug hotkey's report.

## Troubleshooting

- **"No keyboards found" / permission denied**: see Requirements. Run `b0xx-linux list-devices` to see what's readable.
- **Dolphin doesn't list the controller**: start b0xx-linux first, then click *Refresh* in Dolphin's controller config, or restart Dolphin.
- **Something else (like Steam) also reacts to the virtual controller**: it's an ordinary gamepad to the rest of the system. Disable it in that program if needed.
- **"Waiting for all keys to be released"**: when grabbing, b0xx-linux waits until nothing is held on that keyboard, so that no key stays stuck.

## Development

`cargo test` runs unit tests for the ported B0XX logic (`src/logic.rs`), hotkey parsing, and the coordinate encoding for both Dolphin flavours.

| File | Contents |
|---|---|
| `src/logic.rs` | Pure port of the B0XX state machine from `b0xx.ahk`. Coordinates are integers in Melee's 1/80 units. |
| `src/gamepad.rs` | The uinput controller and the encoding of Melee coordinates into axis values. |
| `src/input.rs` | Keyboard discovery, reader threads, grabbing, the passthrough keyboard. |
| `src/app.rs` | The main loop: hotkey matching, suspend/reload. |
| `src/keys.rs`, `src/config.rs` | AutoHotkey key names and the `hotkeys.ini` format. |
| `src/configure.rs` | The interactive `configure` command. |
