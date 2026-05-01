use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use zbus::Connection;

mod agent;
mod bluez;
mod model;
mod palette;
mod ui;

use agent::{AgentRequest, register_agent};
use bluez::{
    AdapterProxy, connect_device, disconnect_device, fetch_devices, find_adapter_path,
    pair_device, remove_device, set_trusted, start_discovery, stop_discovery,
};
use model::{AgentReply, App, DeviceAction, Popup, available_actions};
use ui::ui;

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let conn = Connection::system().await.context("Cannot connect to D-Bus system bus")?;
    let mut app = App::new();

    // Start the BlueZ agent so we can handle PIN/passkey requests during pairing.
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<AgentRequest>();
    let _agent_conn = register_agent(agent_tx).await.ok();

    terminal.draw(|f| ui(f, &mut app))?;

    match fetch_devices(&conn).await {
        Ok(devs) => { app.devices = devs; app.loading = false; }
        Err(e)   => { app.loading = false; app.error = Some(e.to_string()); }
    }
    if let Ok(path) = find_adapter_path(&conn).await {
        if let Ok(proxy) = AdapterProxy::builder(&conn).path(path.as_str())?.build().await {
            app.adapter_name    = proxy.name().await.ok();
            app.adapter_address = proxy.address().await.ok();
        }
        // Auto-start discovery on launch; ignore "Already discovering" and similar non-fatal errors
        match start_discovery(&conn, &path).await {
            Ok(()) => {
                app.scanning = true;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Already") || msg.contains("InProgress") {
                    app.scanning = true;
                }
            }
        }
        app.adapter_path = Some(path);
    }

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // Drain any pending agent requests before blocking on input.
        while let Ok(req) = agent_rx.try_recv() {
            handle_agent_request(&mut app, req);
        }

        let poll_ms = if app.scanning { 1000 } else { 150 };

        if !event::poll(Duration::from_millis(poll_ms))? {
            if app.scanning {
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
                Popup::None => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Up   | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),

                    KeyCode::Char('s') => {
                        if app.scanning {
                            if let Some(ref adapter) = app.adapter_path.clone() {
                                let _ = stop_discovery(&conn, adapter).await;
                            }
                            app.scanning = false;
                        } else if let Some(ref adapter) = app.adapter_path.clone() {
                            match start_discovery(&conn, adapter).await {
                                Ok(()) => { app.scanning = true; }
                                Err(e) => {
                                    let msg = e.to_string();
                                    if msg.contains("Already") || msg.contains("InProgress") {
                                        app.scanning = true;
                                    } else {
                                        app.popup = Popup::Message { text: format!("scan failed: {e}"), ok: false };
                                    }
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

                                let result: Result<String> = async {
                                    match action {
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
                                    }
                                }
                                .await;

                                // During pairing the agent may have set a popup (PinInput etc).
                                // Only overwrite with a result message if we're still in Working state.
                                if matches!(app.popup, Popup::Working { .. }) {
                                    if let Ok(devs) = fetch_devices(&conn).await {
                                        let old = app.list_state.selected().unwrap_or(0);
                                        app.devices = devs;
                                        app.list_state.select(Some(old.min(app.devices.len().saturating_sub(1))));
                                    }
                                    app.popup = match result {
                                        Ok(msg)  => Popup::Message { text: msg, ok: true },
                                        Err(e)   => Popup::Message { text: e.to_string(), ok: false },
                                    };
                                }
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

                Popup::Message { .. } => { app.popup = Popup::None; }

                Popup::Working { .. } => {
                    // While working, still drain agent requests so they can replace this popup.
                    while let Ok(req) = agent_rx.try_recv() {
                        handle_agent_request(&mut app, req);
                    }
                }

                Popup::PinInput { .. } => {
                    let Popup::PinInput { device, input } = &app.popup else { unreachable!() };
                    let device = device.clone();
                    let mut input = input.clone();
                    match key.code {
                        KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '-' => {
                            if input.len() < 16 { input.push(c); }
                            app.popup = Popup::PinInput { device, input };
                        }
                        KeyCode::Backspace => {
                            input.pop();
                            app.popup = Popup::PinInput { device, input };
                        }
                        KeyCode::Enter => {
                            if let Some(AgentReply::PinCode(tx)) = app.agent_reply.take() {
                                let _ = tx.send(Ok(input));
                            }
                            app.popup = Popup::None;
                        }
                        KeyCode::Esc => {
                            if let Some(AgentReply::PinCode(tx)) = app.agent_reply.take() {
                                let _ = tx.send(Err(()));
                            }
                            app.popup = Popup::None;
                        }
                        _ => {}
                    }
                }

                Popup::PasskeyInput { .. } => {
                    let Popup::PasskeyInput { device, input } = &app.popup else { unreachable!() };
                    let device = device.clone();
                    let mut input = input.clone();
                    match key.code {
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            if input.len() < 6 { input.push(c); }
                            app.popup = Popup::PasskeyInput { device, input };
                        }
                        KeyCode::Backspace => {
                            input.pop();
                            app.popup = Popup::PasskeyInput { device, input };
                        }
                        KeyCode::Enter => {
                            if let Some(AgentReply::Passkey(tx)) = app.agent_reply.take() {
                                let pk = input.parse::<u32>().unwrap_or(0);
                                let _ = tx.send(Ok(pk));
                            }
                            app.popup = Popup::None;
                        }
                        KeyCode::Esc => {
                            if let Some(AgentReply::Passkey(tx)) = app.agent_reply.take() {
                                let _ = tx.send(Err(()));
                            }
                            app.popup = Popup::None;
                        }
                        _ => {}
                    }
                }

                Popup::ConfirmPasskey { .. } => {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            if let Some(AgentReply::Confirm(tx)) = app.agent_reply.take() {
                                let _ = tx.send(Ok(()));
                            }
                            app.popup = Popup::None;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            if let Some(AgentReply::Confirm(tx)) = app.agent_reply.take() {
                                let _ = tx.send(Err(()));
                            }
                            app.popup = Popup::None;
                        }
                        _ => {}
                    }
                }

                Popup::DisplayPasskey { .. } => {
                    // Any key dismisses; the reply channel just unblocks the agent.
                    if let Some(AgentReply::Display(tx)) = app.agent_reply.take() {
                        let _ = tx.send(());
                    }
                    app.popup = Popup::None;
                }
            }
        }
    }

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

