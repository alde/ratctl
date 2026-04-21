use crate::types::ButtonCode;

/// Which protocol implementation to use for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    /// ASUS ROG HID protocol (64-byte hidraw packets).
    Asus,
    /// Razer HID protocol (90-byte USB control transfers).
    Razer,
    /// Corsair HID protocol (64-byte hidraw packets).
    Corsair,
}

/// Describes a supported mouse model.
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub name: &'static str,
    pub vendor_id: u16,
    pub product_ids: &'static [u16],
    pub protocol: ProtocolKind,
    pub num_profiles: usize,
    pub button_slots: &'static [ButtonCode],
    /// Number of buttons per HID button group (ASUS only, others use &[]).
    pub button_group_sizes: &'static [usize],
    pub num_leds: usize,
    pub led_names: &'static [&'static str],
    pub num_dpi_presets: usize,
    pub dpi_min: u16,
    pub dpi_max: u16,
    pub dpi_step: u16,
    pub brightness_max: u8,
}

impl DeviceDescriptor {
    pub fn matches(&self, vid: u16, pid: u16) -> bool {
        self.vendor_id == vid && self.product_ids.contains(&pid)
    }
}

// --- Device registry ---

pub static REGISTRY: &[&DeviceDescriptor] = &[
    &SPATHA_X,
    &DEATHADDER_V2,
    &SCIMITAR_RGB_ELITE,
];

pub fn find_descriptor(vid: u16, pid: u16) -> Option<&'static DeviceDescriptor> {
    REGISTRY.iter().find(|d| d.matches(vid, pid)).copied()
}

// --- ASUS devices ---

pub static SPATHA_X: DeviceDescriptor = DeviceDescriptor {
    name: "ASUS ROG Spatha X",
    vendor_id: 0x0B05,
    product_ids: &[0x1977, 0x1979],
    protocol: ProtocolKind::Asus,
    num_profiles: 5,
    // Group 0: primary (8), Group 1: side buttons (6)
    // DpiCycle and DpiTarget share one physical button.
    button_slots: &[
        ButtonCode::LeftClick,
        ButtonCode::RightClick,
        ButtonCode::MiddleClick,
        ButtonCode::Back,
        ButtonCode::Forward,
        ButtonCode::DpiCycle,
        ButtonCode::ScrollUp,
        ButtonCode::ScrollDown,
        ButtonCode::SideA,
        ButtonCode::SideB,
        ButtonCode::SideC,
        ButtonCode::SideD,
        ButtonCode::SideE,
        ButtonCode::SideF,
    ],
    button_group_sizes: &[8, 6],
    num_leds: 3,
    led_names: &["logo", "wheel", "side"],
    num_dpi_presets: 4,
    dpi_min: 100,
    dpi_max: 19000,
    dpi_step: 50,
    brightness_max: 4,
};

// --- Razer devices ---

pub static DEATHADDER_V2: DeviceDescriptor = DeviceDescriptor {
    name: "Razer DeathAdder V2",
    vendor_id: 0x1532,
    product_ids: &[0x0084],
    protocol: ProtocolKind::Razer,
    num_profiles: 1,
    button_slots: &[
        ButtonCode::LeftClick,
        ButtonCode::RightClick,
        ButtonCode::MiddleClick,
        ButtonCode::Back,
        ButtonCode::Forward,
        ButtonCode::DpiCycle,
        ButtonCode::ScrollUp,
        ButtonCode::ScrollDown,
    ],
    button_group_sizes: &[],
    num_leds: 2,
    led_names: &["logo", "scroll"],
    num_dpi_presets: 5,
    dpi_min: 100,
    dpi_max: 20000,
    dpi_step: 50,
    brightness_max: 255,
};

// --- Corsair devices ---

pub static SCIMITAR_RGB_ELITE: DeviceDescriptor = DeviceDescriptor {
    name: "Corsair Scimitar RGB Elite",
    vendor_id: 0x1B1C,
    product_ids: &[0x1B8B],
    protocol: ProtocolKind::Corsair,
    num_profiles: 3,
    button_slots: &[
        ButtonCode::LeftClick,
        ButtonCode::RightClick,
        ButtonCode::MiddleClick,
        ButtonCode::Back,
        ButtonCode::Forward,
        ButtonCode::DpiCycle,
        ButtonCode::ScrollUp,
        ButtonCode::ScrollDown,
        // 12 side buttons (Scimitar thumb grid)
        ButtonCode::SideA,
        ButtonCode::SideB,
        ButtonCode::SideC,
        ButtonCode::SideD,
        ButtonCode::SideE,
        ButtonCode::SideF,
        ButtonCode::SideG,
        ButtonCode::SideH,
        ButtonCode::SideI,
        ButtonCode::SideJ,
        ButtonCode::SideK,
        ButtonCode::SideL,
    ],
    button_group_sizes: &[],
    num_leds: 4,
    led_names: &["logo", "scroll", "side_panel", "dpi_indicator"],
    num_dpi_presets: 5,
    dpi_min: 100,
    dpi_max: 18000,
    dpi_step: 1,
    brightness_max: 255,
};
