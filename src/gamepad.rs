//! The virtual controller (a uinput device) that Dolphin reads through its evdev backend.
//!
//! Dolphin's evdev backend names inputs by their order on the device, so the
//! layout below determines the names used in the bundled Dolphin profile:
//!
//! | Dolphin name  | evdev code      | GameCube      |
//! |---------------|-----------------|---------------|
//! | Axis 0 -/+    | ABS_X           | Main stick X  |
//! | Axis 1 -/+    | ABS_Y           | Main stick Y (up is -) |
//! | Axis 2 -/+    | ABS_RX          | C-stick X     |
//! | Axis 3 -/+    | ABS_RY          | C-stick Y (up is -) |
//! | Axis 4 +      | ABS_RZ          | Analog R      |
//! | Button 0..11  | see `BUTTONS`   |               |

use crate::logic::{Coords, Output};
use anyhow::{Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode, UinputAbsSetup};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8};

pub const DEVICE_NAME: &str = "b0xx-linux Virtual Controller";

/// Axis range is symmetric so that Dolphin maps raw 0 to exactly 0.0 and ±MAX to ±1.0.
const AXIS_MAX: i32 = 32767;

/// GameCube stick byte = 128 + round_or_floor(value * 127). Melee then reads (byte - 128) / 80.
const GC_STICK_RADIUS: f64 = 127.0;
/// GameCube trigger byte = round_or_floor(value * 255).
const GC_TRIGGER_MAX: f64 = 255.0;
/// Aim a quarter step above the target byte, so it's hit whether Dolphin truncates
/// (Ishiiruka/Slippi) or rounds (mainline) when converting to a byte.
const BYTE_BIAS: f64 = 0.25;

/// In Dolphin's numbering order (sorted by evdev code). Index = "Button N".
const BUTTONS: [KeyCode; 12] = [
    KeyCode::BTN_SOUTH,      // 0: A
    KeyCode::BTN_EAST,       // 1: B
    KeyCode::BTN_NORTH,      // 2: X
    KeyCode::BTN_WEST,       // 3: Y
    KeyCode::BTN_TR,         // 4: Z
    KeyCode::BTN_TL2,        // 5: L (digital)
    KeyCode::BTN_TR2,        // 6: R (digital)
    KeyCode::BTN_START,      // 7: Start
    KeyCode::BTN_DPAD_UP,    // 8
    KeyCode::BTN_DPAD_DOWN,  // 9
    KeyCode::BTN_DPAD_LEFT,  // 10
    KeyCode::BTN_DPAD_RIGHT, // 11
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum DolphinFlavor {
    /// Slippi / Faster Melee / other Ishiiruka-based builds: linear stick mapping.
    Ishiiruka,
    /// Mainline Dolphin (2019+): reshapes stick input to an octagonal gate.
    Mainline,
}

pub struct Gamepad {
    device: VirtualDevice,
    flavor: DolphinFlavor,
    last: Option<Output>,
}

impl Gamepad {
    pub fn new(flavor: DolphinFlavor) -> Result<Gamepad> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for key in BUTTONS {
            keys.insert(key);
        }
        let axis = |code| UinputAbsSetup::new(code, AbsInfo::new(0, -AXIS_MAX, AXIS_MAX, 0, 0, 0));
        let device = VirtualDevice::builder()
            .context("opening /dev/uinput (see README for permissions)")?
            .name(DEVICE_NAME)
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x1209, 0xb0c5, 1))
            .with_keys(&keys)?
            .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_X))?
            .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_Y))?
            .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_RX))?
            .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_RY))?
            .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_RZ))?
            .build()
            .context("creating virtual controller")?;
        Ok(Gamepad { device, flavor, last: None })
    }

    /// Sends the new controller state, emitting only what changed.
    pub fn update(&mut self, out: &Output) -> Result<()> {
        if self.last.as_ref() == Some(out) {
            return Ok(());
        }
        let b = &out.buttons;
        let pressed = [
            b.a, b.b, b.x, b.y, b.z, b.l, b.r, b.start, b.d_up, b.d_down, b.d_left, b.d_right,
        ];
        let main = stick_axes(out.main, self.flavor);
        let c = stick_axes(out.c, self.flavor);
        let axes = [main.0, main.1, c.0, c.1, trigger_axis(out.r_analog)];

        let mut events = Vec::new();
        for (key, &down) in BUTTONS.iter().zip(&pressed) {
            events.push(InputEvent::new(EventType::KEY.0, key.code(), down as i32));
        }
        let codes = [
            AbsoluteAxisCode::ABS_X,
            AbsoluteAxisCode::ABS_Y,
            AbsoluteAxisCode::ABS_RX,
            AbsoluteAxisCode::ABS_RY,
            AbsoluteAxisCode::ABS_RZ,
        ];
        for (code, value) in codes.iter().zip(axes) {
            events.push(InputEvent::new(EventType::ABSOLUTE.0, code.0, value));
        }
        // The kernel drops events whose value didn't change, so sending everything is fine.
        self.device.emit(&events).context("writing to virtual controller")?;
        self.last = Some(*out);
        Ok(())
    }
}

