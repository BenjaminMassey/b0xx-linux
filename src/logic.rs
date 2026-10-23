//! The B0XX button -> GameCube controller logic, ported from b0xx.ahk.
//!
//! This module is pure: it knows nothing about keyboards or uinput. Feed it
//! button presses/releases and read back the controller state with `output()`.
//!
//! Stick coordinates are expressed in Melee units of 1/80 (0.0125), which is
//! the stick's native resolution, so every B0XX coordinate is an exact integer.
//! e.g. 0.7 => 56, 0.6625 => 53, 1.0 => 80.

/// One of the 25 configurable B0XX inputs. Order matches the numbering in hotkeys.ini.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Button {
    AnalogUp,
    AnalogDown,
    AnalogLeft,
    AnalogRight,
    ModX,
    ModY,
    A,
    B,
    L,
    R,
    X,
    Y,
    Z,
    CUp,
    CDown,
    CLeft,
    CRight,
    LightShield,
    MidShield,
    Start,
    DUp,
    DDown,
    DLeft,
    DRight,
    Debug,
}

impl Button {
    pub const ALL: [Button; 25] = [
        Button::AnalogUp,
        Button::AnalogDown,
        Button::AnalogLeft,
        Button::AnalogRight,
        Button::ModX,
        Button::ModY,
        Button::A,
        Button::B,
        Button::L,
        Button::R,
        Button::X,
        Button::Y,
        Button::Z,
        Button::CUp,
        Button::CDown,
        Button::CLeft,
        Button::CRight,
        Button::LightShield,
        Button::MidShield,
        Button::Start,
        Button::DUp,
        Button::DDown,
        Button::DLeft,
        Button::DRight,
        Button::Debug,
    ];

    /// 1-based index used as the key in hotkeys.ini.
    pub fn slot(self) -> usize {
        Self::ALL.iter().position(|&b| b == self).unwrap() + 1
    }

    pub fn from_slot(slot: usize) -> Option<Button> {
        Self::ALL.get(slot.checked_sub(1)?).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Button::AnalogUp => "Analog Up",
            Button::AnalogDown => "Analog Down",
            Button::AnalogLeft => "Analog Left",
            Button::AnalogRight => "Analog Right",
            Button::ModX => "ModX",
            Button::ModY => "ModY",
            Button::A => "A",
            Button::B => "B",
            Button::L => "L",
            Button::R => "R",
            Button::X => "X",
            Button::Y => "Y",
            Button::Z => "Z",
            Button::CUp => "C-stick Up",
            Button::CDown => "C-stick Down",
            Button::CLeft => "C-stick Left",
            Button::CRight => "C-stick Right",
            Button::LightShield => "Light Shield",
            Button::MidShield => "Mid Shield",
            Button::Start => "Start",
            Button::DUp => "D-pad Up",
            Button::DDown => "D-pad Down",
            Button::DLeft => "D-pad Left",
            Button::DRight => "D-pad Right",
            Button::Debug => "Debug",
        }
    }
}

/// Stick position in Melee units of 1/80.
pub type Coords = (i32, i32);

// b0xx constants (Melee value in comments)
const ORIGIN: Coords = (0, 0);
const VERTICAL: Coords = (0, 80); // 0, 1
const VERTICAL_MOD_X: Coords = (0, 43); // 0, 0.5375
const VERTICAL_MOD_Y: Coords = (0, 59); // 0, 0.7375
const HORIZONTAL: Coords = (80, 0); // 1, 0
const HORIZONTAL_MOD_X: Coords = (53, 0); // 0.6625, 0
const HORIZONTAL_MOD_Y: Coords = (27, 0); // 0.3375, 0
const QUADRANT: Coords = (56, 56); // 0.7, 0.7
const QUADRANT_MOD_X: Coords = (59, 25); // 0.7375, 0.3125
const QUADRANT_MOD_Y: Coords = (25, 59); // 0.3125, 0.7375

