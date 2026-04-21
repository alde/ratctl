use crossterm::event::{KeyCode, KeyEvent};
use evdev::Device as EvdevDevice;

use crate::config::{self, Config};
use crate::protocols::MouseProtocol;
use crate::types::{DEBOUNCE_TIMES, LedMode, POLLING_RATES};

use super::views::binding_action_list;
use super::{App, Mode, Tab};

pub fn handle_key(
    app: &mut App,
    key: KeyEvent,
    proto: &mut dyn MouseProtocol,
    evdev_device: &mut Option<&mut EvdevDevice>,
    config_path: &std::path::Path,
) {
    match app.mode {
        Mode::Normal => handle_normal(app, key, proto, evdev_device, config_path),
        Mode::EditingBinding { slot } => handle_editing_binding(app, key, slot),
        Mode::EditingDpi { preset } => handle_editing_dpi(app, key, preset),
        Mode::EditingLedColor { zone } => handle_editing_led_color(app, key, zone),
        Mode::WaitingForButton => handle_waiting_for_button(app, key, evdev_device),
    }
}

fn handle_normal(
    app: &mut App,
    key: KeyEvent,
    proto: &mut dyn MouseProtocol,
    evdev_device: &mut Option<&mut EvdevDevice>,
    config_path: &std::path::Path,
) {
    // Clear confirm_quit on any key that isn't 'q'
    if key.code != KeyCode::Char('q') {
        app.confirm_quit = false;
        app.status = None;
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => {
            if app.is_any_dirty() && !app.confirm_quit {
                app.confirm_quit = true;
                app.set_status("Unsaved changes! Press 'q' again to quit or 's' to save.");
                return;
            }
            app.should_quit = true;
            return;
        }

        // Tab navigation
        KeyCode::Tab => {
            app.tab = app.tab.next();
            app.cursor = 0;
        }
        KeyCode::BackTab => {
            app.tab = app.tab.prev();
            app.cursor = 0;
        }

        // Profile switching
        KeyCode::Char(c @ '1'..='9') => {
            let idx = (c as u8) - b'1';
            if (idx as usize) < app.desc.num_profiles {
                app.active_profile = idx;
                app.cursor = 0;
                app.set_status(format!("Switched to profile {}", idx + 1));
            }
        }

        // Cursor movement
        KeyCode::Up | KeyCode::Char('k') => {
            if app.cursor > 0 {
                app.cursor -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.cursor < app.max_cursor() {
                app.cursor += 1;
            }
        }

        // Enter / edit
        KeyCode::Enter => match app.tab {
            Tab::Buttons => {
                app.mode = Mode::EditingBinding { slot: app.cursor };
                app.binding_cursor = 0;
            }
            Tab::Dpi => {
                app.mode = Mode::EditingDpi {
                    preset: app.cursor,
                };
                app.input_buf = app.current_profile().settings.dpi_presets[app.cursor].to_string();
            }
            Tab::Leds => {
                // Toggle through LED modes on Enter
                cycle_led_mode(app, app.cursor, true);
            }
            Tab::Settings => match app.cursor {
                0 => cycle_polling_rate(app, true),
                1 => cycle_debounce(app, true),
                2 => toggle_angle_snapping(app),
                _ => {}
            },
        },

        // Left/right for inline value adjustment
        KeyCode::Left | KeyCode::Char(',') => match app.tab {
            Tab::Dpi => adjust_dpi(app, app.cursor, -50),
            Tab::Settings if app.cursor == 0 => cycle_polling_rate(app, false),
            Tab::Settings if app.cursor == 1 => cycle_debounce(app, false),
            Tab::Leds => cycle_led_mode(app, app.cursor, false),
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('.') => match app.tab {
            Tab::Dpi => adjust_dpi(app, app.cursor, 50),
            Tab::Settings if app.cursor == 0 => cycle_polling_rate(app, true),
            Tab::Settings if app.cursor == 1 => cycle_debounce(app, true),
            Tab::Leds => cycle_led_mode(app, app.cursor, true),
            _ => {}
        },

        // Press physical button to select
        KeyCode::Char('p') if app.tab == Tab::Buttons => {
            if let Some(evdev) = evdev_device.as_mut() {
                match crate::evdev::grab(evdev) {
                    Ok(()) => {
                        app.mode = Mode::WaitingForButton;
                        app.set_status("Press a mouse button... [Esc] cancel");
                    }
                    Err(e) => app.set_error(format!("Failed to grab mouse: {}", e)),
                }
            } else {
                app.set_error("No evdev device available for button detection");
            }
        }

        // LED-specific shortcuts
        KeyCode::Char('c') if app.tab == Tab::Leds => {
            let led = &app.current_profile().leds[app.cursor];
            app.input_buf = format!("{:02x}{:02x}{:02x}", led.r, led.g, led.b);
            app.mode = Mode::EditingLedColor { zone: app.cursor };
        }
        KeyCode::Char('b') if app.tab == Tab::Leds => {
            cycle_led_brightness(app, app.cursor);
        }

        // Save config to file
        KeyCode::Char('s') => {
            match save_config(app, config_path) {
                Ok(()) => {
                    app.dirty.fill(false);
                    app.set_status(format!("Config saved to {}", config_path.display()));
                }
                Err(e) => app.set_error(format!("Save failed: {}", e)),
            }
        }

        // Apply to device
        KeyCode::Char('a') => {
            match apply_to_device(app, proto) {
                Ok(()) => app.set_status("Applied to device and saved to flash"),
                Err(e) => app.set_error(format!("Apply failed: {}", e)),
            }
        }

        _ => {}
    }
}

fn handle_editing_binding(app: &mut App, key: KeyEvent, slot: usize) {
    let actions = binding_action_list();
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.binding_cursor > 0 {
                app.binding_cursor -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.binding_cursor < actions.len() - 1 {
                app.binding_cursor += 1;
            }
        }
        KeyCode::Enter => {
            if let Some((name, binding)) = actions.get(app.binding_cursor) {
                app.current_profile_mut().buttons[slot] = *binding;
                app.mark_dirty();
                app.mode = Mode::Normal;
                app.set_status(format!("Set button {} to {}", slot, name));
            }
        }
        _ => {}
    }
}

fn handle_editing_dpi(app: &mut App, key: KeyEvent, preset: usize) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.input_buf.clear();
        }
        KeyCode::Enter => {
            if let Ok(dpi) = app.input_buf.parse::<u16>() {
                let clamped = dpi.clamp(app.desc.dpi_min, app.desc.dpi_max);
                let step = app.desc.dpi_step;
                let rounded = (clamped / step) * step;
                let rounded = rounded.max(app.desc.dpi_min);
                app.current_profile_mut().settings.dpi_presets[preset] = rounded;
                app.mark_dirty();
                app.set_status(format!("DPI preset {} set to {}", preset + 1, rounded));
            } else {
                app.set_error("Invalid DPI value");
            }
            app.mode = Mode::Normal;
            app.input_buf.clear();
        }
        KeyCode::Backspace => {
            app.input_buf.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if app.input_buf.len() < 5 {
                app.input_buf.push(c);
            }
        }
        _ => {}
    }
}

