//! Razer HID protocol — 90-byte feature reports over hidraw.
//!
//! Packet format:
//!   [0]     Status (0x00=new, 0x02=success, 0x03=fail)
//!   [1]     Transaction ID
//!   [2..4]  Remaining packets (BE u16)
//!   [4]     Protocol type (0x00)
//!   [5]     Data size (payload length)
//!   [6]     Command class
//!   [7]     Command ID (bit 7: 0=set, 1=get)
//!   [8..88] Arguments (80 bytes)
//!   [88]    CRC (XOR of bytes 2..87)
//!   [89]    Reserved (0x00)

use anyhow::{bail, Result};
use nix::libc;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::devices::DeviceDescriptor;
use crate::types::{ButtonBinding, DeviceProfile, LedConfig, LedMode, Settings};

const RAZER_PACKET_SIZE: usize = 90;

// Transaction ID for mouse devices
const TRANSACTION_ID: u8 = 0x1F;

// Command classes
const CLASS_MISC: u8 = 0x00;
const CLASS_LED: u8 = 0x03;
const CLASS_DPI: u8 = 0x04;

// Command IDs (set = ID, get = ID | 0x80)
const CMD_SET_POLLING_RATE: u8 = 0x05;
const CMD_GET_POLLING_RATE: u8 = 0x85;
const CMD_SET_LED_STATE: u8 = 0x00;
const CMD_GET_LED_STATE: u8 = 0x80;
const CMD_SET_LED_RGB: u8 = 0x01;
const CMD_GET_LED_RGB: u8 = 0x81;
const CMD_SET_LED_EFFECT: u8 = 0x02;
const CMD_GET_LED_EFFECT: u8 = 0x82;
const CMD_SET_LED_BRIGHTNESS: u8 = 0x03;
const CMD_GET_LED_BRIGHTNESS: u8 = 0x83;
const CMD_SET_DPI_XY: u8 = 0x05;
const CMD_GET_DPI_XY: u8 = 0x85;

// Razer LED IDs
const LED_SCROLL: u8 = 0x01;
const LED_LOGO: u8 = 0x04;

// Razer LED effects
const EFFECT_STATIC: u8 = 0x00;
const EFFECT_BREATHING: u8 = 0x02;
const EFFECT_SPECTRUM: u8 = 0x04;

// Polling rate values
const RATE_1000HZ: u8 = 0x01;
const RATE_500HZ: u8 = 0x02;
const RATE_250HZ: u8 = 0x04;
const RATE_125HZ: u8 = 0x08;

/// Buffer size for hidraw feature reports: 1 byte report ID + 90 bytes payload.
const FEATURE_REPORT_SIZE: usize = 1 + RAZER_PACKET_SIZE;

fn hidiocsfeature(len: usize) -> libc::c_ulong {
    // _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
    let dir: libc::c_ulong = 0xC0000000; // _IOC_WRITE | _IOC_READ
    let typ: libc::c_ulong = (b'H' as libc::c_ulong) << 8;
    let nr: libc::c_ulong = 0x06;
    let size: libc::c_ulong = (len as libc::c_ulong) << 16;
    dir | size | typ | nr
}

fn hidiocgfeature(len: usize) -> libc::c_ulong {
    // _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
    let dir: libc::c_ulong = 0xC0000000; // _IOC_WRITE | _IOC_READ
    let typ: libc::c_ulong = (b'H' as libc::c_ulong) << 8;
    let nr: libc::c_ulong = 0x07;
    let size: libc::c_ulong = (len as libc::c_ulong) << 16;
    dir | size | typ | nr
}

/// Razer hidraw device handle.
pub struct RazerDevice {
    file: std::fs::File,
    pub path: PathBuf,
}

impl RazerDevice {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open {}: {}", path.display(), e))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Send a 90-byte feature report (SET_REPORT).
    /// Prepends report ID 0x00, making a 91-byte buffer for the ioctl.
    fn send_feature(&self, buf: &[u8; RAZER_PACKET_SIZE]) -> Result<()> {
        let fd = self.file.as_raw_fd();
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x00; // Report ID
        report[1..].copy_from_slice(buf);
        let ret = unsafe {
            libc::ioctl(
                fd,
                hidiocsfeature(FEATURE_REPORT_SIZE) as libc::c_ulong,
                report.as_mut_ptr(),
            )
        };
        if ret < 0 {
            bail!(
                "HIDIOCSFEATURE failed: {}",
                std::io::Error::last_os_error()
            );
        }
        Ok(())
    }

    /// Get a 90-byte feature report (GET_REPORT).
    /// Prepends report ID 0x00, strips it from the result.
    fn get_feature(&self) -> Result<[u8; RAZER_PACKET_SIZE]> {
        let fd = self.file.as_raw_fd();
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report[0] = 0x00; // Report ID to request
        let ret = unsafe {
            libc::ioctl(
                fd,
                hidiocgfeature(FEATURE_REPORT_SIZE) as libc::c_ulong,
                report.as_mut_ptr(),
            )
        };
        if ret < 0 {
            bail!(
                "HIDIOCGFEATURE failed: {}",
                std::io::Error::last_os_error()
            );
        }
        let mut result = [0u8; RAZER_PACKET_SIZE];
        result.copy_from_slice(&report[1..]); // Strip report ID
        Ok(result)
    }

