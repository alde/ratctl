//! ASUS ROG HID protocol — 64-byte hidraw packets.
//!
//! Packet format:
//!   [0]     Command byte (high nibble: request class)
//!   [1]     Sub-command / mode
//!   [2]     Parameter (profile index, zone index, preset index, etc.)
//!   [3]     Reserved (0x00)
//!   [4..63] Payload (little-endian values, layout depends on command)
//!
//! Request classes:
//!   0x12 — Read (profile data, LEDs, settings, buttons)
//!   0x50 — Control (set profile, save to flash)
//!   0x51 — Write (set button, set LED, set setting)
//!
//! Responses mirror the request command bytes, with payload starting at byte 4.
//! Error responses have bytes [0..1] = [0xFF, 0xAA].
//!
//! DPI encoding: raw value = (dpi - 50) / 50, transmitted as LE u16.
//! Settings register layout (from byte 4): N×DPI presets (LE u16 each),
//! then polling rate index (LE u16), debounce index (LE u16), angle snapping (LE u16).

use anyhow::{bail, Result};

use crate::device::{HidrawDevice, PACKET_SIZE};
use crate::devices::DeviceDescriptor;
use crate::types::{
    BindingKind, ButtonBinding, DeviceProfile, LedConfig, LedMode, ProfileData, Settings,
    DEBOUNCE_TIMES, POLLING_RATES,
};

// --- Command codes ---

const CMD_GET_PROFILE_DATA: [u8; 2] = [0x12, 0x00];
const CMD_GET_LED_DATA: [u8; 2] = [0x12, 0x03];
const CMD_GET_SETTINGS: [u8; 2] = [0x12, 0x04];
const CMD_GET_BUTTON_DATA: [u8; 2] = [0x12, 0x05];
const CMD_SET_PROFILE: [u8; 2] = [0x50, 0x02];
const CMD_SAVE: [u8; 2] = [0x50, 0x03];
const CMD_SET_BUTTON: [u8; 2] = [0x51, 0x21];
const CMD_SET_LED: [u8; 2] = [0x51, 0x28];
const CMD_SET_SETTING: [u8; 2] = [0x51, 0x31];

// --- Protocol functions ---

fn make_request(cmd: [u8; 2]) -> [u8; PACKET_SIZE] {
    let mut buf = [0u8; PACKET_SIZE];
    buf[0] = cmd[0];
    buf[1] = cmd[1];
    buf
}

/// Read profile data (current profile index, firmware version).
pub fn get_profile_data(dev: &mut HidrawDevice) -> Result<ProfileData> {
    let req = make_request(CMD_GET_PROFILE_DATA);
    let resp = dev.transact(&req)?;

    let current_profile = resp[6];
    let fw = format!("{}.{}.{}", resp[13], resp[12], resp[11]);

    Ok(ProfileData {
        current_profile,
        firmware_version: fw,
    })
}

/// Read all button bindings for the current profile.
///
/// Response layout from byte 4: pairs of (action_code, action_type) for each button slot.
/// Devices with multiple button groups (e.g. ASUS side buttons) are read by iterating
/// over groups specified in `desc.button_group_sizes`.
pub fn get_button_data(dev: &mut HidrawDevice, desc: &DeviceDescriptor) -> Result<Vec<ButtonBinding>> {
    let groups = if desc.button_group_sizes.is_empty() {
        // Single group: all buttons in group 0
        vec![desc.button_slots.len()]
    } else {
        desc.button_group_sizes.to_vec()
    };

    let mut buttons = Vec::with_capacity(desc.button_slots.len());
    for (group_idx, &group_size) in groups.iter().enumerate() {
        let mut req = make_request(CMD_GET_BUTTON_DATA);
        req[2] = group_idx as u8;
        let resp = dev.transact(&req)?;

        for i in 0..group_size {
            let offset = 4 + i * 2;
            let kind = match resp[offset + 1] {
                0 => BindingKind::Keyboard,
                _ => BindingKind::Mouse,
            };
            buttons.push(ButtonBinding {
                action_code: resp[offset],
                kind,
            });
        }
    }
    Ok(buttons)
}