fn handle_editing_led_color(app: &mut App, key: KeyEvent, zone: usize) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.input_buf.clear();
        }
        KeyCode::Enter => {
            if app.input_buf.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&app.input_buf[0..2], 16),
                    u8::from_str_radix(&app.input_buf[2..4], 16),
                    u8::from_str_radix(&app.input_buf[4..6], 16),
                ) {
                    let led = &mut app.current_profile_mut().leds[zone];
                    led.r = r;
                    led.g = g;
                    led.b = b;
                    app.mark_dirty();
                    app.set_status(format!("LED zone {} color set to #{}", zone, app.input_buf));
                } else {
                    app.set_error("Invalid hex color");
                }
            } else {
                app.set_error("Color must be 6 hex digits (e.g. ff8800)");
            }
            app.mode = Mode::Normal;
            app.input_buf.clear();
        }
        KeyCode::Backspace => {
            app.input_buf.pop();
        }
        KeyCode::Char(c) if c.is_ascii_hexdigit() => {
            if app.input_buf.len() < 6 {
                app.input_buf.push(c);
            }
        }
        _ => {}
    }
}

fn handle_waiting_for_button(
    app: &mut App,
    key: KeyEvent,
    evdev_device: &mut Option<&mut EvdevDevice>,
) {
    if key.code == KeyCode::Esc {
        if let Some(evdev) = evdev_device.as_mut() {
            crate::evdev::ungrab(evdev);
        }
        app.mode = Mode::Normal;
        app.set_status("Button select cancelled");
    }
}

