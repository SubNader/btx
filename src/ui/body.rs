use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::model::{App, BtDevice};
use crate::palette::*;
use crate::ui::truncate;

pub fn render_body(frame: &mut Frame, area: Rect, app: &mut App) {
    let use_split = area.width >= 92;
    let (list_area, detail_area) = if use_split {
        let cols = Layout::horizontal([Constraint::Min(46), Constraint::Length(44)]).split(area);
        (cols[0], Some(cols[1]))
    } else {
        (area, None)
    };

    render_device_list(frame, list_area, app);

    if let Some(d_area) = detail_area {
        render_detail_panel(frame, d_area, app.selected_device());
    }
}

fn render_device_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app.devices.iter().map(build_list_item).collect();

    let list = List::new(items)
        .block(Block::default().padding(Padding::new(1, 1, 1, 0)))
        .highlight_style(Style::default().bg(SEL_BG).fg(FG).add_modifier(Modifier::BOLD))
        .highlight_symbol("▌ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn build_list_item(d: &BtDevice) -> ListItem<'static> {
    let conn_dot = if d.connected {
        Span::styled("● ", Style::default().fg(GREEN))
    } else {
        Span::styled("○ ", Style::default().fg(FG_DIM))
    };

    let name_style = if d.connected {
        Style::default().fg(FG).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG)
    };

    let emoji = format!("{}  ", d.emoji());
    let name  = format!("{:<28}", truncate(&d.name, 28));

    let (ac_label, ac_style) = if !d.paired {
        ("  not paired ", Style::default().fg(FG_DIM).bg(ratatui::style::Color::Rgb(22, 22, 32)))
    } else if d.trusted {
        ("  ✦ auto     ", Style::default().fg(GREEN).bg(ratatui::style::Color::Rgb(15, 40, 22)).add_modifier(Modifier::BOLD))
    } else {
        ("  · no auto  ", Style::default().fg(FG_DIM).bg(ratatui::style::Color::Rgb(22, 22, 32)))
    };

    let batt_span = match d.battery {
        Some(p) => {
            let color = if p > 50 { GREEN } else if p > 20 { AMBER } else { RED };
            Span::styled(format!(" {}{}% ", d.battery_emoji(), p), Style::default().fg(color))
        }
        None => Span::raw(""),
    };

    Line::from(vec![
        conn_dot,
        Span::styled(emoji, Style::default().fg(BLUE)),
        Span::styled(name, name_style),
        Span::styled(ac_label, ac_style),
        batt_span,
    ])
    .into()
}

pub fn render_detail_panel(frame: &mut Frame, area: Rect, device: Option<&BtDevice>) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(PANEL_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 1 });

    let Some(d) = device else {
        frame.render_widget(
            Paragraph::new(Span::styled("select a device", Style::default().fg(FG_DIM))),
            inner,
        );
        return;
    };

    let conn_color = if d.connected { GREEN } else { FG_DIM };
    let conn_label = if d.connected { "🟢 connected" } else { "⚪ not connected" };
    let ac_color   = if d.trusted { GREEN } else { FG_DIM };

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            format!("{}  {}", d.emoji(), d.kind_label()),
            Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("name     ", Style::default().fg(FG_DIM)),
            Span::styled(d.name.clone(), Style::default().fg(FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("address  ", Style::default().fg(FG_DIM)),
            Span::styled(d.address.clone(), Style::default().fg(FG_DIM)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("status   ", Style::default().fg(FG_DIM)),
            Span::styled(conn_label, Style::default().fg(conn_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("paired   ", Style::default().fg(FG_DIM)),
            Span::styled(
                if d.paired { "✔ yes" } else { "✘ no" },
                Style::default().fg(if d.paired { FG } else { AMBER }),
            ),
        ]),
        Line::from(vec![
            Span::styled("signal   ", Style::default().fg(FG_DIM)),
            Span::styled(d.signal_bars(), Style::default().fg(d.signal_color())),
            Span::styled(
                d.rssi.map(|r| format!("  {} dBm", r)).unwrap_or_else(|| "  n/a".into()),
                Style::default().fg(FG_DIM),
            ),
        ]),
    ];

    if let Some(pct) = d.battery {
        let (bar, bar_color) = d.battery_bar().unwrap_or(("░░░░░", FG_DIM));
        let batt_color = if pct > 50 { GREEN } else if pct > 20 { AMBER } else { RED };
        lines.push(Line::from(vec![
            Span::styled("battery  ", Style::default().fg(FG_DIM)),
            Span::styled(bar, Style::default().fg(bar_color)),
            Span::styled(
                format!("  {}{}%", d.battery_emoji(), pct),
                Style::default().fg(batt_color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    lines.push(Line::from(""));

    let (ac_icon, ac_desc) = if d.trusted {
        ("✦", "enabled — reconnects at startup")
    } else {
        ("·", "disabled")
    };
    lines.push(Line::from(vec![
        Span::styled("autoconn ", Style::default().fg(FG_DIM)),
        Span::styled(
            format!("{} {}", ac_icon, ac_desc),
            Style::default().fg(ac_color).add_modifier(Modifier::BOLD),
        ),
    ]));

    if d.trusted {
        lines.push(Line::from(vec![
            Span::styled("         ", Style::default()),
            Span::styled(
                "🔄 will reconnect when bluetooth starts",
                Style::default().fg(TEAL).add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    lines.push(Line::from(""));

    if d.paired {
        let conn_hint = if d.connected { "⏏️  disconnect" } else { "🔗 connect" };
        lines.push(Line::from(Span::styled(
            format!("Enter  actions  ·  {}  ·  ✦ autoconn", conn_hint),
            Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Enter  pair this device",
            Style::default().fg(AMBER).add_modifier(Modifier::ITALIC),
        )));
    }

    frame.render_widget(Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }), inner);
}