fn handle_agent_request(app: &mut App, req: AgentRequest) {
    match req {
        AgentRequest::RequestPinCode { device, reply } => {
            app.agent_reply = Some(AgentReply::PinCode(reply));
            app.popup = Popup::PinInput { device, input: String::new() };
        }
        AgentRequest::RequestPasskey { device, reply } => {
            app.agent_reply = Some(AgentReply::Passkey(reply));
            app.popup = Popup::PasskeyInput { device, input: String::new() };
        }
        AgentRequest::DisplayPasskey { device, passkey, reply } => {
            app.agent_reply = Some(AgentReply::Display(reply));
            app.popup = Popup::DisplayPasskey { device, passkey: format!("{:06}", passkey) };
        }
        AgentRequest::DisplayPinCode { device, pin, reply } => {
            app.agent_reply = Some(AgentReply::Display(reply));
            app.popup = Popup::DisplayPasskey { device, passkey: pin };
        }
        AgentRequest::RequestConfirmation { device, passkey, reply } => {
            app.agent_reply = Some(AgentReply::Confirm(reply));
            app.popup = Popup::ConfirmPasskey { device, passkey };
        }
        AgentRequest::RequestAuthorization { device, reply } => {
            // Auto-approve authorization (user already chose to pair).
            let _ = reply.send(Ok(()));
            let _ = device;
        }
    }
}
