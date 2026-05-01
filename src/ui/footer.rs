use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::model::Popup;
use crate::palette::*;
use crate::ui::{kb, sep, pad};

pub fn render_footer(frame: &mut Frame, area: Rect, popup: &Popup) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(BLUE_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 0 });

    let spans = match popup {
        Popup::None => vec![
            kb("↑↓/jk"), sep("navigate"), pad(),
            kb("Enter"), sep("actions"), pad(),
            kb("s"), sep("scan"), pad(),
            kb("r"), sep("refresh"), pad(),
            kb("q"), sep("quit"),
        ],
        Popup::ActionMenu { .. } => vec![
            kb("↑↓/jk"), sep("select"), pad(),
            kb("Enter"), sep("run"), pad(),
            kb("Esc"), sep("back"),
        ],
        Popup::Scanning => vec![
            kb("↑↓/jk"), sep("select"), pad(),
            kb("Enter"), sep("pair"), pad(),
            kb("Esc"), sep("stop scan"),
        ],
        _ => vec![
            kb("y / Enter"), sep("confirm"), pad(),
            kb("n / Esc"), sep("cancel"),
        ],
    };

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        Rect { y: inner.y + 1, height: 1, ..inner },
    );
}