/// Converts Melee stick units (1/80) to raw evdev axis values (x, y), with y inverted
/// so that up is negative as on a normal gamepad.
fn stick_axes((x, y): Coords, flavor: DolphinFlavor) -> (i32, i32) {
    // Desired Dolphin stick value, in -1..1, such that it lands on the right byte.
    // A zero axis is left exactly at zero so the stick rests perfectly centred.
    let target = |units: i32| if units == 0 { 0.0 } else { (units as f64 + BYTE_BIAS) / GC_STICK_RADIUS };
    let mut dx = target(x);
    let mut dy = target(y);

    if flavor == DolphinFlavor::Mainline && (x, y) != (0, 0) {
        // Mainline Dolphin maps the (default, square) input shape onto an octagonal
        // gate; the bundled profile sets the gate size to 100%. Undo that so the
        // output is linear: scale by square radius / octagon radius at this angle.
        let angle = dy.atan2(dx);
        let scale = square_radius(angle) / octagon_radius(angle);
        dx *= scale;
        dy *= scale;
    }
    (to_axis(dx), -to_axis(dy))
}

fn trigger_axis(value: u8) -> i32 {
    if value == 0 { 0 } else { to_axis((value as f64 + BYTE_BIAS) / GC_TRIGGER_MAX) }
}

fn to_axis(v: f64) -> i32 {
    (v * AXIS_MAX as f64).round().clamp(-AXIS_MAX as f64, AXIS_MAX as f64) as i32
}

/// Distance from the centre to the edge of a unit square at `angle`.
fn square_radius(angle: f64) -> f64 {
    1.0 / ((angle + FRAC_PI_4).rem_euclid(FRAC_PI_2) - FRAC_PI_4).cos()
}

/// Distance from the centre to the edge of a unit octagon (vertices on the axes and diagonals).
fn octagon_radius(angle: f64) -> f64 {
    FRAC_PI_8.cos() / ((angle + FRAC_PI_8).rem_euclid(FRAC_PI_4) - FRAC_PI_8).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Ishiiruka does: linear, then truncate.
    fn ishiiruka_byte(raw: i32) -> i32 {
        (128.0 + raw as f64 / AXIS_MAX as f64 * 127.0).floor() as i32
    }

    #[test]
    fn ishiiruka_hits_exact_bytes() {
        for units in -80..=80 {
            let (x, y) = stick_axes((units, units), DolphinFlavor::Ishiiruka);
            assert_eq!(ishiiruka_byte(x), 128 + units);
            assert_eq!(ishiiruka_byte(-y), 128 + units);
            // Mainline rounds instead of truncating.
            let rounded = (128.0 + x as f64 / AXIS_MAX as f64 * 127.0).round() as i32;
            assert_eq!(rounded, 128 + units);
        }
        assert_eq!(stick_axes((0, 0), DolphinFlavor::Ishiiruka), (0, 0));
        assert_eq!(stick_axes((0, 0), DolphinFlavor::Mainline), (0, 0));
    }

    #[test]
    fn mainline_inverse_reshape() {
        // Simulate mainline's reshape: out = in * octagon(angle) / square(angle).
        for &(x, y) in &[(80, 0), (56, 56), (59, 25), (73, 31), (-42, 68), (0, -80), (27, 0), (51, -30)] {
            let (rx, ry) = stick_axes((x, y), DolphinFlavor::Mainline);
            let (ix, iy) = (rx as f64 / AXIS_MAX as f64, -ry as f64 / AXIS_MAX as f64);
            let angle = iy.atan2(ix);
            let k = octagon_radius(angle) / square_radius(angle);
            let bx = (128.0 + ix * k * 127.0).round() as i32;
            let by = (128.0 + iy * k * 127.0).round() as i32;
            assert_eq!((bx - 128, by - 128), (x, y));
        }
    }

    #[test]
    fn trigger_bytes() {
        for v in [49u8, 94] {
            let raw = trigger_axis(v);
            assert_eq!((raw as f64 / AXIS_MAX as f64 * 255.0).floor() as u8, v);
            assert_eq!((raw as f64 / AXIS_MAX as f64 * 255.0).round() as u8, v);
        }
    }
}