/// Read DPI, polling rate, debounce, and angle snapping for the current profile.
///
/// Response layout from byte 4: N×DPI (LE u16), polling rate index (LE u16),
/// debounce index (LE u16), angle snapping flag (LE u16).
pub fn get_settings(dev: &mut HidrawDevice, desc: &DeviceDescriptor) -> Result<Settings> {
    let mut req = make_request(CMD_GET_SETTINGS);
    req[2] = 0; // mode 0 = normal (not separate X/Y)
    let resp = dev.transact(&req)?;

    let ndpi = desc.num_dpi_presets;
    let mut dpi_presets = Vec::with_capacity(ndpi);
    for i in 0..ndpi {
        let offset = 4 + i * 2;
        let raw = u16::from_le_bytes([resp[offset], resp[offset + 1]]);
        dpi_presets.push(raw_to_dpi(raw));
    }

    let rate_offset = 4 + ndpi * 2;
    let rate_idx = u16::from_le_bytes([resp[rate_offset], resp[rate_offset + 1]]) as usize;
    let polling_rate = POLLING_RATES.get(rate_idx).copied().unwrap_or(1000);

    let debounce_offset = rate_offset + 2;
    let debounce_idx =
        u16::from_le_bytes([resp[debounce_offset], resp[debounce_offset + 1]]) as usize;
    let debounce_ms = DEBOUNCE_TIMES.get(debounce_idx).copied().unwrap_or(8);

    let snapping_offset = debounce_offset + 2;
    let angle_snapping =
        u16::from_le_bytes([resp[snapping_offset], resp[snapping_offset + 1]]) != 0;

    Ok(Settings {
        dpi_presets,
        polling_rate,
        debounce_ms,
        angle_snapping,
    })
}

/// Read LED configuration for one zone (0-indexed).
pub fn get_led_data(dev: &mut HidrawDevice, zone_idx: u8) -> Result<LedConfig> {
    let mut req = make_request(CMD_GET_LED_DATA);
    req[2] = zone_idx;
    let resp = dev.transact(&req)?;

    let mode = LedMode::from_u8(resp[4]).unwrap_or(LedMode::Static);
    Ok(LedConfig {
        mode,
        brightness: resp[5],
        r: resp[6],
        g: resp[7],
        b: resp[8],
    })
}

/// Read all LED zones for the device.
pub fn get_all_leds(dev: &mut HidrawDevice, desc: &DeviceDescriptor) -> Result<Vec<LedConfig>> {
    let mut leds = Vec::with_capacity(desc.num_leds);
    for i in 0..desc.num_leds {
        leds.push(get_led_data(dev, i as u8)?);
    }
    Ok(leds)
}

/// Read the complete state of the current profile.
pub fn read_current_profile(
    dev: &mut HidrawDevice,
    desc: &DeviceDescriptor,
) -> Result<DeviceProfile> {
    let buttons = get_button_data(dev, desc)?;
    let settings = get_settings(dev, desc)?;
    let leds = get_all_leds(dev, desc)?;
    Ok(DeviceProfile {
        buttons,
        settings,
        leds,
    })
}

/// Read all profiles by switching to each, reading, then switching back.
pub fn read_all_profiles(
    dev: &mut HidrawDevice,
    desc: &DeviceDescriptor,
) -> Result<(u8, Vec<DeviceProfile>)> {
    let profile_data = get_profile_data(dev)?;
    let original = profile_data.current_profile;

    let mut profiles = Vec::with_capacity(desc.num_profiles);
    for i in 0..desc.num_profiles as u8 {
        set_profile(dev, i)?;
        profiles.push(read_current_profile(dev, desc)?);
    }

    // Restore original profile
    set_profile(dev, original)?;
    Ok((original, profiles))
}

// --- Write commands ---

/// Switch to a different profile (0-indexed).
pub fn set_profile(dev: &mut HidrawDevice, index: u8) -> Result<()> {
    let mut req = make_request(CMD_SET_PROFILE);
    req[2] = index;
    dev.transact(&req)?;
    Ok(())
}

/// Save current settings to device flash.
pub fn save(dev: &mut HidrawDevice) -> Result<()> {
    let req = make_request(CMD_SAVE);
    dev.transact(&req)?;
    Ok(())
}

/// Set a single button binding.
pub fn set_button(
    dev: &mut HidrawDevice,
    desc: &DeviceDescriptor,
    slot: usize,
    binding: ButtonBinding,
) -> Result<()> {
    if slot >= desc.button_slots.len() {
        bail!("Button slot {} out of range (max {})", slot, desc.button_slots.len());
    }

    let src_code = desc.button_slots[slot] as u8;
    let mut req = make_request(CMD_SET_BUTTON);
    req[4] = src_code;
    req[5] = 0x01; // src_type = mouse button
    req[6] = binding.action_code;
    req[7] = binding.action_type_byte();
    dev.transact(&req)?;
    Ok(())
}

/// Set a DPI preset value.
pub fn set_dpi(dev: &mut HidrawDevice, desc: &DeviceDescriptor, preset: usize, dpi: u16) -> Result<()> {
    if preset >= desc.num_dpi_presets {
        bail!("DPI preset {} out of range", preset);
    }
    if dpi < desc.dpi_min || dpi > desc.dpi_max {
        bail!(
            "DPI value {} out of range ({}-{})",
            dpi,
            desc.dpi_min,
            desc.dpi_max
        );
    }
    let raw = dpi_to_raw(dpi);
    let le_bytes = raw.to_le_bytes();
    let mut req = make_request(CMD_SET_SETTING);
    req[2] = preset as u8;
    req[4] = le_bytes[0];
    req[5] = le_bytes[1];
    dev.transact(&req)?;
    Ok(())
}

