//! Corsair NXP protocol — 64-byte interrupt reports over hidraw.
//!
//! Packet format (Scimitar RGB Elite, PID 0x1B8B):
//!   [0]     Command (0x07=SET, 0x0E=GET)
//!   [1]     Field
//!   [2]     Subcommand
//!   [3]     Profile (0x00=current, 0x01=HW storage)
//!   [4..64] Payload

use anyhow::{bail, Result};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::os::fd::AsFd;

use crate::devices::DeviceDescriptor;
use crate::types::{ButtonBinding, DeviceProfile, LedConfig, LedMode, Settings};

const PACKET_SIZE: usize = 64;
const RECV_TIMEOUT_MS: i32 = 3000;

// Commands
const CMD_SET: u8 = 0x07;
const CMD_GET: u8 = 0x0E;

// Fields
const FIELD_IDENT: u8 = 0x01;
const FIELD_SPECIAL: u8 = 0x04;
const FIELD_POLLRATE: u8 = 0x0A;
const FIELD_MOUSE: u8 = 0x13;
const FIELD_M_COLOR: u8 = 0x22;

// Special subcommands
const MODE_SOFTWARE: u8 = 0x02;
const MODE_HARDWARE: u8 = 0x01;

// Mouse DPI subcommands
const MOUSE_DPI_STAGE: u8 = 0xD0; // OR'd with stage index 0-5
const MOUSE_DPI_SELECT: u8 = 0x02;
const MOUSE_DPI_ENABLED: u8 = 0x05;

// Polling rate values (interval in ms)
const RATE_1MS: u8 = 1; // 1000Hz
const RATE_2MS: u8 = 2; // 500Hz
const RATE_4MS: u8 = 4; // 250Hz
const RATE_8MS: u8 = 8; // 125Hz

pub struct CorsairDevice {
    file: std::fs::File,
    pub path: PathBuf,
}

impl CorsairDevice {
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

    fn send(&mut self, buf: &[u8; PACKET_SIZE]) -> Result<()> {
        self.file
            .write_all(buf)
            .map_err(|e| anyhow::anyhow!("Write failed: {}", e))?;
        Ok(())
    }

    fn recv(&mut self) -> Result<[u8; PACKET_SIZE]> {
        let mut pollfd = [PollFd::new(self.file.as_fd(), PollFlags::POLLIN)];
        let ready = poll(&mut pollfd, PollTimeout::try_from(RECV_TIMEOUT_MS).unwrap())
            .map_err(|e| anyhow::anyhow!("poll failed: {}", e))?;
        if ready == 0 {
            bail!("Timeout waiting for Corsair device response");
        }
        let mut buf = [0u8; PACKET_SIZE];
        let n = self.file.read(&mut buf)?;
        if n < PACKET_SIZE {
            bail!("Short read: {} bytes", n);
        }
        Ok(buf)
    }

    pub fn transact(&mut self, request: &[u8; PACKET_SIZE]) -> Result<[u8; PACKET_SIZE]> {
        self.send(request)?;
        thread::sleep(Duration::from_millis(10));
        self.recv()
    }

    /// Switch device to software control mode. Required before sending commands.
    pub fn enter_software_mode(&mut self) -> Result<()> {
        let mut pkt = [0u8; PACKET_SIZE];
        pkt[0] = CMD_SET;
        pkt[1] = FIELD_SPECIAL;
        pkt[2] = MODE_SOFTWARE;
        self.send(&pkt)?;
        thread::sleep(Duration::from_millis(50));
        Ok(())
    }

    /// Switch device back to hardware mode.
    pub fn enter_hardware_mode(&mut self) -> Result<()> {
        let mut pkt = [0u8; PACKET_SIZE];
        pkt[0] = CMD_SET;
        pkt[1] = FIELD_SPECIAL;
        pkt[2] = MODE_HARDWARE;
        self.send(&pkt)?;
        Ok(())
    }
}

/// RAII guard that restores hardware mode on drop.
/// Use this around any block that enters software mode.
pub struct SoftwareModeGuard<'a> {
    dev: &'a mut CorsairDevice,
}

