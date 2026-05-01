use ratatui::style::Color;
use ratatui::widgets::ListState;

use crate::palette::*;

#[derive(Debug, Clone)]
pub struct BtDevice {
    pub path: String,
    pub name: String,
    pub address: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    pub rssi: Option<i16>,
    pub icon: String,
    pub battery: Option<u8>,
}

impl BtDevice {
    pub fn emoji(&self) -> &'static str {
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

    pub fn kind_label(&self) -> &'static str {
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

    pub fn signal_bars(&self) -> &'static str {
        match self.rssi {
            Some(r) if r > -60 => "▂▄▆█",
            Some(r) if r > -70 => "▂▄▆░",
            Some(r) if r > -80 => "▂▄░░",
            Some(_)            => "▂░░░",
            None               => "░░░░",
        }
    }

    pub fn signal_color(&self) -> Color {
        match self.rssi {
            Some(r) if r > -60 => GREEN,
            Some(r) if r > -70 => Color::Rgb(180, 210, 80),
            Some(r) if r > -80 => AMBER,
            Some(_)            => RED,
            None               => FG_DIM,
        }
    }

    pub fn battery_bar(&self) -> Option<(&'static str, Color)> {
        let pct = self.battery?;
        let v = if pct > 90      { ("█████", GREEN) }
            else if pct > 70     { ("████░", GREEN) }
            else if pct > 50     { ("███░░", Color::Rgb(180, 210, 80)) }
            else if pct > 30     { ("██░░░", AMBER) }
            else if pct > 15     { ("█░░░░", RED) }
            else                 { ("▌░░░░", RED) };
        Some(v)
    }

    pub fn battery_emoji(&self) -> &'static str {
        match self.battery {
            Some(p) if p > 20 => "🔋",
            Some(_)           => "🪫",
            None              => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAction {
    Connect,
    Disconnect,
    Pair,
    Remove,
    ToggleAutoconnect,
}

impl DeviceAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connect           => "Connect",
            Self::Disconnect        => "Disconnect",
            Self::Pair              => "Pair",
            Self::Remove            => "Remove / unpair",
            Self::ToggleAutoconnect => "Toggle autoconnect",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Connect           => "🔗",
            Self::Disconnect        => "⏏️",
            Self::Pair              => "🤝",
            Self::Remove            => "🗑️",
            Self::ToggleAutoconnect => "✦",
        }
    }

    pub fn accent(&self) -> Color {
        match self {
            Self::Connect           => GREEN,
            Self::Disconnect        => AMBER,
            Self::Pair              => BLUE,
            Self::Remove            => RED,
            Self::ToggleAutoconnect => TEAL,
        }
    }
}

pub fn available_actions(dev: &BtDevice) -> Vec<DeviceAction> {
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

pub enum Popup {
    None,
    ActionMenu { device_idx: usize, selected: usize },
    Confirm { device_idx: usize, action: DeviceAction },
    Working { device_idx: usize, action: DeviceAction },
    Message { text: String, ok: bool },
}

pub struct App {
    pub devices: Vec<BtDevice>,
    pub list_state: ListState,
    pub popup: Popup,
    pub loading: bool,
    pub error: Option<String>,
    pub adapter_path: Option<String>,
    pub adapter_name: Option<String>,
    pub adapter_address: Option<String>,
    pub scanning: bool,
}

impl App {
    pub fn new() -> Self {
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

    pub fn move_up(&mut self) {
        if self.devices.is_empty() { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
    }

    pub fn move_down(&mut self) {
        if self.devices.is_empty() { return; }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1).min(self.devices.len() - 1)));
    }

    pub fn selected_device(&self) -> Option<&BtDevice> {
        self.list_state.selected().and_then(|i| self.devices.get(i))
    }
}