    /// Send a command and read the response.
    pub fn transact(&self, request: &[u8; RAZER_PACKET_SIZE]) -> Result<[u8; RAZER_PACKET_SIZE]> {
        self.send_feature(request)?;

        // Razer devices need a short delay between set and get
        thread::sleep(Duration::from_micros(600));

        let response = self.get_feature()?;

        match response[0] {
            0x02 => Ok(response), // Success
            0x03 => bail!("Device returned error (command failed)"),
            0x04 => bail!("Device returned timeout"),
            0x05 => bail!("Device returned 'not supported'"),
            _ => {
                // Some devices return 0x00 on success for get commands
                Ok(response)
            }
        }
    }
}

fn build_packet(class: u8, cmd: u8, data_size: u8, args: &[u8]) -> [u8; RAZER_PACKET_SIZE] {
    let mut pkt = [0u8; RAZER_PACKET_SIZE];
    pkt[0] = 0x00;
    pkt[1] = TRANSACTION_ID;
    pkt[4] = 0x00;
    pkt[5] = data_size;
    pkt[6] = class;
    pkt[7] = cmd;
    let len = args.len().min(80);
    pkt[8..8 + len].copy_from_slice(&args[..len]);
    pkt[88] = pkt[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
    pkt
}

// --- DPI ---

/// Read current DPI (returns X and Y, usually identical).
pub fn get_dpi(dev: &RazerDevice) -> Result<(u16, u16)> {
    let pkt = build_packet(CLASS_DPI, CMD_GET_DPI_XY, 0x07, &[0x00; 7]);
    let resp = dev.transact(&pkt)?;
    let dpi_x = u16::from_be_bytes([resp[9], resp[10]]);
    let dpi_y = u16::from_be_bytes([resp[11], resp[12]]);
    Ok((dpi_x, dpi_y))
}

/// Set DPI (X and Y to the same value).
pub fn set_dpi(dev: &RazerDevice, dpi: u16) -> Result<()> {
    let dpi_bytes = dpi.to_be_bytes();
    let args = [0x00, dpi_bytes[0], dpi_bytes[1], dpi_bytes[0], dpi_bytes[1], 0x00, 0x00];
    let pkt = build_packet(CLASS_DPI, CMD_SET_DPI_XY, 0x07, &args);
    dev.transact(&pkt)?;
    Ok(())
}

// --- Polling rate ---

fn rate_to_byte(hz: u16) -> Option<u8> {
    match hz {
        1000 => Some(RATE_1000HZ),
        500 => Some(RATE_500HZ),
        250 => Some(RATE_250HZ),
        125 => Some(RATE_125HZ),
        _ => None,
    }
}

fn byte_to_rate(b: u8) -> u16 {
    match b {
        RATE_1000HZ => 1000,
        RATE_500HZ => 500,
        RATE_250HZ => 250,
        RATE_125HZ => 125,
        _ => 1000,
    }
}

pub fn get_polling_rate(dev: &RazerDevice) -> Result<u16> {
    let pkt = build_packet(CLASS_MISC, CMD_GET_POLLING_RATE, 0x01, &[0x00]);
    let resp = dev.transact(&pkt)?;
    Ok(byte_to_rate(resp[8]))
}

pub fn set_polling_rate(dev: &RazerDevice, hz: u16) -> Result<()> {
    let byte = rate_to_byte(hz)
        .ok_or_else(|| anyhow::anyhow!("Invalid polling rate: {}Hz (valid: 125, 250, 500, 1000)", hz))?;
    let pkt = build_packet(CLASS_MISC, CMD_SET_POLLING_RATE, 0x01, &[byte]);
    dev.transact(&pkt)?;
    Ok(())
}

// --- LEDs ---

fn led_id_from_name(name: &str) -> Option<u8> {
    match name {
        "logo" => Some(LED_LOGO),
        "scroll" => Some(LED_SCROLL),
        _ => None,
    }
}

pub fn get_led_rgb(dev: &RazerDevice, led_id: u8) -> Result<(u8, u8, u8)> {
    let pkt = build_packet(CLASS_LED, CMD_GET_LED_RGB, 0x05, &[0x00, led_id, 0, 0, 0]);
    let resp = dev.transact(&pkt)?;
    Ok((resp[10], resp[11], resp[12]))
}

pub fn set_led_rgb(dev: &RazerDevice, led_id: u8, r: u8, g: u8, b: u8) -> Result<()> {
    let pkt = build_packet(CLASS_LED, CMD_SET_LED_RGB, 0x05, &[0x00, led_id, r, g, b]);
    dev.transact(&pkt)?;
    Ok(())
}

pub fn get_led_brightness(dev: &RazerDevice, led_id: u8) -> Result<u8> {
    let pkt = build_packet(CLASS_LED, CMD_GET_LED_BRIGHTNESS, 0x03, &[0x00, led_id, 0]);
    let resp = dev.transact(&pkt)?;
    Ok(resp[10])
}

pub fn set_led_brightness(dev: &RazerDevice, led_id: u8, brightness: u8) -> Result<()> {
    let pkt = build_packet(CLASS_LED, CMD_SET_LED_BRIGHTNESS, 0x03, &[0x00, led_id, brightness]);
    dev.transact(&pkt)?;
    Ok(())
}

pub fn get_led_effect(dev: &RazerDevice, led_id: u8) -> Result<u8> {
    let pkt = build_packet(CLASS_LED, CMD_GET_LED_EFFECT, 0x03, &[0x00, led_id, 0]);
    let resp = dev.transact(&pkt)?;
    Ok(resp[10])
}

pub fn set_led_effect(dev: &RazerDevice, led_id: u8, effect: u8) -> Result<()> {
    let pkt = build_packet(CLASS_LED, CMD_SET_LED_EFFECT, 0x03, &[0x00, led_id, effect]);
    dev.transact(&pkt)?;
    Ok(())
}

fn razer_effect_to_led_mode(effect: u8) -> LedMode {
    match effect {
        EFFECT_STATIC => LedMode::Static,
        EFFECT_BREATHING => LedMode::Breathing,
        EFFECT_SPECTRUM => LedMode::Cycle,
        _ => LedMode::Static,
    }
}

fn led_mode_to_razer_effect(mode: LedMode) -> u8 {
    match mode {
        LedMode::Static => EFFECT_STATIC,
        LedMode::Breathing => EFFECT_BREATHING,
        LedMode::Cycle | LedMode::Rainbow => EFFECT_SPECTRUM,
        _ => EFFECT_STATIC,
    }
}

// --- High-level read/write for MouseProtocol ---

const RAZER_LED_IDS: [u8; 2] = [LED_LOGO, LED_SCROLL];

pub fn read_profile(dev: &RazerDevice, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
    // DPI — Razer only has one "current" DPI, no presets on the wire
    let (dpi_x, _dpi_y) = get_dpi(dev)?;
    let mut dpi_presets = vec![dpi_x];
    // Pad to the expected number of presets
    while dpi_presets.len() < desc.num_dpi_presets {
        dpi_presets.push(dpi_x);
    }

    let polling_rate = get_polling_rate(dev)?;

    // LEDs
    let mut leds = Vec::with_capacity(desc.num_leds);
    for (i, &_zone_name) in desc.led_names.iter().enumerate() {
        let led_id = RAZER_LED_IDS.get(i).copied().unwrap_or(LED_LOGO);
        let (r, g, b) = get_led_rgb(dev, led_id)?;
        let brightness = get_led_brightness(dev, led_id)?;
        let effect = get_led_effect(dev, led_id)?;
        leds.push(LedConfig {
            mode: razer_effect_to_led_mode(effect),
            brightness,
            r,
            g,
            b,
        });
    }

    // Buttons — standard buttons handled by kernel, no remapping
    let mut buttons = Vec::with_capacity(desc.button_slots.len());
    for &slot in desc.button_slots {
        buttons.push(ButtonBinding::mouse_action(slot));
    }

    Ok(DeviceProfile {
        buttons,
        settings: Settings {
            dpi_presets,
            polling_rate,
            debounce_ms: 0, // Not configurable on Razer
            angle_snapping: false,
        },
        leds,
    })
}

pub fn apply_profile(dev: &RazerDevice, desc: &DeviceDescriptor, profile: &DeviceProfile) -> Result<()> {
    // DPI — apply first preset as the active DPI
    if let Some(&dpi) = profile.settings.dpi_presets.first() {
        if dpi < desc.dpi_min || dpi > desc.dpi_max {
            bail!(
                "DPI value {} out of range ({}-{})",
                dpi,
                desc.dpi_min,
                desc.dpi_max
            );
        }
        set_dpi(dev, dpi)?;
    }

    set_polling_rate(dev, profile.settings.polling_rate)?;

    // LEDs
    for (i, led) in profile.leds.iter().enumerate() {
        let led_id = RAZER_LED_IDS.get(i).copied().unwrap_or(LED_LOGO);
        set_led_rgb(dev, led_id, led.r, led.g, led.b)?;
        set_led_brightness(dev, led_id, led.brightness)?;
        set_led_effect(dev, led_id, led_mode_to_razer_effect(led.mode))?;
    }

    Ok(())
}

// --- Device discovery ---

/// Find the Razer hidraw device on interface 0 (feature reports).
pub fn find_razer_hidraw(desc: &DeviceDescriptor) -> Result<PathBuf> {
    crate::device::find_hidraw_for_device(desc, "00")
}