/// Poll evdev for a button press when in WaitingForButton mode.
/// Called from the main event loop, non-blocking.
pub fn poll_evdev_button(app: &mut App, evdev_device: &mut Option<&mut EvdevDevice>) {
    if app.mode != Mode::WaitingForButton {
        return;
    }
    let Some(evdev) = evdev_device.as_mut() else {
        return;
    };
    match crate::evdev::poll_button_press(evdev, 0) {
        Ok(Some(button_code)) => {
            crate::evdev::ungrab(evdev);
            if let Some(slot) = crate::evdev::button_code_to_slot(app.desc, button_code) {
                app.cursor = slot;
                app.mode = Mode::EditingBinding { slot };
                app.binding_cursor = 0;
            } else {
                app.mode = Mode::Normal;
                app.set_error("Button not mappable to a configurable slot");
            }
        }
        Ok(None) => {} // No button pressed yet, keep waiting
        Err(e) => {
            crate::evdev::ungrab(evdev);
            app.mode = Mode::Normal;
            app.set_error(format!("evdev error: {}", e));
        }
    }
}

// --- Value adjustment helpers ---

fn adjust_dpi(app: &mut App, preset: usize, delta: i32) {
    let current = app.current_profile().settings.dpi_presets[preset] as i32;
    let min = app.desc.dpi_min as i32;
    let max = app.desc.dpi_max as i32;
    let new_val = (current + delta).clamp(min, max) as u16;
    app.current_profile_mut().settings.dpi_presets[preset] = new_val;
    app.mark_dirty();
}

fn cycle_polling_rate(app: &mut App, forward: bool) {
    let current = app.current_profile().settings.polling_rate;
    let idx = POLLING_RATES
        .iter()
        .position(|&r| r == current)
        .unwrap_or(3);
    let new_idx = if forward {
        (idx + 1) % POLLING_RATES.len()
    } else {
        (idx + POLLING_RATES.len() - 1) % POLLING_RATES.len()
    };
    app.current_profile_mut().settings.polling_rate = POLLING_RATES[new_idx];
    app.mark_dirty();
}

fn cycle_debounce(app: &mut App, forward: bool) {
    let current = app.current_profile().settings.debounce_ms;
    let idx = DEBOUNCE_TIMES
        .iter()
        .position(|&d| d == current)
        .unwrap_or(1);
    let new_idx = if forward {
        (idx + 1) % DEBOUNCE_TIMES.len()
    } else {
        (idx + DEBOUNCE_TIMES.len() - 1) % DEBOUNCE_TIMES.len()
    };
    app.current_profile_mut().settings.debounce_ms = DEBOUNCE_TIMES[new_idx];
    app.mark_dirty();
}

fn toggle_angle_snapping(app: &mut App) {
    let current = app.current_profile().settings.angle_snapping;
    app.current_profile_mut().settings.angle_snapping = !current;
    app.mark_dirty();
}

fn cycle_led_mode(app: &mut App, zone: usize, forward: bool) {
    const NUM_LED_MODES: u8 = 7;
    let current = app.current_profile().leds[zone].mode as u8;
    let new_val = if forward {
        (current + 1) % NUM_LED_MODES
    } else {
        (current + NUM_LED_MODES - 1) % NUM_LED_MODES
    };
    if let Some(mode) = LedMode::from_u8(new_val) {
        app.current_profile_mut().leds[zone].mode = mode;
        app.mark_dirty();
    }
}

fn cycle_led_brightness(app: &mut App, zone: usize) {
    let max = app.desc.brightness_max;
    let current = app.current_profile().leds[zone].brightness;
    app.current_profile_mut().leds[zone].brightness = (current + 1) % (max + 1);
    app.mark_dirty();
}

// --- Device and config operations ---

fn save_config(app: &App, config_path: &std::path::Path) -> anyhow::Result<()> {
    let mut cfg = Config {
        active_profile: Some(app.active_profile + 1),
        profile: Default::default(),
    };
    for (i, dp) in app.profiles.iter().enumerate() {
        cfg.profile
            .insert((i + 1).to_string(), config::profile_to_config(dp, app.desc));
    }
    config::save(&cfg, config_path)
}

fn apply_to_device(app: &mut App, proto: &mut dyn MouseProtocol) -> anyhow::Result<()> {
    let original_profile = app.active_profile;

    // Apply all dirty profiles, not just the active one.
    for i in 0..app.profiles.len() {
        if !app.dirty[i] {
            continue;
        }
        proto.set_profile(i as u8)?;
        proto.apply_profile(app.desc, &app.profiles[i])?;
        proto.save()?;
        app.dirty[i] = false;
    }

    // Restore the active profile on the device.
    proto.set_profile(original_profile)?;
    Ok(())
}
