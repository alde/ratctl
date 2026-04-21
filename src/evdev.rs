use anyhow::{bail, Result};
use evdev::{Device, EventSummary, KeyCode};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::os::fd::AsFd;
use std::path::PathBuf;

use crate::devices::DeviceDescriptor;
use crate::types::ButtonCode;

/// Find the evdev device matching the given descriptor (the one with BTN_LEFT).
pub fn find_mouse_evdev(desc: &DeviceDescriptor) -> Result<(PathBuf, Device)> {
    for (path, device) in evdev::enumerate() {
        let id = device.input_id();
        if !desc.matches(id.vendor(), id.product()) {
            continue;
        }
        // The mouse interface supports BTN_LEFT; the keyboard interface doesn't
        if let Some(keys) = device.supported_keys() {
            if keys.contains(KeyCode::BTN_LEFT) {
                return Ok((path, device));
            }
        }
    }
    bail!(
        "No {} evdev device found. \
         Check the mouse is connected and you have permissions on /dev/input/event*",
        desc.name
    );
}

/// Grab the evdev device exclusively so button presses don't propagate.
pub fn grab(device: &mut Device) -> Result<()> {
    device.grab().map_err(|e| {
        anyhow::anyhow!(
            "Failed to grab mouse device (EVIOCGRAB). \
             Another program may have it grabbed. Error: {}",
            e
        )
    })
}

/// Release the evdev device grab.
pub fn ungrab(device: &mut Device) {
    let _ = device.ungrab();
}

/// Non-blocking poll for a mouse button press. Returns `None` if no button
/// was pressed within the timeout. Call this from the TUI event loop so that
/// keyboard events can still be processed between polls.
pub fn poll_button_press(device: &mut Device, timeout_ms: i32) -> Result<Option<ButtonCode>> {
    let mut pollfd = [PollFd::new(device.as_fd(), PollFlags::POLLIN)];
    let ready = poll(&mut pollfd, PollTimeout::try_from(timeout_ms).unwrap())
        .map_err(|e| anyhow::anyhow!("evdev poll failed: {}", e))?;

    if ready == 0 {
        return Ok(None);
    }

    for event in device.fetch_events()? {
        if let EventSummary::Key(_, key, value) = event.destructure() {
            // value 1 = press, 0 = release, 2 = repeat
            if value == 1 {
                if let Some(button) = evdev_key_to_asus(key) {
                    return Ok(Some(button));
                }
            }
        }
    }
    Ok(None)
}

/// Map a Linux evdev key code to the corresponding ASUS button code.
fn evdev_key_to_asus(key: KeyCode) -> Option<ButtonCode> {
    match key {
        KeyCode::BTN_LEFT => Some(ButtonCode::LeftClick),
        KeyCode::BTN_RIGHT => Some(ButtonCode::RightClick),
        KeyCode::BTN_MIDDLE => Some(ButtonCode::MiddleClick),
        KeyCode::BTN_SIDE => Some(ButtonCode::Back),
        KeyCode::BTN_EXTRA => Some(ButtonCode::Forward),
        // The Spatha X thumb grid buttons typically appear as BTN_FORWARD,
        // BTN_BACK, BTN_TASK, or higher numbered BTN codes.
        // These mappings may need adjustment after testing on real hardware.
        KeyCode::BTN_FORWARD => Some(ButtonCode::SideA),
        KeyCode::BTN_BACK => Some(ButtonCode::SideB),
        KeyCode::BTN_TASK => Some(ButtonCode::SideC),
        _ => {
            let code = key.0;
            if (0x110..=0x11F).contains(&code) {
                eprintln!(
                    "Unknown evdev button code: {:#06x}. \
                     Please report this so we can add it.",
                    code
                );
            }
            None
        }
    }
}

/// Find the button slot index for a given button code using the device descriptor.
pub fn button_code_to_slot(desc: &DeviceDescriptor, code: ButtonCode) -> Option<usize> {
    desc.button_slots.iter().position(|c| *c == code)
}
