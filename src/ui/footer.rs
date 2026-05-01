use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::model::Popup;
use crate::palette::*;
use crate::ui::{kb, sep, pad};

pub fn render_footer(frame: &mut Frame, area: Rect, popup: &Popup, scanning: bool) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(BLUE_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 0 });

    let spans = match popup {
        Popup::None if scanning => vec![
            kb("↑↓/jk"), sep("navigate"), pad(),
            kb("Enter"), sep("actions"), pad(),
            kb("s"), sep("stop scan"), pad(),
            kb("q"), sep("quit"),
        ],
        Popup::None => vec![
            kb("↑↓/jk"), sep("navigate"), pad(),
            kb("Enter"), sep("actions"), pad(),
            kb("s"), sep("scan"), pad(),
            kb("q"), sep("quit"),
        ],
        Popup::ActionMenu { .. } => vec![
            kb("↑↓/jk"), sep("select"), pad(),
            kb("Enter"), sep("run"), pad(),
            kb("Esc"), sep("back"),
        ],
        Popup::Working { .. } => vec![
            Span::styled("  please wait…", Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC)),
        ],
        Popup::PinInput { .. } | Popup::PasskeyInput { .. } => vec![
            kb("Enter"), sep("confirm"), pad(),
            kb("Esc"), sep("cancel"),
        ],
        Popup::ConfirmPasskey { .. } => vec![
            kb("y / Enter"), sep("yes"), pad(),
            kb("n / Esc"), sep("no"),
        ],
        Popup::DisplayPasskey { .. } => vec![
            kb("Enter"), sep("done"),
        ],
        _ => vec![
            kb("y / Enter"), sep("confirm"), pad(),
            kb("n / Esc"), sep("cancel"),
        ],
    };

    let row = Rect { y: inner.y + 1, height: 1, ..inner };
    let [keybinds_area, version_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .areas(row);

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        keybinds_area,
    );

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    frame.render_widget(
        Paragraph::new(version)
            .alignment(Alignment::Right)
            .style(Style::default().fg(FG_DIM)),
        version_area,
    );
}
