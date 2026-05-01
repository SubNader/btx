use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
};

use crate::model::Popup;
use crate::palette::*;

pub fn render_header(
    frame: &mut Frame,
    area: Rect,
    scanning: bool,
    popup: &Popup,
    adapter_name: Option<&str>,
    adapter_address: Option<&str>,
) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(BLUE_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 0 });

    let status_tag = match popup {
        Popup::Working { action, .. } => Span::styled(
            format!("  {}  {}…", action.emoji(), action.label()),
            Style::default().fg(action.accent()).add_modifier(Modifier::BOLD),
        ),
        Popup::PinInput { .. } | Popup::PasskeyInput { .. } => Span::styled(
            "  🔑  authentication required",
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        ),
        Popup::ConfirmPasskey { .. } => Span::styled(
            "  🔐  confirm passkey",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        ),
        Popup::DisplayPasskey { .. } => Span::styled(
            "  📟  type on device",
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        ),
        _ if scanning => Span::styled(
            "  📡 scanning…",
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        ),
        _ => Span::raw(""),
    };

    let title_line = Line::from(vec![
        Span::styled("📶 ", Style::default()),
        Span::styled("btx", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        Span::styled("  bluetooth manager", Style::default().fg(FG_DIM)),
        status_tag,
    ]);

    let subtitle_line = if matches!(popup, Popup::Working { .. }) {
        Line::from(Span::styled(
            "   please wait…",
            Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC),
        ))
    } else {
        match (adapter_name, adapter_address) {
            (Some(name), Some(addr)) => Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(name, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
                Span::styled("  ", Style::default()),
                Span::styled(addr, Style::default().fg(FG_DIM)),
            ]),
            _ => Line::from(Span::styled(
                "   connect · pair · autoconnect · battery",
                Style::default().fg(FG_DIM),
            )),
        }
    };

    frame.render_widget(
        Paragraph::new(Text::from(vec![title_line, subtitle_line])),
        Rect { y: inner.y, height: inner.height.min(2), ..inner },
    );
}

pub fn render_loading(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  🔍 scanning devices…", Style::default().fg(FG_DIM))))
            .block(Block::default().padding(Padding::vertical(2))),
        area,
    );
}

pub fn render_error(frame: &mut Frame, area: Rect, err: &str) {
    let text = Text::from(vec![
        Line::from(Span::styled("  ✗  could not reach BlueZ", Style::default().fg(RED).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("     {}", err), Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(Span::styled("     make sure bluetoothd is running and try  r  to refresh", Style::default().fg(FG_DIM))),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().padding(Padding::vertical(2))).wrap(Wrap { trim: false }),
        area,
    );
}

pub fn render_empty(frame: &mut Frame, area: Rect) {
    let text = Text::from(vec![
        Line::from(Span::styled("   📭  no devices found", Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(Span::styled("   press  s  to scan for nearby devices", Style::default().fg(FG_DIM))),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().padding(Padding::vertical(2))),
        area,
    );
}
