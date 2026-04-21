use anyhow::{bail, Context, Result};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use crate::devices::{self, DeviceDescriptor};

pub const PACKET_SIZE: usize = 64;

/// Timeout for device responses in milliseconds.
const RECV_TIMEOUT_MS: i32 = 3000;

/// Error sentinel returned by the device when disconnected/sleeping.
const ERROR_SENTINEL: [u8; 2] = [0xFF, 0xAA];

pub struct HidrawDevice {
    file: File,
    pub path: PathBuf,
}

impl HidrawDevice {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("Failed to open {}", path.display()))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Send a 64-byte packet to the device.
    pub fn send(&mut self, buf: &[u8; PACKET_SIZE]) -> Result<()> {
        self.file
            .write_all(buf)
            .context("Failed to write to hidraw device")?;
        Ok(())
    }

    /// Read a 64-byte response from the device, with timeout.
    pub fn recv(&mut self) -> Result<[u8; PACKET_SIZE]> {
        let mut pollfd = [PollFd::new(self.file.as_fd(), PollFlags::POLLIN)];
        let ready = poll(&mut pollfd, PollTimeout::try_from(RECV_TIMEOUT_MS).unwrap())
            .context("poll() failed on hidraw device")?;
        if ready == 0 {
            bail!("Timeout waiting for device response");
        }

        let mut buf = [0u8; PACKET_SIZE];
        let n = self
            .file
            .read(&mut buf)
            .context("Failed to read from hidraw device")?;
        if n != PACKET_SIZE {
            bail!(
                "Short read from hidraw: got {} bytes, expected {}",
                n,
                PACKET_SIZE
            );
        }
        Ok(buf)
    }

    /// Send a request and read the response. Returns error if device
    /// responds with the disconnect/sleep sentinel.
    pub fn transact(&mut self, request: &[u8; PACKET_SIZE]) -> Result<[u8; PACKET_SIZE]> {
        self.send(request)?;
        let response = self.recv()?;
        if response[0] == ERROR_SENTINEL[0] && response[1] == ERROR_SENTINEL[1] {
            bail!("Device returned error (0xFF 0xAA) — mouse may be disconnected or sleeping");
        }
        Ok(response)
    }
}

/// Result of scanning for a supported device.
pub struct DetectedDevice {
    pub path: PathBuf,
    pub descriptor: &'static DeviceDescriptor,
}

/// Detect the device descriptor for a manually specified hidraw path.
pub fn detect_from_path(path: &Path) -> Result<DetectedDevice> {
    // /dev/hidrawN -> /sys/class/hidraw/hidrawN/device/uevent
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid device path: {}", path.display()))?;
    let sysfs_path = PathBuf::from("/sys/class/hidraw").join(name);
    let uevent_path = sysfs_path.join("device/uevent");
    let uevent = fs::read_to_string(&uevent_path)
        .with_context(|| format!("Failed to read uevent for {}", path.display()))?;
    if let Some((vid, pid)) = parse_hid_id(&uevent) {
        if let Some(descriptor) = devices::find_descriptor(vid, pid) {
            return Ok(DetectedDevice {
                path: path.to_path_buf(),
                descriptor,
            });
        }
        bail!(
            "Device at {} (VID {:#06x}, PID {:#06x}) is not supported",
            path.display(),
            vid,
            pid
        );
    }
    bail!("Could not read VID/PID for {}", path.display());
}

/// Scan sysfs for all supported hidraw devices on interface 0 (vendor control).
pub fn find_all_devices() -> Result<Vec<DetectedDevice>> {
    let hidraw_dir = PathBuf::from("/sys/class/hidraw");
    if !hidraw_dir.exists() {
        bail!("/sys/class/hidraw not found — is this a Linux system?");
    }

    let mut found = Vec::new();
    // Track which descriptors we've already matched to avoid duplicates
    // (a device may have multiple hidraw nodes on interface 0)
    let mut seen_names: Vec<&str> = Vec::new();

    for entry in fs::read_dir(&hidraw_dir).context("Failed to read /sys/class/hidraw")? {
        let entry = entry?;
        let name = entry.file_name();
        let uevent_path = entry.path().join("device/uevent");

        if !uevent_path.exists() {
            continue;
        }

        let uevent = fs::read_to_string(&uevent_path)
            .with_context(|| format!("Failed to read {}", uevent_path.display()))?;

        if let Some((vid, pid)) = parse_hid_id(&uevent) {
            if let Some(descriptor) = devices::find_descriptor(vid, pid) {
                if !seen_names.contains(&descriptor.name)
                    && is_interface(&entry.path(), "00")?
                {
                    seen_names.push(descriptor.name);
                    found.push(DetectedDevice {
                        path: PathBuf::from("/dev").join(name),
                        descriptor,
                    });
                }
            }
        }
    }

    Ok(found)
}

