use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
    },
};
use zbus::{Connection, proxy};

// ── Palette ───────────────────────────────────────────────────────────────────

const BLUE: Color     = Color::Rgb(82, 148, 226);
const BLUE_DIM: Color = Color::Rgb(50, 90, 140);
const BLUE_BG: Color  = Color::Rgb(18, 30, 50);
const GREEN: Color    = Color::Rgb(80, 200, 120);
const AMBER: Color    = Color::Rgb(230, 170, 50);
const RED: Color      = Color::Rgb(220, 80, 80);
const PURPLE: Color   = Color::Rgb(170, 100, 220);
const FG: Color       = Color::Rgb(220, 225, 235);
const FG_DIM: Color   = Color::Rgb(110, 120, 140);
const SEL_BG: Color   = Color::Rgb(28, 48, 78);
const PANEL_BG: Color = Color::Rgb(12, 18, 30);
const TEAL: Color     = Color::Rgb(60, 200, 180);

// ── BlueZ D-Bus proxies ───────────────────────────────────────────────────────

#[proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.bluez",
    default_path = "/"
)]
trait ObjectManager {
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<
                String,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            >,
        >,
    >;
}

#[proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
trait Device {
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
    fn pair(&self) -> zbus::Result<()>;
    fn cancel_pairing(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn paired(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn trusted(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_trusted(&self, value: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn rssi(&self) -> zbus::Result<i16>;
    #[zbus(property)]
    fn icon(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn alias(&self) -> zbus::Result<String>;
}

#[proxy(interface = "org.bluez.Battery1", default_service = "org.bluez")]
trait Battery {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<u8>;
}

#[proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
trait Adapter {
    fn start_discovery(&self) -> zbus::Result<()>;
    fn stop_discovery(&self) -> zbus::Result<()>;
    fn remove_device(&self, device: zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    #[zbus(property)]
    fn discovering(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn address(&self) -> zbus::Result<String>;
}

// ── Device model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct BtDevice {
    path: String,
    name: String,
    address: String,
    paired: bool,
    trusted: bool,
    connected: bool,
    rssi: Option<i16>,
    icon: String,
    battery: Option<u8>,
}

impl BtDevice {
    fn emoji(&self) -> &'static str {
        match self.icon.as_str() {
            "audio-headset"    => "🎧",
            "audio-headphones" => "🎧",
            "audio-card"       => "🔊",
            "input-keyboard"   => "⌨️",
            "input-mouse"      => "🖱️",
            "input-gaming"     => "🎮",
            "phone"            => "📱",
            "computer"         => "💻",
            "printer"          => "🖨️",
            _                  => "📶",
        }
    }

    fn kind_label(&self) -> &'static str {
        match self.icon.as_str() {
            "audio-headset"    => "Headset",
            "audio-headphones" => "Headphones",
            "audio-card"       => "Speaker",
            "input-keyboard"   => "Keyboard",
            "input-mouse"      => "Mouse",
            "input-gaming"     => "Gamepad",
            "phone"            => "Phone",
            "computer"         => "Computer",
            "printer"          => "Printer",
            _                  => "Bluetooth",
        }
    }

    fn signal_bars(&self) -> &'static str {
        match self.rssi {
            Some(r) if r > -60 => "▂▄▆█",
            Some(r) if r > -70 => "▂▄▆░",
            Some(r) if r > -80 => "▂▄░░",
            Some(_)            => "▂░░░",
            None               => "░░░░",
        }
    }

    fn signal_color(&self) -> Color {
        match self.rssi {
            Some(r) if r > -60 => GREEN,
            Some(r) if r > -70 => Color::Rgb(180, 210, 80),
            Some(r) if r > -80 => AMBER,
            Some(_)            => RED,
            None               => FG_DIM,
        }
    }

    fn battery_bar(&self) -> Option<(&'static str, Color)> {
        let pct = self.battery?;
        let v = if pct > 90 { ("█████", GREEN) }
            else if pct > 70 { ("████░", GREEN) }
            else if pct > 50 { ("███░░", Color::Rgb(180, 210, 80)) }
            else if pct > 30 { ("██░░░", AMBER) }
            else if pct > 15 { ("█░░░░", RED) }
            else             { ("▌░░░░", RED) };
        Some(v)
    }

    fn battery_emoji(&self) -> &'static str {
        match self.battery {
            Some(p) if p > 20 => "🔋",
            Some(_)           => "🪫",
            None              => "",
        }
    }
}

// ── D-Bus operations ──────────────────────────────────────────────────────────

async fn fetch_devices(conn: &Connection) -> Result<Vec<BtDevice>> {
    let manager = ObjectManagerProxy::new(conn)
        .await
        .context("Failed to connect to BlueZ")?;

    let objects = manager
        .get_managed_objects()
        .await
        .context("BlueZ returned no objects — is bluetoothd running?")?;

    let mut devices = Vec::new();

    for (path, interfaces) in &objects {
        if !interfaces.contains_key("org.bluez.Device1") {
            continue;
        }
        let proxy = DeviceProxy::builder(conn)
            .path(path.as_ref())?
            .build()
            .await?;

        let address   = proxy.address().await.unwrap_or_default();
        let name = match proxy.alias().await {
            Ok(a) if !a.is_empty() => a,
            _ => proxy.name().await.unwrap_or_else(|_| address.clone()),
        };
        let paired    = proxy.paired().await.unwrap_or(false);
        let trusted   = proxy.trusted().await.unwrap_or(false);
        let connected = proxy.connected().await.unwrap_or(false);
        let rssi      = proxy.rssi().await.ok();
        let icon      = proxy.icon().await.unwrap_or_default();

        let battery: Option<u8> = if interfaces.contains_key("org.bluez.Battery1") {
            async {
                let b = BatteryProxy::builder(conn)
                    .path(path.as_ref())?
                    .build()
                    .await?;
                b.percentage().await
            }
            .await
            .ok()
        } else {
            None
        };

        devices.push(BtDevice { path: path.to_string(), name, address, paired, trusted, connected, rssi, icon, battery });
    }

    devices.sort_by(|a, b| {
        b.connected.cmp(&a.connected)
            .then(b.paired.cmp(&a.paired))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(devices)
}

async fn find_adapter_path(conn: &Connection) -> Result<String> {
    let manager = ObjectManagerProxy::new(conn).await?;
    let objects = manager.get_managed_objects().await?;
    for (path, interfaces) in &objects {
        if interfaces.contains_key("org.bluez.Adapter1") {
            return Ok(path.to_string());
        }
    }
    anyhow::bail!("no bluetooth adapter found")
}

async fn set_trusted(conn: &Connection, path: &str, trusted: bool) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.set_trusted(trusted).await?;
    Ok(())
}

async fn connect_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.connect().await?;
    Ok(())
}

async fn disconnect_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.disconnect().await?;
    Ok(())
}

async fn pair_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.pair().await?;
    Ok(())
}

