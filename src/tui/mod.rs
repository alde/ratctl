mod input;
mod views;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use evdev::Device as EvdevDevice;
use ratatui::prelude::*;
use std::io::{self, stdout};
use std::time::Duration;

use crate::protocols::MouseProtocol;
use crate::devices::DeviceDescriptor;
use crate::types::DeviceProfile;

/// Which tab is currently selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Buttons,
    Dpi,
    Leds,
    Settings,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Buttons, Tab::Dpi, Tab::Leds, Tab::Settings];

    fn name(&self) -> &'static str {
        match self {
            Tab::Buttons => "Buttons",
            Tab::Dpi => "DPI",
            Tab::Leds => "LEDs",
            Tab::Settings => "Settings",
        }
    }

    fn next(&self) -> Tab {
        match self {
            Tab::Buttons => Tab::Dpi,
            Tab::Dpi => Tab::Leds,
            Tab::Leds => Tab::Settings,
            Tab::Settings => Tab::Buttons,
        }
    }

    fn prev(&self) -> Tab {
        match self {
            Tab::Buttons => Tab::Settings,
            Tab::Dpi => Tab::Buttons,
            Tab::Leds => Tab::Dpi,
            Tab::Settings => Tab::Leds,
        }
    }
}

/// TUI interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    WaitingForButton,
    EditingBinding { slot: usize },
    EditingDpi { preset: usize },
    EditingLedColor { zone: usize },
}

/// Status message shown in the footer.
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

/// All mutable TUI state.
pub struct App {
    pub desc: &'static DeviceDescriptor,
    pub active_profile: u8,
    pub profiles: Vec<DeviceProfile>,
    pub tab: Tab,
    pub mode: Mode,
    pub cursor: usize,
    pub dirty: Vec<bool>,
    pub status: Option<StatusMessage>,
    pub should_quit: bool,
    pub confirm_quit: bool,
    pub input_buf: String,
    pub binding_cursor: usize,
    pub binding_filter: Option<String>,
}

impl App {
    pub fn current_profile(&self) -> &DeviceProfile {
        &self.profiles[self.active_profile as usize]
    }

    pub fn current_profile_mut(&mut self) -> &mut DeviceProfile {
        &mut self.profiles[self.active_profile as usize]
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: false,
        });
    }

    pub fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: true,
        });
    }

    pub fn is_any_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty[self.active_profile as usize] = true;
    }

    pub fn max_cursor(&self) -> usize {
        match self.tab {
            Tab::Buttons => self.desc.button_slots.len().saturating_sub(1),
            Tab::Dpi => self.desc.num_dpi_presets.saturating_sub(1),
            Tab::Leds => self.desc.num_leds.saturating_sub(1),
            Tab::Settings => 2,
        }
    }
}

fn cleanup_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);
}

pub fn run(
    proto: &mut dyn MouseProtocol,
    desc: &'static DeviceDescriptor,
    evdev_device: Option<&mut EvdevDevice>,
    config_path: &std::path::Path,
) -> Result<()> {
    let (active, profiles) = proto.read_all_profiles(desc)?;

    let num_profiles = profiles.len();
    let mut app = App {
        desc,
        active_profile: active,
        profiles,
        tab: Tab::Buttons,
        mode: Mode::Normal,
        cursor: 0,
        dirty: vec![false; num_profiles],
        status: None,
        should_quit: false,
        confirm_quit: false,
        input_buf: String::new(),
        binding_cursor: 0,
        binding_filter: None,
    };

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_terminal();
        original_hook(info);
    }));

    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, proto, evdev_device, config_path);

    cleanup_terminal();
    let _ = std::panic::take_hook();

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    proto: &mut dyn MouseProtocol,
    mut evdev_device: Option<&mut EvdevDevice>,
    config_path: &std::path::Path,
) -> Result<()> {
    loop {
        terminal.draw(|f| views::render(f, app))?;

        if app.should_quit {
            break;
        }

        input::poll_evdev_button(app, &mut evdev_device);

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('c')
                {
                    if app.mode == Mode::WaitingForButton {
                        if let Some(evdev) = evdev_device.as_mut() {
                            crate::evdev::ungrab(evdev);
                        }
                    }
                    break;
                }

                input::handle_key(app, key_event, proto, &mut evdev_device, config_path);
            }
        }
    }
    Ok(())
}