/// Scan sysfs for a single supported device. Errors if none found.
pub fn find_device() -> Result<DetectedDevice> {
    let mut devices = find_all_devices()?;
    if devices.is_empty() {
        let supported: Vec<&str> = devices::REGISTRY.iter().map(|d| d.name).collect();
        bail!(
            "No supported mouse found. Supported devices: {}\n\
             Check the mouse is connected and you have permissions on /dev/hidraw*",
            supported.join(", ")
        );
    }
    Ok(devices.remove(0))
}

/// Parse HID_ID=BBBB:VVVVVVVV:PPPPPPPP from a uevent file.
pub fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    for line in uevent.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() == 3 {
                let vid = u16::from_str_radix(parts[1].trim_start_matches('0'), 16)
                    .or_else(|_| u16::from_str_radix(parts[1], 16))
                    .ok()?;
                let pid = u16::from_str_radix(parts[2].trim_start_matches('0'), 16)
                    .or_else(|_| u16::from_str_radix(parts[2], 16))
                    .ok()?;
                return Some((vid, pid));
            }
        }
    }
    None
}

/// Check if a hidraw sysfs entry is on the given USB interface number.
pub fn is_interface(sysfs_path: &Path, expected_iface: &str) -> Result<bool> {
    let device_link = sysfs_path.join("device");
    let resolved = fs::canonicalize(&device_link)
        .with_context(|| format!("Failed to resolve {}", device_link.display()))?;

    let mut current = resolved.as_path();
    for _ in 0..10 {
        let iface_file = current.join("bInterfaceNumber");
        if iface_file.exists() {
            let iface_num = fs::read_to_string(&iface_file)?;
            return Ok(iface_num.trim() == expected_iface);
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    Ok(false)
}

/// Find a hidraw device matching a descriptor on a specific USB interface.
pub fn find_hidraw_for_device(
    desc: &DeviceDescriptor,
    interface: &str,
) -> Result<PathBuf> {
    let hidraw_dir = PathBuf::from("/sys/class/hidraw");
    for entry in fs::read_dir(&hidraw_dir).context("Failed to read /sys/class/hidraw")? {
        let entry = entry?;
        let uevent_path = entry.path().join("device/uevent");
        if !uevent_path.exists() {
            continue;
        }
        let uevent = fs::read_to_string(&uevent_path)?;
        if let Some((vid, pid)) = parse_hid_id(&uevent) {
            if desc.matches(vid, pid) && is_interface(&entry.path(), interface)? {
                return Ok(PathBuf::from("/dev").join(entry.file_name()));
            }
        }
    }
    bail!(
        "No {} hidraw device found on interface {}",
        desc.name,
        interface
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hid_id_spatha_x() {
        let uevent = "HID_ID=0003:00000B05:00001977\nHID_NAME=ASUS ROG SPATHA X\n";
        let (vid, pid) = parse_hid_id(uevent).unwrap();
        assert_eq!(vid, 0x0B05);
        assert_eq!(pid, 0x1977);
    }

    #[test]
    fn test_parse_hid_id_wireless() {
        let uevent = "HID_ID=0003:00000B05:00001979\n";
        let (vid, pid) = parse_hid_id(uevent).unwrap();
        assert_eq!(vid, 0x0B05);
        assert_eq!(pid, 0x1979);
    }

    #[test]
    fn test_parse_hid_id_missing() {
        assert!(parse_hid_id("HID_NAME=Something\n").is_none());
    }
}