async fn remove_device(conn: &Connection, adapter_path: &str, device_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    let path = zbus::zvariant::ObjectPath::try_from(device_path)?;
    proxy.remove_device(path).await?;
    Ok(())
}

async fn start_discovery(conn: &Connection, adapter_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    proxy.start_discovery().await?;
    Ok(())
}

async fn stop_discovery(conn: &Connection, adapter_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    proxy.stop_discovery().await?;
    Ok(())
}

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceAction {
    Connect,
    Disconnect,
    Pair,
    Remove,
    ToggleAutoconnect,
}

impl DeviceAction {
    fn label(&self) -> &'static str {
        match self {
            Self::Connect          => "Connect",
            Self::Disconnect       => "Disconnect",
            Self::Pair             => "Pair",
            Self::Remove           => "Remove / unpair",
            Self::ToggleAutoconnect => "Toggle autoconnect",
        }
    }

    fn emoji(&self) -> &'static str {
        match self {
            Self::Connect          => "🔗",
            Self::Disconnect       => "⏏️",
            Self::Pair             => "🤝",
            Self::Remove           => "🗑️",
            Self::ToggleAutoconnect => "✦",
        }
    }

    fn accent(&self) -> Color {
        match self {
            Self::Connect          => GREEN,
            Self::Disconnect       => AMBER,
            Self::Pair             => BLUE,
            Self::Remove           => RED,
            Self::ToggleAutoconnect => TEAL,
        }
    }
}

fn available_actions(dev: &BtDevice) -> Vec<DeviceAction> {
    let mut actions = Vec::new();
    if dev.paired {
        if dev.connected {
            actions.push(DeviceAction::Disconnect);
        } else {
            actions.push(DeviceAction::Connect);
        }
        actions.push(DeviceAction::ToggleAutoconnect);
        actions.push(DeviceAction::Remove);
    } else {
        actions.push(DeviceAction::Pair);
    }
    actions
}

enum Popup {
    None,
    ActionMenu { device_idx: usize, selected: usize },
    Confirm { device_idx: usize, action: DeviceAction },
    Working { device_idx: usize, action: DeviceAction },
    Message { text: String, ok: bool },
    Scanning,
}