const AIRDODGE_VERTICAL: Coords = VERTICAL;
const AIRDODGE_VERTICAL_MOD_X: Coords = VERTICAL_MOD_X;
const AIRDODGE_VERTICAL_MOD_Y: Coords = VERTICAL_MOD_Y;
const AIRDODGE_HORIZONTAL: Coords = HORIZONTAL;
const AIRDODGE_HORIZONTAL_MOD_X: Coords = HORIZONTAL_MOD_X;
const AIRDODGE_HORIZONTAL_MOD_Y: Coords = HORIZONTAL_MOD_Y;
const AIRDODGE_QUADRANT_12: Coords = (56, 56); // 0.7, 0.7
const AIRDODGE_QUADRANT_34: Coords = (56, 55); // 0.7, 0.6875
const AIRDODGE_QUADRANT_MOD_X: Coords = (51, 30); // 0.6375, 0.375
const AIRDODGE_QUADRANT_12_MOD_Y: Coords = (38, 70); // 0.475, 0.875
const AIRDODGE_QUADRANT_34_MOD_Y: Coords = (40, 68); // 0.5, 0.85

const FIREFOX_MOD_X_C_DOWN: Coords = (56, 29); // 0.7, 0.3625     ~27 deg
const FIREFOX_MOD_X_C_LEFT: Coords = (63, 39); // 0.7875, 0.4875  ~32 deg
const FIREFOX_MOD_X_C_UP: Coords = (56, 41); // 0.7, 0.5125       ~36 deg
const FIREFOX_MOD_X_C_RIGHT: Coords = (49, 42); // 0.6125, 0.525  ~41 deg
const FIREFOX_MOD_Y_C_RIGHT: Coords = (51, 61); // 0.6375, 0.7625 ~50 deg
const FIREFOX_MOD_Y_C_UP: Coords = (41, 56); // 0.5125, 0.7       ~54 deg
const FIREFOX_MOD_Y_C_LEFT: Coords = (39, 63); // 0.4875, 0.7875  ~58 deg
const FIREFOX_MOD_Y_C_DOWN: Coords = (29, 56); // 0.3625, 0.7     ~63 deg

const EXT_FIREFOX_MOD_X: Coords = (73, 31); // 0.9125, 0.3875      ~23 deg
const EXT_FIREFOX_MOD_X_C_DOWN: Coords = (70, 36); // 0.875, 0.45   ~27 deg
const EXT_FIREFOX_MOD_X_C_LEFT: Coords = (68, 42); // 0.85, 0.525   ~32 deg
const EXT_FIREFOX_MOD_X_C_UP: Coords = (59, 43); // 0.7375, 0.5375 ~36 deg
const EXT_FIREFOX_MOD_X_C_RIGHT: Coords = (51, 43); // 0.6375, 0.5375 ~40 deg
const EXT_FIREFOX_MOD_Y_C_RIGHT: Coords = (47, 57); // 0.5875, 0.7125 ~50 deg
const EXT_FIREFOX_MOD_Y_C_UP: Coords = (47, 64); // 0.5875, 0.8    ~54 deg
const EXT_FIREFOX_MOD_Y_C_LEFT: Coords = (42, 68); // 0.525, 0.85   ~58 deg
const EXT_FIREFOX_MOD_Y_C_DOWN: Coords = (36, 70); // 0.45, 0.875   ~63 deg
const EXT_FIREFOX_MOD_Y: Coords = (31, 73); // 0.3875, 0.9125      ~67 deg

const C_ORIGIN: Coords = (0, 0);
const C_DIAGONAL: Coords = (42, 68); // 0.525, 0.85
const C_VERTICAL: Coords = (0, 80); // 0, 1
const C_HORIZONTAL: Coords = (80, 0); // 1, 0
const C_HORIZONTAL_MOD_X_UP: Coords = (72, 40); // 0.9, 0.5
const C_HORIZONTAL_MOD_X_DOWN: Coords = (72, -40); // 0.9, -0.5

/// Analog shoulder values (0-255) for the two lightshield buttons.
const LIGHT_SHIELD_VALUE: u8 = 49;
const MID_SHIELD_VALUE: u8 = 94;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Dir {
    #[default]
    None,
    Neg,
    Pos,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GcButtons {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub z: bool,
    pub l: bool,
    pub r: bool,
    pub start: bool,
    pub d_up: bool,
    pub d_down: bool,
    pub d_left: bool,
    pub d_right: bool,
}

/// The full state of the emulated GameCube controller.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Output {
    pub main: Coords,
    pub c: Coords,
    /// Analog R shoulder, 0-255.
    pub r_analog: u8,
    pub buttons: GcButtons,
}

