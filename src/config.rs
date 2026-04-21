use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::devices::DeviceDescriptor;
use crate::types::{
    self, BindingKind, ButtonBinding, ButtonCode, DeviceProfile, LedConfig, LedMode, Settings,
    DEBOUNCE_TIMES, POLLING_RATES,
};

// --- TOML config types ---

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub active_profile: Option<u8>,
    #[serde(default)]
    pub profile: BTreeMap<String, ProfileConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub polling_rate: u16,
    pub debounce_ms: u8,
    #[serde(default)]
    pub angle_snapping: bool,
    pub dpi: BTreeMap<String, u16>,
    pub buttons: BTreeMap<String, String>,
    pub leds: BTreeMap<String, LedZoneConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LedZoneConfig {
    pub mode: String,
    pub brightness: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

// --- Load / Save ---

pub fn load(path: &Path) -> Result<Config> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

pub fn save(config: &Config, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn default_config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("ratctl")
        .join("config.toml")
}

// --- Conversion: DeviceProfile -> ProfileConfig ---

pub fn profile_to_config(profile: &DeviceProfile, desc: &DeviceDescriptor) -> ProfileConfig {
    let mut dpi = BTreeMap::new();
    for (i, &val) in profile.settings.dpi_presets.iter().enumerate() {
        dpi.insert(format!("preset_{}", i + 1), val);
    }

    let mut buttons = BTreeMap::new();
    for (i, binding) in profile.buttons.iter().enumerate() {
        let name = desc
            .button_slots
            .get(i)
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| format!("button_{}", i));
        buttons.insert(name, format_binding(binding));
    }

    let mut leds = BTreeMap::new();
    for (i, led) in profile.leds.iter().enumerate() {
        let zone_name = desc
            .led_names
            .get(i)
            .copied()
            .unwrap_or("unknown")
            .to_string();
        leds.insert(zone_name, led_to_config(led));
    }

    ProfileConfig {
        polling_rate: profile.settings.polling_rate,
        debounce_ms: profile.settings.debounce_ms,
        angle_snapping: profile.settings.angle_snapping,
        dpi,
        buttons,
        leds,
    }
}

pub fn config_to_profile(config: &ProfileConfig, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
    // Parse buttons in slot order
    let mut buttons = Vec::with_capacity(desc.button_slots.len());
    for slot in desc.button_slots {
        let name = slot.name();
        let binding_str = config
            .buttons
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Missing button '{}' in config", name))?;
        buttons.push(
            parse_binding(binding_str).with_context(|| format!("button '{}'", name))?,
        );
    }

    // Parse DPI presets
    let mut dpi_presets = Vec::with_capacity(desc.num_dpi_presets);
    for i in 0..desc.num_dpi_presets {
        let key = format!("preset_{}", i + 1);
        let &dpi = config
            .dpi
            .get(&key)
            .ok_or_else(|| anyhow::anyhow!("Missing DPI '{}' in config", key))?;
        if dpi < desc.dpi_min || dpi > desc.dpi_max {
            bail!(
                "DPI {} value {} out of range ({}-{})",
                key,
                dpi,
                desc.dpi_min,
                desc.dpi_max
            );
        }
        dpi_presets.push(dpi);
    }

    // Validate polling rate
    if !POLLING_RATES.contains(&config.polling_rate) {
        bail!(
            "Invalid polling rate: {}Hz (valid: {})",
            config.polling_rate,
            POLLING_RATES
                .map(|r| format!("{}Hz", r))
                .join(", ")
        );
    }

    // Validate debounce
    if !DEBOUNCE_TIMES.contains(&config.debounce_ms) {
        bail!(
            "Invalid debounce: {}ms (valid: {})",
            config.debounce_ms,
            DEBOUNCE_TIMES
                .map(|d| format!("{}ms", d))
                .join(", ")
        );
    }

    let settings = Settings {
        dpi_presets,
        polling_rate: config.polling_rate,
        debounce_ms: config.debounce_ms,
        angle_snapping: config.angle_snapping,
    };

    // Parse LEDs
    let mut leds = Vec::with_capacity(desc.num_leds);
    for &zone_name in desc.led_names.iter() {
        let zone_cfg = config
            .leds
            .get(zone_name)
            .ok_or_else(|| anyhow::anyhow!("Missing LED zone '{}' in config", zone_name))?;
        let led = config_to_led(zone_cfg, zone_name)?;
        if led.brightness > desc.brightness_max {
            bail!(
                "LED zone '{}' brightness {} exceeds max ({})",
                zone_name,
                led.brightness,
                desc.brightness_max
            );
        }
        leds.push(led);
    }

    Ok(DeviceProfile {
        buttons,
        settings,
        leds,
    })
}