struct App {
    devices: Vec<BtDevice>,
    list_state: ListState,
    popup: Popup,
    loading: bool,
    error: Option<String>,
    adapter_path: Option<String>,
    adapter_name: Option<String>,
    adapter_address: Option<String>,
    scanning: bool,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            devices: Vec::new(),
            list_state,
            popup: Popup::None,
            loading: true,
            error: None,
            adapter_path: None,
            adapter_name: None,
            adapter_address: None,
            scanning: false,
        }
    }

    fn move_up(&mut self) {
        if self.devices.is_empty() { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
    }

    fn move_down(&mut self) {
        if self.devices.is_empty() { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1).min(self.devices.len() - 1)));
    }

    fn selected_device(&self) -> Option<&BtDevice> {
        self.list_state.selected().and_then(|i| self.devices.get(i))
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

fn ui(frame: &mut Frame, app: &mut App) {
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

    // Render popups on top
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

fn render_header(frame: &mut Frame, area: Rect, scanning: bool, adapter_name: Option<&str>, adapter_address: Option<&str>) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(BLUE_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 0 });

    let scan_tag = if scanning {
        Span::styled("  📡 scanning…", Style::default().fg(PURPLE).add_modifier(Modifier::BOLD))
    } else {
        Span::raw("")
    };

    let title_line = Line::from(vec![
        Span::styled("📶 ", Style::default()),
        Span::styled("btx", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        Span::styled("  bluetooth manager", Style::default().fg(FG_DIM)),
        scan_tag,
    ]);

    // Second line: adapter name + MAC, or fallback hint
    let subtitle_line = match (adapter_name, adapter_address) {
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
    };

    frame.render_widget(
        Paragraph::new(Text::from(vec![title_line, subtitle_line])),
        Rect { y: inner.y, height: inner.height.min(2), ..inner },
    );
}

fn render_loading(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  🔍 scanning devices…", Style::default().fg(FG_DIM))))
            .block(Block::default().padding(Padding::vertical(2))),
        area,
    );
}

fn render_error(frame: &mut Frame, area: Rect, err: &str) {
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

fn render_empty(frame: &mut Frame, area: Rect) {
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

fn render_body(frame: &mut Frame, area: Rect, app: &mut App) {
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
        ("  not paired ", Style::default().fg(FG_DIM).bg(Color::Rgb(22, 22, 32)))
    } else if d.trusted {
        ("  ✦ auto     ", Style::default().fg(GREEN).bg(Color::Rgb(15, 40, 22)).add_modifier(Modifier::BOLD))
    } else {
        ("  · no auto  ", Style::default().fg(FG_DIM).bg(Color::Rgb(22, 22, 32)))
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

fn render_detail_panel(frame: &mut Frame, area: Rect, device: Option<&BtDevice>) {
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

    // Contextual action hints
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

fn render_footer(frame: &mut Frame, area: Rect, popup: &Popup) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BLUE_DIM))
        .style(Style::default().bg(BLUE_BG));
    frame.render_widget(block, area);

    let inner = area.inner(Margin { horizontal: 2, vertical: 0 });

    let spans: Vec<Span> = match popup {
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

// ── Action menu popup ─────────────────────────────────────────────────────────

fn render_action_menu(frame: &mut Frame, area: Rect, dev: &BtDevice, selected: usize) {
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

    // Device subtitle
    let sub = Line::from(vec![
        Span::styled(format!("  {}", dev.address), Style::default().fg(FG_DIM)),
    ]);
    frame.render_widget(Paragraph::new(sub), Rect { y: inner.y, height: 1, ..inner });

    // Action items
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

// ── Confirm popup ─────────────────────────────────────────────────────────────

fn render_confirm_popup(frame: &mut Frame, area: Rect, dev: &BtDevice, action: DeviceAction) {
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

// ── Working popup ─────────────────────────────────────────────────────────────

fn render_working_popup(frame: &mut Frame, area: Rect, dev: &BtDevice, action: DeviceAction) {
    let popup_area = centered_rect(52, 7, area);
    frame.render_widget(Clear, popup_area);

    let accent = action.accent();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            format!("  {}  {}…  ", action.emoji(), action.label()),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 3, vertical: 1 });
    let lines = Text::from(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}  ", dev.emoji()), Style::default()),
            Span::styled(dev.name.clone(), Style::default().fg(FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled("  please wait…", Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC))),
    ]);
    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Scan overlay ──────────────────────────────────────────────────────────────

fn render_scan_overlay(frame: &mut Frame, area: Rect, devices: &[BtDevice]) {
    let popup_area = centered_rect(62, 20, area);
    frame.render_widget(Clear, popup_area);

    let unpaired: Vec<&BtDevice> = devices.iter().filter(|d| !d.paired).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(PURPLE))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(Span::styled(
            "  📡  scanning for nearby devices  ",
            Style::default().fg(PURPLE).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 2, vertical: 1 });

    if unpaired.is_empty() {
        let text = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  searching… put device in pairing mode",
                Style::default().fg(FG_DIM).add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  press Esc to stop scanning",
                Style::default().fg(FG_DIM),
            )),
        ]);
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }

    let header = Line::from(Span::styled(
        format!("  {} device(s) found — select to pair:", unpaired.len()),
        Style::default().fg(FG_DIM),
    ));
    frame.render_widget(Paragraph::new(header), Rect { y: inner.y, height: 1, ..inner });

    let list_area = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };

    let items: Vec<ListItem> = unpaired.iter().map(|d| {
        let rssi_str = d.rssi.map(|r| format!("  {} dBm", r)).unwrap_or_default();
        Line::from(vec![
            Span::styled(format!("  {}  ", d.emoji()), Style::default().fg(BLUE)),
            Span::styled(format!("{:<28}", truncate(&d.name, 28)), Style::default().fg(FG)),
            Span::styled(d.signal_bars(), Style::default().fg(d.signal_color())),
            Span::styled(rssi_str, Style::default().fg(FG_DIM)),
        ])
        .into()
    }).collect();

    let mut state = ListState::default();
    state.select(Some(0));
    let list = List::new(items)
        .highlight_style(Style::default().bg(SEL_BG).fg(FG).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    frame.render_stateful_widget(list, list_area, &mut state);
}