#[derive(Default)]
pub struct B0xx {
    up: bool,
    down: bool,
    left: bool,
    right: bool,

    a: bool,
    b: bool,
    l: bool,
    r: bool,
    x: bool,
    y: bool,
    z: bool,

    light_shield: bool,
    mid_shield: bool,
    /// Whichever lightshield button was pressed most recently wins.
    last_shield: u8,

    mod_x: bool,
    mod_y: bool,

    c_up: bool,
    c_down: bool,
    c_left: bool,
    c_right: bool,

    start: bool,
    d_up: bool,
    d_down: bool,
    d_left: bool,
    d_right: bool,

    /// C buttons pressed while ModX+ModY are held act as the D-pad until released.
    c_up_as_dpad: bool,
    c_down_as_dpad: bool,
    c_left_as_dpad: bool,
    c_right_as_dpad: bool,

    most_recent_vertical: Dir,
    most_recent_horizontal: Dir,
    most_recent_vertical_c: Dir,
    most_recent_horizontal_c: Dir,

    simultaneous_horizontal_modifier_lockout: bool,
}

impl B0xx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, button: Button) {
        match button {
            Button::AnalogUp => {
                self.up = true;
                self.most_recent_vertical = Dir::Pos;
            }
            Button::AnalogDown => {
                self.down = true;
                self.most_recent_vertical = Dir::Neg;
            }
            Button::AnalogLeft => {
                self.left = true;
                self.most_recent_horizontal = Dir::Neg;
                if self.right {
                    self.simultaneous_horizontal_modifier_lockout = true;
                }
            }
            Button::AnalogRight => {
                self.right = true;
                self.most_recent_horizontal = Dir::Pos;
                if self.left {
                    self.simultaneous_horizontal_modifier_lockout = true;
                }
            }
            Button::ModX => {
                self.mod_x = true;
                // Lockout is order dependent, only applies if modifier isn't pressed after horizontals
                self.simultaneous_horizontal_modifier_lockout = false;
            }
            Button::ModY => self.mod_y = true,
            Button::A => self.a = true,
            Button::B => self.b = true,
            Button::L => self.l = true,
            Button::R => self.r = true,
            Button::X => self.x = true,
            Button::Y => self.y = true,
            Button::Z => self.z = true,
            // Pressing ModX and ModY simultaneously changes C buttons to D pad
            Button::CUp => {
                self.c_up = true;
                if self.both_mods() {
                    self.c_up_as_dpad = true;
                } else {
                    self.most_recent_vertical_c = Dir::Pos;
                }
            }
            Button::CDown => {
                self.c_down = true;
                if self.both_mods() {
                    self.c_down_as_dpad = true;
                } else {
                    self.most_recent_vertical_c = Dir::Neg;
                }
            }
            Button::CLeft => {
                self.c_left = true;
                if self.both_mods() {
                    self.c_left_as_dpad = true;
                } else {
                    self.most_recent_horizontal_c = Dir::Neg;
                }
            }
            Button::CRight => {
                self.c_right = true;
                if self.both_mods() {
                    self.c_right_as_dpad = true;
                } else {
                    self.most_recent_horizontal_c = Dir::Pos;
                }
            }
            Button::LightShield => {
                self.light_shield = true;
                self.last_shield = LIGHT_SHIELD_VALUE;
            }
            Button::MidShield => {
                self.mid_shield = true;
                self.last_shield = MID_SHIELD_VALUE;
            }
            Button::Start => self.start = true,
            Button::DUp => self.d_up = true,
            Button::DDown => self.d_down = true,
            Button::DLeft => self.d_left = true,
            Button::DRight => self.d_right = true,
            Button::Debug => {}
        }
    }

    pub fn release(&mut self, button: Button) {
        match button {
            Button::AnalogUp => self.up = false,
            Button::AnalogDown => self.down = false,
            Button::AnalogLeft => {
                self.left = false;
                self.simultaneous_horizontal_modifier_lockout = false;
            }
            Button::AnalogRight => {
                self.right = false;
                self.simultaneous_horizontal_modifier_lockout = false;
            }
            Button::ModX => {
                self.mod_x = false;
                self.simultaneous_horizontal_modifier_lockout = false;
            }
            Button::ModY => self.mod_y = false,
            Button::A => self.a = false,
            Button::B => self.b = false,
            Button::L => self.l = false,
            Button::R => self.r = false,
            Button::X => self.x = false,
            Button::Y => self.y = false,
            Button::Z => self.z = false,
            Button::CUp => {
                self.c_up = false;
                self.c_up_as_dpad = false;
            }
            Button::CDown => {
                self.c_down = false;
                self.c_down_as_dpad = false;
            }
            Button::CLeft => {
                self.c_left = false;
                self.c_left_as_dpad = false;
            }
            Button::CRight => {
                self.c_right = false;
                self.c_right_as_dpad = false;
            }
            Button::LightShield => {
                self.light_shield = false;
                self.last_shield = if self.mid_shield { MID_SHIELD_VALUE } else { 0 };
            }
            Button::MidShield => {
                self.mid_shield = false;
                self.last_shield = if self.light_shield { LIGHT_SHIELD_VALUE } else { 0 };
            }
            Button::Start => self.start = false,
            Button::DUp => self.d_up = false,
            Button::DDown => self.d_down = false,
            Button::DLeft => self.d_left = false,
            Button::DRight => self.d_right = false,
            Button::Debug => {}
        }
    }

    // Utility functions

    fn up_active(&self) -> bool {
        self.up && self.most_recent_vertical == Dir::Pos
    }

    fn down_active(&self) -> bool {
        self.down && self.most_recent_vertical == Dir::Neg
    }

    fn left_active(&self) -> bool {
        self.left && self.most_recent_horizontal == Dir::Neg
    }

    fn right_active(&self) -> bool {
        self.right && self.most_recent_horizontal == Dir::Pos
    }

    fn c_up_active(&self) -> bool {
        self.c_up && self.most_recent_vertical_c == Dir::Pos && !self.both_mods()
    }

    fn c_down_active(&self) -> bool {
        self.c_down && self.most_recent_vertical_c == Dir::Neg && !self.both_mods()
    }

    fn c_left_active(&self) -> bool {
        self.c_left && self.most_recent_horizontal_c == Dir::Neg && !self.both_mods()
    }

    fn c_right_active(&self) -> bool {
        self.c_right && self.most_recent_horizontal_c == Dir::Pos && !self.both_mods()
    }

    fn mod_x_active(&self) -> bool {
        // deactivate if either:
        //   - modY is also held
        //   - both left and right are held (and were pressed after modX) while neither up or down is active
        self.mod_x
            && !self.mod_y
            && !(self.simultaneous_horizontal_modifier_lockout && !self.any_vert())
    }

    fn mod_y_active(&self) -> bool {
        self.mod_y
            && !self.mod_x
            && !(self.simultaneous_horizontal_modifier_lockout && !self.any_vert())
    }

    fn any_vert(&self) -> bool {
        self.up_active() || self.down_active()
    }

    fn any_horiz(&self) -> bool {
        self.left_active() || self.right_active()
    }

    fn any_quadrant(&self) -> bool {
        self.any_vert() && self.any_horiz()
    }

    fn any_mod(&self) -> bool {
        self.mod_x_active() || self.mod_y_active()
    }

    fn both_mods(&self) -> bool {
        self.mod_x && self.mod_y
    }

    fn any_shield(&self) -> bool {
        self.l || self.r || self.light_shield || self.mid_shield
    }

    fn any_vert_c(&self) -> bool {
        self.c_up_active() || self.c_down_active()
    }

    fn any_horiz_c(&self) -> bool {
        self.c_left_active() || self.c_right_active()
    }

    fn any_c(&self) -> bool {
        self.any_vert_c() || self.any_horiz_c()
    }

    // Coordinate calculation. Each returns the coordinates plus a short trace
    // string describing which branch was taken (shown by the debug hotkey).

    pub fn analog_coords(&self) -> (Coords, &'static str) {
        let (coords, trace) = if self.any_shield() {
            self.analog_coords_airdodge()
        } else if self.any_mod() && self.any_quadrant() && (self.any_c() || self.b) {
            self.analog_coords_firefox()
        } else {
            self.analog_coords_with_no_shield()
        };
        (self.reflect(coords), trace)
    }

    fn reflect(&self, (mut x, mut y): Coords) -> Coords {
        if self.down_active() {
            y = -y;
        }
        if self.left_active() {
            x = -x;
        }
        (x, y)
    }

    fn analog_coords_airdodge(&self) -> (Coords, &'static str) {
        if !self.any_vert() && !self.any_horiz() {
            (ORIGIN, "L-O")
        } else if self.any_quadrant() {
            if self.mod_x_active() {
                (AIRDODGE_QUADRANT_MOD_X, "L-Q-X")
            } else if self.mod_y_active() {
                let c = if self.up_active() { AIRDODGE_QUADRANT_12_MOD_Y } else { AIRDODGE_QUADRANT_34_MOD_Y };
                (c, "L-Q-Y")
            } else {
                let c = if self.up_active() { AIRDODGE_QUADRANT_12 } else { AIRDODGE_QUADRANT_34 };
                (c, "L-Q")
            }
        } else if self.any_vert() {
            if self.mod_x_active() {
                (AIRDODGE_VERTICAL_MOD_X, "L-V-X")
            } else if self.mod_y_active() {
                (AIRDODGE_VERTICAL_MOD_Y, "L-V-Y")
            } else {
                (AIRDODGE_VERTICAL, "L-V")
            }
        } else if self.mod_x_active() {
            (AIRDODGE_HORIZONTAL_MOD_X, "L-H-X")
        } else if self.mod_y_active() {
            // turnaround side-b nerf
            let c = if self.b { AIRDODGE_HORIZONTAL } else { AIRDODGE_HORIZONTAL_MOD_Y };
            (c, "L-H-Y")
        } else {
            (AIRDODGE_HORIZONTAL, "L-H")
        }
    }

    fn analog_coords_with_no_shield(&self) -> (Coords, &'static str) {
        if !self.any_vert() && !self.any_horiz() {
            (ORIGIN, "N-O")
        } else if self.any_quadrant() {
            if self.mod_x_active() {
                (QUADRANT_MOD_X, "N-Q-X")
            } else if self.mod_y_active() {
                (QUADRANT_MOD_Y, "N-Q-Y")
            } else {
                (QUADRANT, "N-Q")
            }
        } else if self.any_vert() {
            if self.mod_x_active() {
                (VERTICAL_MOD_X, "N-V-X")
            } else if self.mod_y_active() {
                (VERTICAL_MOD_Y, "N-V-Y")
            } else {
                (VERTICAL, "N-V")
            }
        } else if self.mod_x_active() {
            (HORIZONTAL_MOD_X, "N-H-X")
        } else if self.mod_y_active() {
            // turnaround side-b nerf
            let c = if self.b { HORIZONTAL } else { HORIZONTAL_MOD_Y };
            (c, "N-H-Y")
        } else {
            (HORIZONTAL, "N-H")
        }
    }

    fn analog_coords_firefox(&self) -> (Coords, &'static str) {
        let pick = |extended: Coords, normal: Coords| if self.b { extended } else { normal };
        if self.mod_x_active() {
            if self.c_up_active() {
                (pick(EXT_FIREFOX_MOD_X_C_UP, FIREFOX_MOD_X_C_UP), "F-X-U")
            } else if self.c_down_active() {
                (pick(EXT_FIREFOX_MOD_X_C_DOWN, FIREFOX_MOD_X_C_DOWN), "F-X-D")
            } else if self.c_left_active() {
                (pick(EXT_FIREFOX_MOD_X_C_LEFT, FIREFOX_MOD_X_C_LEFT), "F-X-L")
            } else if self.c_right_active() {
                (pick(EXT_FIREFOX_MOD_X_C_RIGHT, FIREFOX_MOD_X_C_RIGHT), "F-X-R")
            } else {
                (EXT_FIREFOX_MOD_X, "F-X")
            }
        } else if self.c_up_active() {
            (pick(EXT_FIREFOX_MOD_Y_C_UP, FIREFOX_MOD_Y_C_UP), "F-Y-U")
        } else if self.c_down_active() {
            (pick(EXT_FIREFOX_MOD_Y_C_DOWN, FIREFOX_MOD_Y_C_DOWN), "F-Y-D")
        } else if self.c_left_active() {
            (pick(EXT_FIREFOX_MOD_Y_C_LEFT, FIREFOX_MOD_Y_C_LEFT), "F-Y-L")
        } else if self.c_right_active() {
            (pick(EXT_FIREFOX_MOD_Y_C_RIGHT, FIREFOX_MOD_Y_C_RIGHT), "F-Y-R")
        } else {
            (EXT_FIREFOX_MOD_Y, "F-Y")
        }
    }

    pub fn c_stick_coords(&self) -> Coords {
        let coords = if !self.any_vert_c() && !self.any_horiz_c() {
            C_ORIGIN
        } else if self.any_vert_c() && self.any_horiz_c() {
            C_DIAGONAL
        } else if self.any_vert_c() {
            C_VERTICAL
        } else if self.mod_x_active() && self.up_active() {
            C_HORIZONTAL_MOD_X_UP
        } else if self.mod_x_active() && self.down_active() {
            C_HORIZONTAL_MOD_X_DOWN
        } else {
            C_HORIZONTAL
        };

        let (mut x, mut y) = coords;
        if self.c_down_active() {
            y = -y;
        }
        if self.c_left_active() {
            x = -x;
        }
        (x, y)
    }

    pub fn output(&self) -> Output {
        Output {
            main: self.analog_coords().0,
            c: self.c_stick_coords(),
            r_analog: self.last_shield,
            buttons: GcButtons {
                a: self.a,
                b: self.b,
                x: self.x,
                y: self.y,
                z: self.z,
                l: self.l,
                r: self.r,
                start: self.start,
                d_up: self.d_up || self.c_up_as_dpad,
                d_down: self.d_down || self.c_down_as_dpad,
                d_left: self.d_left || self.c_left_as_dpad,
                d_right: self.d_right || self.c_right_as_dpad,
            },
        }
    }

    pub fn debug_report(&self) -> String {
        let mut active = Vec::new();
        let mut disabled = Vec::new();
        let mut state = |is_active: bool, is_pressed: bool, name: &'static str| {
            if is_active {
                active.push(name);
            } else if is_pressed {
                disabled.push(name);
            }
        };

        state(self.up_active(), self.up, "Up");
        state(self.down_active(), self.down, "Down");
        state(self.left_active(), self.left, "Left");
        state(self.right_active(), self.right, "Right");

        state(self.mod_x_active(), self.mod_x, "ModX");
        state(self.mod_y_active(), self.mod_y, "ModY");

        state(self.a, false, "A");
        state(self.b, false, "B");
        state(self.l, false, "L");
        state(self.r, false, "R");
        state(self.x, false, "X");
        state(self.y, false, "Y");
        state(self.z, false, "Z");

        state(self.light_shield, false, "LightShield");
        state(self.mid_shield, false, "MidShield");

        state(self.c_up_active(), self.c_up, "C-Up");
        state(self.c_down_active(), self.c_down, "C-Down");
        state(self.c_left_active(), self.c_left, "C-Left");
        state(self.c_right_active(), self.c_right, "C-Right");

        let flags = if self.simultaneous_horizontal_modifier_lockout { "SHML" } else { "" };
        let (analog, trace) = self.analog_coords();
        let c = self.c_stick_coords();

        format!(
            "Analog Stick: [{}, {}]\n\
             C Stick: [{}, {}]\n\
             Active held buttons: {}\n\
             Disabled held buttons: {}\n\
             Flags: {}\n\
             Trace: {}",
            melee(analog.0),
            melee(analog.1),
            melee(c.0),
            melee(c.1),
            active.join(", "),
            disabled.join(", "),
            flags,
            trace,
        )
    }
}

