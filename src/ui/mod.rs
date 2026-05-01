pub mod body;
pub mod footer;
pub mod header;
pub mod popups;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Block,
};

use crate::model::{App, Popup};
use crate::palette::*;

use body::render_body;
use footer::render_footer;
use header::{render_empty, render_error, render_header, render_loading};
use popups::{
    render_action_menu, render_confirm_popup, render_message_popup, render_scan_overlay,
    render_working_popup,
};

pub fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    frame.render_widget(Block::default().style(Style::default().bg(PANEL_BG)), area);

    let root = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(area);

    render_header(frame, root[0], app.scanning, app.adapter_name.as_deref(), app.adapter_address.as_deref());

    if app.loading {
        render_loading(frame, root[1]);
    } else if let Some(ref err) = app.error.clone() {
        render_error(frame, root[1], err);
    } else if app.devices.is_empty() {
        render_empty(frame, root[1]);
    } else {
        render_body(frame, root[1], app);
    }

    render_footer(frame, root[2], &app.popup);

    match &app.popup {
        Popup::ActionMenu { device_idx, selected } => {
            let idx = *device_idx;
            let sel = *selected;
            if let Some(dev) = app.devices.get(idx) {
                let dev = dev.clone();
                render_action_menu(frame, area, &dev, sel);
            }
        }
        Popup::Confirm { device_idx, action } => {
            let idx = *device_idx;
            let action = *action;
            if let Some(dev) = app.devices.get(idx) {
                let dev = dev.clone();
                render_confirm_popup(frame, area, &dev, action);
            }
        }
        Popup::Working { device_idx, action } => {
            let idx = *device_idx;
            let action = *action;
            if let Some(dev) = app.devices.get(idx) {
                let dev = dev.clone();
                render_working_popup(frame, area, &dev, action);
            }
        }
        Popup::Message { text, ok } => {
            let (text, ok) = (text.clone(), *ok);
            render_message_popup(frame, area, &text, ok);
        }
        Popup::Scanning => {
            render_scan_overlay(frame, area, &app.devices);
        }
        Popup::None => {}
    }
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

pub fn kb(s: &str) -> Span<'static> {
    Span::styled(
        format!(" {} ", s),
        Style::default().fg(BLUE_BG).bg(BLUE).add_modifier(Modifier::BOLD),
    )
}

pub fn sep(s: &str) -> Span<'static> {
    Span::styled(format!(" {}  ", s), Style::default().fg(FG_DIM))
}

pub fn pad() -> Span<'static> {
    Span::raw("  ")
}

pub fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