// --- Button binding serialization ---

pub fn format_binding(binding: &ButtonBinding) -> String {
    if binding.kind == BindingKind::Mouse {
        ButtonCode::from_u8(binding.action_code)
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| format!("mouse:{:#04x}", binding.action_code))
    } else {
        types::hid_keycode_to_name(binding.action_code)
            .map(|name| format!("key:{}", name))
            .unwrap_or_else(|| format!("key:{:#04x}", binding.action_code))
    }
}

pub fn parse_binding(s: &str) -> Result<ButtonBinding> {
    if let Some(key_name) = s.strip_prefix("key:") {
        let keycode = types::name_to_hid_keycode(key_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown key name: '{}'", key_name))?;
        Ok(ButtonBinding::keyboard_key(keycode))
    } else if let Some(code) = ButtonCode::from_name(s) {
        Ok(ButtonBinding::mouse_action(code))
    } else {
        bail!(
            "Unknown binding '{}'. Use a mouse action name (left_click, back, disabled, etc.) \
             or 'key:<name>' for keyboard keys.",
            s
        );
    }
}

fn led_to_config(led: &LedConfig) -> LedZoneConfig {
    let color = if led.r != 0 || led.g != 0 || led.b != 0 {
        Some(format!("#{:02x}{:02x}{:02x}", led.r, led.g, led.b))
    } else {
        None
    };
    LedZoneConfig {
        mode: led.mode.name().to_string(),
        brightness: led.brightness,
        color,
    }
}

fn config_to_led(config: &LedZoneConfig, zone_name: &str) -> Result<LedConfig> {
    let mode = LedMode::from_name(&config.mode).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown LED mode '{}' for zone '{}'",
            config.mode,
            zone_name
        )
    })?;

    let (r, g, b) = if let Some(ref hex) = config.color {
        parse_hex_color(hex)
            .with_context(|| format!("Invalid color '{}' for LED zone '{}'", hex, zone_name))?
    } else {
        (0, 0, 0)
    };

    Ok(LedConfig {
        mode,
        brightness: config.brightness,
        r,
        g,
        b,
    })
}

fn parse_hex_color(s: &str) -> Result<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        bail!("Color must be 6 hex digits (e.g. #ff0000), got '{}'", s);
    }
    let r = u8::from_str_radix(&s[0..2], 16)?;
    let g = u8::from_str_radix(&s[2..4], 16)?;
    let b = u8::from_str_radix(&s[4..6], 16)?;
    Ok((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_parse_mouse_binding() {
        let binding = ButtonBinding::mouse_action(ButtonCode::LeftClick);
        let s = format_binding(&binding);
        assert_eq!(s, "left_click");
        let back = parse_binding(&s).unwrap();
        assert_eq!(back, binding);
    }

    #[test]
    fn test_format_parse_keyboard_binding() {
        let binding = ButtonBinding::keyboard_key(0x06); // 'c'
        let s = format_binding(&binding);
        assert_eq!(s, "key:c");
        let back = parse_binding(&s).unwrap();
        assert_eq!(back, binding);
    }

    #[test]
    fn test_format_parse_disabled() {
        let binding = ButtonBinding::disabled();
        let s = format_binding(&binding);
        assert_eq!(s, "disabled");
        let back = parse_binding(&s).unwrap();
        assert_eq!(back.action_code, 0xFF);
        assert_eq!(back.kind, BindingKind::Mouse);
    }

    #[test]
    fn test_hex_color_roundtrip() {
        let (r, g, b) = parse_hex_color("#ff8800").unwrap();
        assert_eq!((r, g, b), (0xFF, 0x88, 0x00));
    }

    #[test]
    fn test_hex_color_no_hash() {
        let (r, g, b) = parse_hex_color("aabbcc").unwrap();
        assert_eq!((r, g, b), (0xAA, 0xBB, 0xCC));
    }
}
