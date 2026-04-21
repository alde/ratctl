//! Shared data types for mouse configuration.
//!
//! These types form the common vocabulary between vendor-specific protocols,
//! the config file format, and the TUI.

pub const POLLING_RATES: [u16; 4] = [125, 250, 500, 1000];
pub const DEBOUNCE_TIMES: [u8; 8] = [4, 8, 12, 16, 20, 24, 28, 32];

// --- Button codes ---

/// Physical button identifiers used across protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ButtonCode {
    LeftClick = 0xF0,
    RightClick = 0xF1,
    MiddleClick = 0xF2,
    Back = 0xE4,
    Forward = 0xE5,
    DpiCycle = 0xE6,
    DpiTarget = 0xE7,
    ScrollUp = 0xE8,
    ScrollDown = 0xE9,
    SideA = 0xEA,
    SideB = 0xEB,
    SideC = 0xEC,
    SideD = 0xED,
    SideE = 0xEE,
    SideF = 0xEF,
    SideG = 0xDA,
    SideH = 0xDB,
    SideI = 0xDC,
    SideJ = 0xDD,
    SideK = 0xDE,
    SideL = 0xDF,
    Disabled = 0xFF,
}

impl ButtonCode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0xF0 => Some(Self::LeftClick),
            0xF1 => Some(Self::RightClick),
            0xF2 => Some(Self::MiddleClick),
            0xE4 => Some(Self::Back),
            0xE5 => Some(Self::Forward),
            0xE6 => Some(Self::DpiCycle),
            0xE7 => Some(Self::DpiTarget),
            0xE8 => Some(Self::ScrollUp),
            0xE9 => Some(Self::ScrollDown),
            0xEA => Some(Self::SideA),
            0xEB => Some(Self::SideB),
            0xEC => Some(Self::SideC),
            0xED => Some(Self::SideD),
            0xEE => Some(Self::SideE),
            0xEF => Some(Self::SideF),
            0xDA => Some(Self::SideG),
            0xDB => Some(Self::SideH),
            0xDC => Some(Self::SideI),
            0xDD => Some(Self::SideJ),
            0xDE => Some(Self::SideK),
            0xDF => Some(Self::SideL),
            0xFF => Some(Self::Disabled),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::LeftClick => "left_click",
            Self::RightClick => "right_click",
            Self::MiddleClick => "middle_click",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::DpiCycle => "dpi_cycle",
            Self::DpiTarget => "dpi_target",
            Self::ScrollUp => "scroll_up",
            Self::ScrollDown => "scroll_down",
            Self::SideA => "side_a",
            Self::SideB => "side_b",
            Self::SideC => "side_c",
            Self::SideD => "side_d",
            Self::SideE => "side_e",
            Self::SideF => "side_f",
            Self::SideG => "side_g",
            Self::SideH => "side_h",
            Self::SideI => "side_i",
            Self::SideJ => "side_j",
            Self::SideK => "side_k",
            Self::SideL => "side_l",
            Self::Disabled => "disabled",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "left_click" => Some(Self::LeftClick),
            "right_click" => Some(Self::RightClick),
            "middle_click" => Some(Self::MiddleClick),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            "dpi_cycle" => Some(Self::DpiCycle),
            "dpi_target" => Some(Self::DpiTarget),
            "scroll_up" => Some(Self::ScrollUp),
            "scroll_down" => Some(Self::ScrollDown),
            "side_a" => Some(Self::SideA),
            "side_b" => Some(Self::SideB),
            "side_c" => Some(Self::SideC),
            "side_d" => Some(Self::SideD),
            "side_e" => Some(Self::SideE),
            "side_f" => Some(Self::SideF),
            "side_g" => Some(Self::SideG),
            "side_h" => Some(Self::SideH),
            "side_i" => Some(Self::SideI),
            "side_j" => Some(Self::SideJ),
            "side_k" => Some(Self::SideK),
            "side_l" => Some(Self::SideL),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

// --- LED modes ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LedMode {
    Static = 0,
    Breathing = 1,
    Cycle = 2,
    Rainbow = 3,
    Reactive = 4,
    Custom = 5,
    Battery = 6,
}