impl<'a> SoftwareModeGuard<'a> {
    pub fn enter(dev: &'a mut CorsairDevice) -> Result<Self> {
        dev.enter_software_mode()?;
        Ok(Self { dev })
    }

    /// Access the device for commands while in software mode.
    pub fn device(&mut self) -> &mut CorsairDevice {
        self.dev
    }
}

impl Drop for SoftwareModeGuard<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.dev.enter_hardware_mode() {
            eprintln!(
                "Warning: failed to restore hardware mode ({}). \
                 Mouse may need to be unplugged and reconnected.",
                e
            );
        }
    }
}

fn make_packet(cmd: u8, field: u8, sub: u8, profile: u8) -> [u8; PACKET_SIZE] {
    let mut pkt = [0u8; PACKET_SIZE];
    pkt[0] = cmd;
    pkt[1] = field;
    pkt[2] = sub;
    pkt[3] = profile;
    pkt
}

// --- DPI ---

pub fn get_dpi_stage(dev: &mut CorsairDevice, stage: u8) -> Result<(u16, u16)> {
    let pkt = make_packet(CMD_GET, FIELD_MOUSE, MOUSE_DPI_STAGE | stage, 0x01);
    let resp = dev.transact(&pkt)?;
    let dpi_x = u16::from_be_bytes([resp[5], resp[6]]);
    let dpi_y = u16::from_be_bytes([resp[7], resp[8]]);
    Ok((dpi_x, dpi_y))
}

pub fn set_dpi_stage(dev: &mut CorsairDevice, stage: u8, dpi_x: u16, dpi_y: u16) -> Result<()> {
    let mut pkt = make_packet(CMD_SET, FIELD_MOUSE, MOUSE_DPI_STAGE | stage, 0x00);
    let independent = if dpi_x != dpi_y { 0x01 } else { 0x00 };
    pkt[4] = independent;
    let x_bytes = dpi_x.to_be_bytes();
    let y_bytes = dpi_y.to_be_bytes();
    pkt[5] = x_bytes[0];
    pkt[6] = x_bytes[1];
    pkt[7] = y_bytes[0];
    pkt[8] = y_bytes[1];
    dev.send(&pkt)?;
    Ok(())
}

pub fn select_dpi_stage(dev: &mut CorsairDevice, stage: u8) -> Result<()> {
    let mut pkt = make_packet(CMD_SET, FIELD_MOUSE, MOUSE_DPI_SELECT, 0x00);
    pkt[4] = stage;
    dev.send(&pkt)?;
    Ok(())
}

// --- Polling rate ---

fn hz_to_interval(hz: u16) -> Option<u8> {
    match hz {
        1000 => Some(RATE_1MS),
        500 => Some(RATE_2MS),
        250 => Some(RATE_4MS),
        125 => Some(RATE_8MS),
        _ => None,
    }
}

fn interval_to_hz(interval: u8) -> u16 {
    match interval {
        RATE_1MS => 1000,
        RATE_2MS => 500,
        RATE_4MS => 250,
        RATE_8MS => 125,
        _ => 1000,
    }
}

pub fn get_polling_rate(dev: &mut CorsairDevice) -> Result<u16> {
    let pkt = make_packet(CMD_GET, FIELD_POLLRATE, 0x00, 0x00);
    let resp = dev.transact(&pkt)?;
    Ok(interval_to_hz(resp[4]))
}

pub fn set_polling_rate(dev: &mut CorsairDevice, hz: u16) -> Result<()> {
    let interval = hz_to_interval(hz)
        .ok_or_else(|| anyhow::anyhow!("Invalid polling rate: {}Hz (valid: 125, 250, 500, 1000)", hz))?;
    let mut pkt = make_packet(CMD_SET, FIELD_POLLRATE, 0x00, 0x00);
    pkt[4] = interval;
    dev.send(&pkt)?;
    // Device may disconnect/reconnect after polling rate change
    thread::sleep(Duration::from_millis(200));
    Ok(())
}

