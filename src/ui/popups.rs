use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::model::{available_actions, BtDevice, DeviceAction};
use crate::palette::*;
use crate::ui::{centered_rect, kb, truncate};

pub fn render_action_menu(frame: &mut Frame, area: Rect, dev: &BtDevice, selected: usize) {
    let actions = available_actions(dev);
    let height  = (actions.len() as u16) + 6;
    let popup_area = centered_rect(52, height, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BLUE))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            format!("  {}  {}  ", dev.emoji(), truncate(&dev.name, 24)),
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 2, vertical: 1 });

    let sub = Line::from(vec![
        Span::styled(format!("  {}", dev.address), Style::default().fg(FG_DIM)),
    ]);
    frame.render_widget(Paragraph::new(sub), Rect { y: inner.y, height: 1, ..inner });

    let item_area = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };
    let items: Vec<ListItem> = actions.iter().enumerate().map(|(i, action)| {
        let style = if i == selected {
            Style::default().fg(action.accent()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG_DIM)
        };
        let prefix = if i == selected { "▶ " } else { "  " };
        ListItem::new(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(format!("{}  ", action.emoji()), style),
            Span::styled(action.label(), style),
        ]))
    }).collect();

    let mut state = ListState::default();
    state.select(Some(selected));
    let list = List::new(items)
        .highlight_style(Style::default().bg(SEL_BG))
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Never);
    frame.render_stateful_widget(list, item_area, &mut state);
}

pub fn render_confirm_popup(frame: &mut Frame, area: Rect, dev: &BtDevice, action: DeviceAction) {
    let popup_area = centered_rect(56, 13, area);
    frame.render_widget(Clear, popup_area);

    let accent = action.accent();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            format!("  {}  {}  ", action.emoji(), action.label()),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });

    let consequence: &str = match action {
        DeviceAction::Connect          => "🔗 will connect to this device now",
        DeviceAction::Disconnect       => "⏏️  will disconnect this device",
        DeviceAction::Pair             => "🤝 will initiate pairing — keep device in pairing mode",
        DeviceAction::Remove           => "🗑️  device will be removed and must be re-paired",
        DeviceAction::ToggleAutoconnect => if dev.trusted {
            "⏸  will disable autoconnect at startup"
        } else {
            "🔄 will enable autoconnect at startup"
        },
    };

    let lines = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled("Device:", Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {}  ", dev.emoji()), Style::default()),
            Span::styled(dev.name.clone(), Style::default().fg(FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("  {}", dev.address), Style::default().fg(FG_DIM)),
        ]),
        Line::from(""),
        Line::from(Span::styled(consequence, Style::default().fg(accent).add_modifier(Modifier::ITALIC))),
        Line::from(""),
        Line::from(vec![
            kb("y / Enter"), Span::raw("  confirm    "), kb("n / Esc"), Span::raw("  cancel"),
        ]),
    ]);

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_pin_input(frame: &mut Frame, area: Rect, device: &str, input: &str, numeric: bool) {
    let popup_area = centered_rect(52, 10, area);
    frame.render_widget(Clear, popup_area);

    let (title, hint) = if numeric {
        ("  🔢  enter passkey  ", "6-digit number shown on device")
    } else {
        ("  🔑  enter PIN code  ", "check device label or use 0000 / 1234")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BLUE))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(title, Style::default().fg(BLUE).add_modifier(Modifier::BOLD))));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });

    let masked: String = if numeric {
        format!("{:_<6}", input)
    } else {
        input.to_string()
    };

    let lines = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(truncate(device, 44), Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&masked, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
        ]),
        Line::from(Span::styled(hint, Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC))),
        Line::from(""),
        Line::from(vec![kb("Enter"), Span::raw("  confirm    "), kb("Esc"), Span::raw("  cancel")]),
    ]);

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_confirm_passkey(frame: &mut Frame, area: Rect, device: &str, passkey: u32) {
    let popup_area = centered_rect(52, 10, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TEAL))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            "  🔐  confirm passkey  ",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });

    let lines = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(truncate(device, 44), Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {:06}  ", passkey),
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "does this match what's shown on the device?",
            Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![kb("y / Enter"), Span::raw("  yes    "), kb("n / Esc"), Span::raw("  no")]),
    ]);

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_display_passkey(frame: &mut Frame, area: Rect, device: &str, passkey: &str) {
    let popup_area = centered_rect(52, 10, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PURPLE))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            "  📟  type this on your device  ",
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });

    let lines = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(truncate(device, 44), Style::default().fg(FG_DIM))),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}  ", passkey),
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "type this code on the device, then press Enter",
            Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![kb("Enter"), Span::raw("  done")]),
    ]);

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_message_popup(frame: &mut Frame, area: Rect, text: &str, ok: bool) {
    let popup_area = centered_rect(56, 7, area);
    frame.render_widget(Clear, popup_area);

    let (accent, title) = if ok {
        (GREEN, "  ✔  done  ")
    } else {
        (RED, "  ✗  error  ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(title, Style::default().fg(accent).add_modifier(Modifier::BOLD))));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });
    let lines = Text::from(vec![
        Line::from(""),
        Line::from(Span::styled(text, Style::default().fg(FG))),
        Line::from(""),
        Line::from(Span::styled("press any key to dismiss", Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC))),
    ]);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