impl LedMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Static),
            1 => Some(Self::Breathing),
            2 => Some(Self::Cycle),
            3 => Some(Self::Rainbow),
            4 => Some(Self::Reactive),
            5 => Some(Self::Custom),
            6 => Some(Self::Battery),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Breathing => "breathing",
            Self::Cycle => "cycle",
            Self::Rainbow => "rainbow",
            Self::Reactive => "reactive",
            Self::Custom => "custom",
            Self::Battery => "battery",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "static" => Some(Self::Static),
            "breathing" => Some(Self::Breathing),
            "cycle" => Some(Self::Cycle),
            "rainbow" => Some(Self::Rainbow),
            "reactive" => Some(Self::Reactive),
            "custom" => Some(Self::Custom),
            "battery" => Some(Self::Battery),
            _ => None,
        }
    }
}

// --- Data structures ---

/// Whether a binding targets a mouse action or a keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BindingKind {
    Keyboard = 0,
    Mouse = 1,
}

/// A single button binding as stored on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonBinding {
    pub action_code: u8,
    pub kind: BindingKind,
}

impl ButtonBinding {
    pub fn mouse_action(code: ButtonCode) -> Self {
        Self {
            action_code: code as u8,
            kind: BindingKind::Mouse,
        }
    }

    pub fn keyboard_key(hid_keycode: u8) -> Self {
        Self {
            action_code: hid_keycode,
            kind: BindingKind::Keyboard,
        }
    }

    pub fn disabled() -> Self {
        Self::mouse_action(ButtonCode::Disabled)
    }

    /// Wire format: action_type byte (0 = keyboard, 1 = mouse).
    pub fn action_type_byte(&self) -> u8 {
        self.kind as u8
    }
}

#[derive(Debug, Clone)]
pub struct LedConfig {
    pub mode: LedMode,
    pub brightness: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone)]
pub struct Settings {
    /// Actual DPI values.
    pub dpi_presets: Vec<u16>,
    /// Actual polling rate in Hz.
    pub polling_rate: u16,
    /// Actual debounce time in ms.
    pub debounce_ms: u8,
    pub angle_snapping: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileData {
    pub current_profile: u8,
    pub firmware_version: String,
}

/// Complete state of one profile.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub buttons: Vec<ButtonBinding>,
    pub settings: Settings,
    pub leds: Vec<LedConfig>,
}

// --- USB HID keycode table (subset of common keys) ---

/// Map a USB HID keycode to a human-readable name.
pub fn hid_keycode_to_name(code: u8) -> Option<&'static str> {
    match code {
        0x04 => Some("a"),
        0x05 => Some("b"),
        0x06 => Some("c"),
        0x07 => Some("d"),
        0x08 => Some("e"),
        0x09 => Some("f"),
        0x0A => Some("g"),
        0x0B => Some("h"),
        0x0C => Some("i"),
        0x0D => Some("j"),
        0x0E => Some("k"),
        0x0F => Some("l"),
        0x10 => Some("m"),
        0x11 => Some("n"),
        0x12 => Some("o"),
        0x13 => Some("p"),
        0x14 => Some("q"),
        0x15 => Some("r"),
        0x16 => Some("s"),
        0x17 => Some("t"),
        0x18 => Some("u"),
        0x19 => Some("v"),
        0x1A => Some("w"),
        0x1B => Some("x"),
        0x1C => Some("y"),
        0x1D => Some("z"),
        0x1E => Some("1"),
        0x1F => Some("2"),
        0x20 => Some("3"),
        0x21 => Some("4"),
        0x22 => Some("5"),
        0x23 => Some("6"),
        0x24 => Some("7"),
        0x25 => Some("8"),
        0x26 => Some("9"),
        0x27 => Some("0"),
        0x28 => Some("enter"),
        0x29 => Some("escape"),
        0x2A => Some("backspace"),
        0x2B => Some("tab"),
        0x2C => Some("space"),
        0x2D => Some("minus"),
        0x2E => Some("equal"),
        0x2F => Some("left_bracket"),
        0x30 => Some("right_bracket"),
        0x31 => Some("backslash"),
        0x33 => Some("semicolon"),
        0x34 => Some("apostrophe"),
        0x35 => Some("grave"),
        0x36 => Some("comma"),
        0x37 => Some("dot"),
        0x38 => Some("slash"),
        0x39 => Some("caps_lock"),
        0x3A => Some("f1"),
        0x3B => Some("f2"),
        0x3C => Some("f3"),
        0x3D => Some("f4"),
        0x3E => Some("f5"),
        0x3F => Some("f6"),
        0x40 => Some("f7"),
        0x41 => Some("f8"),
        0x42 => Some("f9"),
        0x43 => Some("f10"),
        0x44 => Some("f11"),
        0x45 => Some("f12"),
        0x46 => Some("print_screen"),
        0x47 => Some("scroll_lock"),
        0x48 => Some("pause"),
        0x49 => Some("insert"),
        0x4A => Some("home"),
        0x4B => Some("page_up"),
        0x4C => Some("delete"),
        0x4D => Some("end"),
        0x4E => Some("page_down"),
        0x4F => Some("right"),
        0x50 => Some("left"),
        0x51 => Some("down"),
        0x52 => Some("up"),
        _ => None,
    }
}