/// Formats a 1/80 unit as a Melee stick value, e.g. 56 => "0.7000".
pub fn melee(units: i32) -> String {
    format!("{:.4}", units as f64 / 80.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Button::*;

    fn with(buttons: &[Button]) -> B0xx {
        let mut b = B0xx::new();
        for &button in buttons {
            b.press(button);
        }
        b
    }

    #[test]
    fn slots_round_trip() {
        for (i, b) in Button::ALL.iter().enumerate() {
            assert_eq!(b.slot(), i + 1);
            assert_eq!(Button::from_slot(i + 1), Some(*b));
        }
        assert_eq!(Button::from_slot(0), None);
        assert_eq!(Button::from_slot(26), None);
    }

    #[test]
    fn cardinals_and_quadrants() {
        assert_eq!(with(&[]).output().main, (0, 0));
        assert_eq!(with(&[AnalogUp]).output().main, (0, 80));
        assert_eq!(with(&[AnalogLeft]).output().main, (-80, 0));
        assert_eq!(with(&[AnalogDown, AnalogRight]).output().main, (56, -56));
        assert_eq!(with(&[ModX, AnalogDown, AnalogLeft]).output().main, (-59, -25));
        assert_eq!(with(&[ModY, AnalogUp, AnalogRight]).output().main, (25, 59));
        assert_eq!(with(&[ModX, AnalogRight]).output().main, (53, 0));
    }

    #[test]
    fn second_input_priority() {
        // Holding left then pressing right goes right; releasing right does not
        // fall back to left (matches the AHK behaviour).
        let mut b = with(&[AnalogLeft, AnalogRight]);
        assert_eq!(b.output().main, (80, 0));
        b.release(AnalogRight);
        assert_eq!(b.output().main, (0, 0));
    }

    #[test]
    fn horizontal_lockout_disables_mods() {
        let b = with(&[ModX, AnalogLeft, AnalogRight]);
        assert_eq!(b.output().main, (80, 0));
        // ...unless the modifier was pressed after both horizontals.
        let b = with(&[AnalogLeft, AnalogRight, ModX]);
        assert_eq!(b.output().main, (53, 0));
        // ...or a vertical is held.
        let b = with(&[ModX, AnalogLeft, AnalogRight, AnalogUp]);
        assert_eq!(b.output().main, (59, 25));
    }

    #[test]
    fn turnaround_side_b_nerf() {
        assert_eq!(with(&[ModY, AnalogLeft]).output().main, (-27, 0));
        assert_eq!(with(&[ModY, AnalogLeft, B]).output().main, (-80, 0));
    }

    #[test]
    fn airdodge() {
        assert_eq!(with(&[L, AnalogDown, AnalogRight]).output().main, (56, -55));
        assert_eq!(with(&[R, AnalogUp, AnalogLeft]).output().main, (-56, 56));
        assert_eq!(with(&[LightShield, ModY, AnalogDown, AnalogLeft]).output().main, (-40, -68));
        assert_eq!(with(&[MidShield, ModX, AnalogUp, AnalogRight]).output().main, (51, 30));
    }

    #[test]
    fn firefox_angles() {
        assert_eq!(with(&[ModX, AnalogUp, AnalogRight, CDown]).output().main, (56, 29));
        assert_eq!(with(&[ModX, AnalogUp, AnalogRight, B]).output().main, (73, 31));
        assert_eq!(with(&[ModX, AnalogUp, AnalogRight, CDown, B]).output().main, (70, 36));
        assert_eq!(with(&[ModY, AnalogDown, AnalogLeft, CLeft]).output().main, (-39, -63));
        // C-stick is also deflected while angling.
        assert_eq!(with(&[ModX, AnalogUp, AnalogRight, CDown]).output().c, (0, -80));
    }

    #[test]
    fn c_stick() {
        assert_eq!(with(&[CLeft]).output().c, (-80, 0));
        assert_eq!(with(&[CUp, CRight]).output().c, (42, 68));
        assert_eq!(with(&[ModX, AnalogUp, CRight]).output().c, (72, 40));
        assert_eq!(with(&[ModX, AnalogDown, CLeft]).output().c, (-72, -40));
    }

    #[test]
    fn both_mods_turn_c_into_dpad() {
        let mut b = with(&[ModX, ModY, CUp]);
        let out = b.output();
        assert!(out.buttons.d_up);
        assert_eq!(out.c, (0, 0));
        b.release(CUp);
        assert!(!b.output().buttons.d_up);
    }

    #[test]
    fn lightshield() {
        let mut b = with(&[LightShield]);
        assert_eq!(b.output().r_analog, 49);
        b.press(MidShield);
        assert_eq!(b.output().r_analog, 94);
        b.release(MidShield);
        assert_eq!(b.output().r_analog, 49);
        b.release(LightShield);
        assert_eq!(b.output().r_analog, 0);
    }
}