// ── Message popup ─────────────────────────────────────────────────────────────

fn render_message_popup(frame: &mut Frame, area: Rect, text: &str, ok: bool) {
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn kb(s: &str) -> Span<'static> {
    Span::styled(
        format!(" {} ", s),
        Style::default().fg(BLUE_BG).bg(BLUE).add_modifier(Modifier::BOLD),
    )
}

fn sep(s: &str) -> Span<'static> {
    Span::styled(format!(" {}  ", s), Style::default().fg(FG_DIM))
}

fn pad() -> Span<'static> {
    Span::raw("  ")
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let conn = Connection::system().await.context("Cannot connect to D-Bus system bus")?;
    let mut app = App::new();

    terminal.draw(|f| ui(f, &mut app))?;

    // Initial load
    match fetch_devices(&conn).await {
        Ok(devs) => { app.devices = devs; app.loading = false; }
        Err(e)   => { app.loading = false; app.error = Some(e.to_string()); }
    }
    if let Ok(path) = find_adapter_path(&conn).await {
        if let Ok(proxy) = AdapterProxy::builder(&conn).path(path.as_str())?.build().await {
            app.adapter_name    = proxy.name().await.ok();
            app.adapter_address = proxy.address().await.ok();
        }
        app.adapter_path = Some(path);
    }

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // During scan: poll more frequently and refresh device list
        let poll_ms = if app.scanning { 1000 } else { 150 };

        if !event::poll(Duration::from_millis(poll_ms))? {
            if app.scanning {
                // Refresh to pick up newly discovered devices
                if let Ok(devs) = fetch_devices(&conn).await {
                    let old = app.list_state.selected().unwrap_or(0);
                    app.devices = devs;
                    app.list_state.select(Some(old.min(app.devices.len().saturating_sub(1))));
                }
            }
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press { continue; }

            match &app.popup {
                // ── Normal navigation ───────────────────────────────────────
                Popup::None => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up   | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),

                    KeyCode::Char('r') => {
                        app.loading = true;
                        app.error = None;
                        terminal.draw(|f| ui(f, &mut app))?;
                        match fetch_devices(&conn).await {
                            Ok(devs) => {
                                let old = app.list_state.selected().unwrap_or(0);
                                app.devices = devs;
                                app.loading = false;
                                app.list_state.select(Some(old.min(app.devices.len().saturating_sub(1))));
                            }
                            Err(e) => { app.loading = false; app.error = Some(e.to_string()); }
                        }
                    }

                    KeyCode::Char('s') => {
                        if let Some(ref adapter) = app.adapter_path.clone() {
                            match start_discovery(&conn, adapter).await {
                                Ok(()) => {
                                    app.scanning = true;
                                    app.popup = Popup::Scanning;
                                }
                                Err(e) => {
                                    app.popup = Popup::Message { text: format!("scan failed: {}", e), ok: false };
                                }
                            }
                        } else {
                            app.popup = Popup::Message { text: "no bluetooth adapter found".into(), ok: false };
                        }
                    }

                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(idx) = app.list_state.selected() {
                            if app.devices.get(idx).is_some() {
                                app.popup = Popup::ActionMenu { device_idx: idx, selected: 0 };
                            }
                        }
                    }

                    _ => {}
                },

                // ── Action menu ─────────────────────────────────────────────
                Popup::ActionMenu { device_idx, selected } => {
                    let idx = *device_idx;
                    let sel = *selected;
                    let actions = app.devices.get(idx).map(available_actions).unwrap_or_default();
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            let new_sel = sel.saturating_sub(1);
                            app.popup = Popup::ActionMenu { device_idx: idx, selected: new_sel };
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let new_sel = (sel + 1).min(actions.len().saturating_sub(1));
                            app.popup = Popup::ActionMenu { device_idx: idx, selected: new_sel };
                        }
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if let Some(&action) = actions.get(sel) {
                                app.popup = Popup::Confirm { device_idx: idx, action };
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.popup = Popup::None;
                        }
                        _ => {}
                    }
                }

                // ── Confirm ─────────────────────────────────────────────────
                Popup::Confirm { device_idx, action } => {
                    let (idx, action) = (*device_idx, *action);
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            if let Some(dev) = app.devices.get(idx) {
                                let path         = dev.path.clone();
                                let name         = dev.name.clone();
                                let trusted      = dev.trusted;
                                let adapter_path = app.adapter_path.clone();

                                app.popup = Popup::Working { device_idx: idx, action };
                                terminal.draw(|f| ui(f, &mut app))?;

                                let result: Result<String> = match action {
                                    DeviceAction::Connect => {
                                        connect_device(&conn, &path).await?;
                                        Ok(format!("🔗 {} connected", name))
                                    }
                                    DeviceAction::Disconnect => {
                                        disconnect_device(&conn, &path).await?;
                                        Ok(format!("⏏️  {} disconnected", name))
                                    }
                                    DeviceAction::Pair => {
                                        pair_device(&conn, &path).await?;
                                        Ok(format!("🤝 {} paired successfully", name))
                                    }
                                    DeviceAction::Remove => {
                                        if let Some(ref ap) = adapter_path {
                                            remove_device(&conn, ap, &path).await?;
                                            Ok(format!("🗑️  {} removed", name))
                                        } else {
                                            anyhow::bail!("no adapter path available")
                                        }
                                    }
                                    DeviceAction::ToggleAutoconnect => {
                                        let new_val = !trusted;
                                        set_trusted(&conn, &path, new_val).await?;
                                        if new_val {
                                            Ok(format!("✦ {} will autoconnect on startup", name))
                                        } else {
                                            Ok(format!("· {} autoconnect disabled", name))
                                        }
                                    }
                                };

                                // Refresh device list after operation
                                if let Ok(devs) = fetch_devices(&conn).await {
                                    let old = app.list_state.selected().unwrap_or(0);
                                    app.devices = devs;
                                    app.list_state.select(Some(old.min(app.devices.len().saturating_sub(1))));
                                }

                                app.popup = match result {
                                    Ok(msg)  => Popup::Message { text: msg, ok: true },
                                    Err(e)   => Popup::Message { text: format!("error: {}", e), ok: false },
                                };
                            } else {
                                app.popup = Popup::None;
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.popup = Popup::None;
                        }
                        _ => {}
                    }
                }

                // ── Scan overlay ────────────────────────────────────────────
                Popup::Scanning => {
                    let unpaired_count = app.devices.iter().filter(|d| !d.paired).count();
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            if let Some(ref adapter) = app.adapter_path.clone() {
                                let _ = stop_discovery(&conn, adapter).await;
                            }
                            app.scanning = false;
                            app.popup = Popup::None;
                        }
                        KeyCode::Enter | KeyCode::Char(' ') if unpaired_count > 0 => {
                            // Find first unpaired device and open its action menu
                            if let Some(idx) = app.devices.iter().position(|d| !d.paired) {
                                if let Some(ref adapter) = app.adapter_path.clone() {
                                    let _ = stop_discovery(&conn, adapter).await;
                                }
                                app.scanning = false;
                                app.list_state.select(Some(idx));
                                app.popup = Popup::Confirm { device_idx: idx, action: DeviceAction::Pair };
                            }
                        }
                        _ => {}
                    }
                }

                // ── Message ─────────────────────────────────────────────────
                Popup::Message { .. } => { app.popup = Popup::None; }

                // ── Working (no input while in progress) ────────────────────
                Popup::Working { .. } => {}
            }
        }
    }

    // Clean up discovery if still running
    if app.scanning {
        if let Some(ref adapter) = app.adapter_path {
            let _ = stop_discovery(&conn, adapter).await;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