/// Set polling rate by actual Hz value.
pub fn set_polling_rate(dev: &mut HidrawDevice, desc: &DeviceDescriptor, rate_hz: u16) -> Result<()> {
    let idx = POLLING_RATES
        .iter()
        .position(|&r| r == rate_hz)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid polling rate: {}Hz (valid: {})",
                rate_hz,
                POLLING_RATES.map(|r| format!("{}Hz", r)).join(", ")
            )
        })?;
    let mut req = make_request(CMD_SET_SETTING);
    req[2] = desc.num_dpi_presets as u8; // field index for polling rate
    req[4] = idx as u8;
    dev.transact(&req)?;
    Ok(())
}

/// Set debounce time in ms.
pub fn set_debounce(dev: &mut HidrawDevice, desc: &DeviceDescriptor, ms: u8) -> Result<()> {
    let idx = DEBOUNCE_TIMES
        .iter()
        .position(|&d| d == ms)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid debounce time: {}ms (valid: {})",
                ms,
                DEBOUNCE_TIMES.map(|d| format!("{}ms", d)).join(", ")
            )
        })?;
    let mut req = make_request(CMD_SET_SETTING);
    req[2] = (desc.num_dpi_presets + 1) as u8; // field index for debounce
    req[4] = idx as u8;
    dev.transact(&req)?;
    Ok(())
}

/// Set angle snapping on/off.
pub fn set_angle_snapping(dev: &mut HidrawDevice, desc: &DeviceDescriptor, enabled: bool) -> Result<()> {
    let mut req = make_request(CMD_SET_SETTING);
    req[2] = (desc.num_dpi_presets + 2) as u8; // field index for snapping
    req[4] = if enabled { 1 } else { 0 };
    dev.transact(&req)?;
    Ok(())
}

/// Set LED for one zone (0-indexed).
pub fn set_led(dev: &mut HidrawDevice, zone_idx: u8, config: &LedConfig) -> Result<()> {
    let mut req = make_request(CMD_SET_LED);
    req[2] = zone_idx;
    req[4] = config.mode as u8;
    req[5] = config.brightness;
    req[6] = config.r;
    req[7] = config.g;
    req[8] = config.b;
    dev.transact(&req)?;
    Ok(())
}

/// Apply a complete profile to the device (current profile).
pub fn apply_profile(
    dev: &mut HidrawDevice,
    desc: &DeviceDescriptor,
    profile: &DeviceProfile,
) -> Result<()> {
    for (slot, binding) in profile.buttons.iter().enumerate() {
        set_button(dev, desc, slot, *binding)
            .with_context(|| format!("Failed writing button slot {}", slot))?;
    }

    for (i, &dpi) in profile.settings.dpi_presets.iter().enumerate() {
        set_dpi(dev, desc, i, dpi)
            .with_context(|| format!("Failed writing DPI preset {}", i + 1))?;
    }

    set_polling_rate(dev, desc, profile.settings.polling_rate)
        .context("Failed writing polling rate")?;
    set_debounce(dev, desc, profile.settings.debounce_ms)
        .context("Failed writing debounce")?;
    set_angle_snapping(dev, desc, profile.settings.angle_snapping)
        .context("Failed writing angle snapping")?;

    for (i, led) in profile.leds.iter().enumerate() {
        set_led(dev, i as u8, led)
            .with_context(|| format!("Failed writing LED zone {}", i))?;
    }

    Ok(())
}

// --- DPI conversion helpers ---

/// Convert raw protocol value to DPI. Uses u32 to avoid overflow.
fn raw_to_dpi(raw: u16) -> u16 {
    let dpi = raw as u32 * 50 + 50;
    dpi.min(u16::MAX as u32) as u16
}

fn dpi_to_raw(dpi: u16) -> u16 {
    (dpi.saturating_sub(50)) / 50
}

// bring context trait into scope for apply_profile
use anyhow::Context;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpi_conversion_roundtrip() {
        for dpi in (100..=19000).step_by(50) {
            let raw = dpi_to_raw(dpi);
            let back = raw_to_dpi(raw);
            assert_eq!(back, dpi, "DPI roundtrip failed for {}", dpi);
        }
    }

    #[test]
    fn test_raw_to_dpi_no_overflow() {
        // raw=1311 would overflow u16 if done naively (1311*50=65550 > 65535)
        let dpi = raw_to_dpi(1311);
        assert_eq!(dpi, 65535); // clamped
        let dpi = raw_to_dpi(u16::MAX);
        assert_eq!(dpi, 65535); // clamped
    }
}