// --- LEDs ---

/// Set all LED zones at once.
/// zones: slice of (zone_id_1based, r, g, b)
pub fn set_led_colors(dev: &mut CorsairDevice, zones: &[(u8, u8, u8, u8)]) -> Result<()> {
    let mut pkt = make_packet(CMD_SET, FIELD_M_COLOR, zones.len() as u8, 0x01);
    for (i, &(zone_id, r, g, b)) in zones.iter().enumerate() {
        let offset = 4 + i * 4;
        if offset + 3 >= PACKET_SIZE {
            break;
        }
        pkt[offset] = zone_id;
        pkt[offset + 1] = r;
        pkt[offset + 2] = g;
        pkt[offset + 3] = b;
    }
    dev.send(&pkt)?;
    Ok(())
}

// --- High-level read/write ---

const CORSAIR_LED_ZONE_IDS: &[u8] = &[0x01, 0x02, 0x03, 0x04]; // logo, scroll, side_panel, dpi

pub fn read_profile(dev: &mut CorsairDevice, desc: &DeviceDescriptor) -> Result<DeviceProfile> {
    let mut guard = SoftwareModeGuard::enter(dev)?;

    let mut dpi_presets = Vec::with_capacity(desc.num_dpi_presets);
    for i in 0..desc.num_dpi_presets {
        let (dpi_x, _dpi_y) = get_dpi_stage(guard.device(), i as u8)?;
        dpi_presets.push(dpi_x);
    }

    let polling_rate = get_polling_rate(guard.device())?;

    // LEDs — NXP protocol doesn't expose a clean GET for SW-mode RGB.
    let mut leds = Vec::with_capacity(desc.num_leds);
    for _ in 0..desc.num_leds {
        leds.push(LedConfig {
            mode: LedMode::Static,
            brightness: 255,
            r: 0,
            g: 0,
            b: 0,
        });
    }

    let mut buttons = Vec::with_capacity(desc.button_slots.len());
    for &slot in desc.button_slots {
        buttons.push(ButtonBinding::mouse_action(slot));
    }

    // guard drops here -> enter_hardware_mode() called automatically
    drop(guard);

    Ok(DeviceProfile {
        buttons,
        settings: Settings {
            dpi_presets,
            polling_rate,
            debounce_ms: 0,
            angle_snapping: false,
        },
        leds,
    })
}

pub fn apply_profile(
    dev: &mut CorsairDevice,
    desc: &DeviceDescriptor,
    profile: &DeviceProfile,
) -> Result<()> {
    let mut guard = SoftwareModeGuard::enter(dev)?;

    // DPI stages
    for (i, &dpi) in profile.settings.dpi_presets.iter().enumerate() {
        if dpi < desc.dpi_min || dpi > desc.dpi_max {
            bail!(
                "DPI preset {} value {} out of range ({}-{})",
                i + 1,
                dpi,
                desc.dpi_min,
                desc.dpi_max
            );
        }
        set_dpi_stage(guard.device(), i as u8, dpi, dpi)?;
    }
    select_dpi_stage(guard.device(), 0)?;

    // LEDs (before polling rate, which may cause disconnect)
    let zones: Vec<(u8, u8, u8, u8)> = profile
        .leds
        .iter()
        .enumerate()
        .map(|(i, led)| {
            let zone_id = CORSAIR_LED_ZONE_IDS.get(i).copied().unwrap_or((i + 1) as u8);
            (zone_id, led.r, led.g, led.b)
        })
        .collect();
    if !zones.is_empty() {
        set_led_colors(guard.device(), &zones)?;
    }

    // Polling rate LAST — device may disconnect and re-enumerate
    set_polling_rate(guard.device(), profile.settings.polling_rate)?;

    // guard drops -> enter_hardware_mode()
    Ok(())
}

// --- Device discovery ---

pub fn find_corsair_hidraw(desc: &DeviceDescriptor) -> Result<PathBuf> {
    crate::device::find_hidraw_for_device(desc, "02")
}
