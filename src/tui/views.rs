use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs};

use crate::config;
use crate::types::ButtonCode;

use super::{App, Mode, Tab};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_content(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);

    match app.mode {
        Mode::EditingBinding { slot } => render_binding_popup(f, app, slot),
        Mode::WaitingForButton => render_waiting_popup(f),
        Mode::EditingLedColor { .. } => {} // inline in LED row
        _ => {}
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(30)])
        .split(area);

    let tab_titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| {
            if *t == app.tab {
                Line::from(t.name()).bold()
            } else {
                Line::from(t.name())
            }
        })
        .collect();
    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.desc.name),
        )
        .highlight_style(Style::default().fg(Color::Yellow).bold())
        .select(selected);
    f.render_widget(tabs, header_chunks[0]);

    let profile_text: Vec<Span> = (0..app.desc.num_profiles as u8)
        .map(|i| {
            let num = format!(" {} ", i + 1);
            if i == app.active_profile {
                Span::styled(
                    num,
                    Style::default().fg(Color::Black).bg(Color::Yellow).bold(),
                )
            } else {
                Span::styled(num, Style::default().fg(Color::DarkGray))
            }
        })
        .collect();
    let profile_line = Line::from(profile_text);
    let profile_widget = Paragraph::new(profile_line)
        .block(Block::default().borders(Borders::ALL).title("Profile"));
    f.render_widget(profile_widget, header_chunks[1]);
}

fn render_content(f: &mut Frame, app: &App, area: Rect) {
    match app.tab {
        Tab::Buttons => render_buttons(f, app, area),
        Tab::Dpi => render_dpi(f, app, area),
        Tab::Leds => render_leds(f, app, area),
        Tab::Settings => render_settings(f, app, area),
    }
}