/// Map a key name to USB HID keycode.
pub fn name_to_hid_keycode(name: &str) -> Option<u8> {
    match name {
        "a" => Some(0x04),
        "b" => Some(0x05),
        "c" => Some(0x06),
        "d" => Some(0x07),
        "e" => Some(0x08),
        "f" => Some(0x09),
        "g" => Some(0x0A),
        "h" => Some(0x0B),
        "i" => Some(0x0C),
        "j" => Some(0x0D),
        "k" => Some(0x0E),
        "l" => Some(0x0F),
        "m" => Some(0x10),
        "n" => Some(0x11),
        "o" => Some(0x12),
        "p" => Some(0x13),
        "q" => Some(0x14),
        "r" => Some(0x15),
        "s" => Some(0x16),
        "t" => Some(0x17),
        "u" => Some(0x18),
        "v" => Some(0x19),
        "w" => Some(0x1A),
        "x" => Some(0x1B),
        "y" => Some(0x1C),
        "z" => Some(0x1D),
        "1" => Some(0x1E),
        "2" => Some(0x1F),
        "3" => Some(0x20),
        "4" => Some(0x21),
        "5" => Some(0x22),
        "6" => Some(0x23),
        "7" => Some(0x24),
        "8" => Some(0x25),
        "9" => Some(0x26),
        "0" => Some(0x27),
        "enter" => Some(0x28),
        "escape" => Some(0x29),
        "backspace" => Some(0x2A),
        "tab" => Some(0x2B),
        "space" => Some(0x2C),
        "minus" => Some(0x2D),
        "equal" => Some(0x2E),
        "left_bracket" => Some(0x2F),
        "right_bracket" => Some(0x30),
        "backslash" => Some(0x31),
        "semicolon" => Some(0x33),
        "apostrophe" => Some(0x34),
        "grave" => Some(0x35),
        "comma" => Some(0x36),
        "dot" => Some(0x37),
        "slash" => Some(0x38),
        "caps_lock" => Some(0x39),
        "f1" => Some(0x3A),
        "f2" => Some(0x3B),
        "f3" => Some(0x3C),
        "f4" => Some(0x3D),
        "f5" => Some(0x3E),
        "f6" => Some(0x3F),
        "f7" => Some(0x40),
        "f8" => Some(0x41),
        "f9" => Some(0x42),
        "f10" => Some(0x43),
        "f11" => Some(0x44),
        "f12" => Some(0x45),
        "print_screen" => Some(0x46),
        "scroll_lock" => Some(0x47),
        "pause" => Some(0x48),
        "insert" => Some(0x49),
        "home" => Some(0x4A),
        "page_up" => Some(0x4B),
        "delete" => Some(0x4C),
        "end" => Some(0x4D),
        "page_down" => Some(0x4E),
        "right" => Some(0x4F),
        "left" => Some(0x50),
        "down" => Some(0x51),
        "up" => Some(0x52),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_code_roundtrip() {
        for code in [0xF0, 0xF1, 0xF2, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xFF] {
            let bc = ButtonCode::from_u8(code).unwrap();
            let name = bc.name();
            let back = ButtonCode::from_name(name).unwrap();
            assert_eq!(bc, back, "ButtonCode roundtrip failed for {:#04x}", code);
        }
    }

    #[test]
    fn test_led_mode_roundtrip() {
        for i in 0..=6u8 {
            let mode = LedMode::from_u8(i).unwrap();
            let name = mode.name();
            let back = LedMode::from_name(name).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn test_hid_keycode_roundtrip() {
        for (code, name) in [(0x04, "a"), (0x1E, "1"), (0x28, "enter"), (0x3A, "f1")] {
            assert_eq!(hid_keycode_to_name(code), Some(name));
            assert_eq!(name_to_hid_keycode(name), Some(code));
        }
    }
}
