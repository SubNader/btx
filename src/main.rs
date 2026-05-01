use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use zbus::Connection;

mod bluez;
mod model;
mod palette;
mod ui;

use bluez::{
    AdapterProxy, connect_device, disconnect_device, fetch_devices, find_adapter_path,
    pair_device, remove_device, set_trusted, start_discovery, stop_discovery,
};
use model::{App, DeviceAction, Popup, available_actions};
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
                // Any other error: silently skip auto-scan; user can still press 's' manually
            }
        }
        app.adapter_path = Some(path);
    }

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

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
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if app.scanning {
                            if let Some(ref adapter) = app.adapter_path.clone() {
                                let _ = stop_discovery(&conn, adapter).await;
                            }
                            app.scanning = false;
                        } else {
                            break;
                        }
                    }
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
                        if !app.scanning {
                            if let Some(ref adapter) = app.adapter_path.clone() {
                                match start_discovery(&conn, adapter).await {
                                    Ok(()) => {
                                        app.scanning = true;
                                    }
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

                Popup::Message { .. } => { app.popup = Popup::None; }

                Popup::Working { .. } => {}
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