fn render_buttons(f: &mut Frame, app: &App, area: Rect) {
    let profile = app.current_profile();
    let items: Vec<ListItem> = profile
        .buttons
        .iter()
        .enumerate()
        .map(|(i, binding)| {
            let slot_name = app
                .desc
                .button_slots
                .get(i)
                .map(|c| c.name())
                .unwrap_or("?");
            let action = config::format_binding(binding);
            let text = format!("  {:14} -> {}", slot_name, action);

            let style = if i == app.cursor && app.mode == Mode::Normal {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let title = if app.mode == Mode::Normal {
        "Buttons  [Enter] edit  [p] press to select"
    } else {
        "Buttons"
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}

fn render_dpi(f: &mut Frame, app: &App, area: Rect) {
    let profile = app.current_profile();
    let items: Vec<ListItem> = profile
        .settings
        .dpi_presets
        .iter()
        .enumerate()
        .map(|(i, &dpi)| {
            let text = if matches!(app.mode, Mode::EditingDpi { preset } if preset == i) {
                format!("  Preset {}: {}_ DPI", i + 1, app.input_buf)
            } else {
                format!("  Preset {}: {} DPI", i + 1, dpi)
            };

            let style = if i == app.cursor {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("DPI Presets  [Enter] edit  [</>] adjust by 50"),
    );
    f.render_widget(list, area);
}

fn render_leds(f: &mut Frame, app: &App, area: Rect) {
    let profile = app.current_profile();
    let items: Vec<ListItem> = profile
        .leds
        .iter()
        .enumerate()
        .map(|(i, led)| {
            let zone_name = app.desc.led_names.get(i).copied().unwrap_or("?");

            let editing_color = matches!(app.mode, Mode::EditingLedColor { zone } if zone == i);
            let color_str = if editing_color {
                format!("#{}_", app.input_buf)
            } else {
                format!("#{:02x}{:02x}{:02x}", led.r, led.g, led.b)
            };

            let label = format!(
                "  {:6}  mode={:10}  brightness={}  color=",
                zone_name,
                led.mode.name(),
                led.brightness,
            );

            let swatch = Span::styled(
                "  ",
                Style::default().bg(Color::Rgb(led.r, led.g, led.b)),
            );

            let cursor_style = if i == app.cursor {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(label, cursor_style),
                Span::styled(color_str, cursor_style),
                Span::raw(" "),
                swatch,
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("LEDs  [Enter] cycle mode  [c] color  [b] brightness  [</>] cycle"),
    );
    f.render_widget(list, area);
}

fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    let profile = app.current_profile();
    let items = vec![
        {
            let text = format!("  Polling rate:   {} Hz", profile.settings.polling_rate);
            let style = if app.cursor == 0 {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        },
        {
            let text = format!("  Debounce:       {} ms", profile.settings.debounce_ms);
            let style = if app.cursor == 1 {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        },
        {
            let on_off = if profile.settings.angle_snapping {
                "on"
            } else {
                "off"
            };
            let text = format!("  Angle snapping: {}", on_off);
            let style = if app.cursor == 2 {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        },
    ];

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Settings  [</>] cycle values  [Enter] toggle"),
    );
    f.render_widget(list, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_text = if let Some(ref status) = app.status {
        let style = if status.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        Line::from(Span::styled(&status.text, style))
    } else {
        let dirty_marker = if app.is_any_dirty() { " [modified]" } else { "" };
        Line::from(format!(
            " [q] quit  [s] save  [a] apply  [Tab] next tab  [1-{}] profile{}",
            app.desc.num_profiles, dirty_marker
        ))
    };

    let footer = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn render_binding_popup(f: &mut Frame, app: &App, slot: usize) {
    let slot_name = app
        .desc
        .button_slots
        .get(slot)
        .map(|c| c.name())
        .unwrap_or("?");

    let actions = binding_action_list();
    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let style = if i == app.binding_cursor {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {}", name)).style(style)
        })
        .collect();

    let area = centered_rect(40, 60, f.area());
    f.render_widget(Clear, area);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Bind: {} [Enter] select [Esc] cancel", slot_name)),
    );
    f.render_widget(list, area);
}

fn render_waiting_popup(f: &mut Frame) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);
    let text = Paragraph::new("\n  Press a mouse button to select it...\n  [Esc] cancel")
        .block(Block::default().borders(Borders::ALL).title("Button Select"));
    f.render_widget(text, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn binding_action_list() -> Vec<(&'static str, crate::types::ButtonBinding)> {
    use crate::types::ButtonBinding;
    vec![
        ("left_click", ButtonBinding::mouse_action(ButtonCode::LeftClick)),
        ("right_click", ButtonBinding::mouse_action(ButtonCode::RightClick)),
        ("middle_click", ButtonBinding::mouse_action(ButtonCode::MiddleClick)),
        ("back", ButtonBinding::mouse_action(ButtonCode::Back)),
        ("forward", ButtonBinding::mouse_action(ButtonCode::Forward)),
        ("dpi_cycle", ButtonBinding::mouse_action(ButtonCode::DpiCycle)),
        ("dpi_target", ButtonBinding::mouse_action(ButtonCode::DpiTarget)),
        ("scroll_up", ButtonBinding::mouse_action(ButtonCode::ScrollUp)),
        ("scroll_down", ButtonBinding::mouse_action(ButtonCode::ScrollDown)),
        ("side_a", ButtonBinding::mouse_action(ButtonCode::SideA)),
        ("side_b", ButtonBinding::mouse_action(ButtonCode::SideB)),
        ("side_c", ButtonBinding::mouse_action(ButtonCode::SideC)),
        ("side_d", ButtonBinding::mouse_action(ButtonCode::SideD)),
        ("side_e", ButtonBinding::mouse_action(ButtonCode::SideE)),
        ("side_f", ButtonBinding::mouse_action(ButtonCode::SideF)),
        ("side_g", ButtonBinding::mouse_action(ButtonCode::SideG)),
        ("side_h", ButtonBinding::mouse_action(ButtonCode::SideH)),
        ("side_i", ButtonBinding::mouse_action(ButtonCode::SideI)),
        ("side_j", ButtonBinding::mouse_action(ButtonCode::SideJ)),
        ("side_k", ButtonBinding::mouse_action(ButtonCode::SideK)),
        ("side_l", ButtonBinding::mouse_action(ButtonCode::SideL)),
        ("disabled", ButtonBinding::disabled()),
        ("key:a", ButtonBinding::keyboard_key(0x04)),
        ("key:b", ButtonBinding::keyboard_key(0x05)),
        ("key:c", ButtonBinding::keyboard_key(0x06)),
        ("key:d", ButtonBinding::keyboard_key(0x07)),
        ("key:e", ButtonBinding::keyboard_key(0x08)),
        ("key:f", ButtonBinding::keyboard_key(0x09)),
        ("key:r", ButtonBinding::keyboard_key(0x15)),
        ("key:space", ButtonBinding::keyboard_key(0x2C)),
        ("key:enter", ButtonBinding::keyboard_key(0x28)),
        ("key:escape", ButtonBinding::keyboard_key(0x29)),
        ("key:tab", ButtonBinding::keyboard_key(0x2B)),
        ("key:f1", ButtonBinding::keyboard_key(0x3A)),
        ("key:f2", ButtonBinding::keyboard_key(0x3B)),
        ("key:f3", ButtonBinding::keyboard_key(0x3C)),
        ("key:f4", ButtonBinding::keyboard_key(0x3D)),
        ("key:f5", ButtonBinding::keyboard_key(0x3E)),
    ]
}
