use tokio::sync::{mpsc, oneshot};
use zbus::{interface, zvariant::OwnedObjectPath};

/// Requests the agent sends to the main UI loop.
pub enum AgentRequest {
    RequestPinCode {
        device: String,
        reply: oneshot::Sender<Result<String, ()>>,
    },
    RequestPasskey {
        device: String,
        reply: oneshot::Sender<Result<u32, ()>>,
    },
    DisplayPasskey {
        device: String,
        passkey: u32,
        reply: oneshot::Sender<()>,
    },
    DisplayPinCode {
        device: String,
        pin: String,
        reply: oneshot::Sender<()>,
    },
    RequestConfirmation {
        device: String,
        passkey: u32,
        reply: oneshot::Sender<Result<(), ()>>,
    },
    RequestAuthorization {
        device: String,
        reply: oneshot::Sender<Result<(), ()>>,
    },
}

pub struct Agent {
    tx: mpsc::UnboundedSender<AgentRequest>,
}

impl Agent {
    pub fn new(tx: mpsc::UnboundedSender<AgentRequest>) -> Self {
        Self { tx }
    }
}

#[interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn release(&self) {}

    async fn request_pin_code(&self, device: OwnedObjectPath) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::RequestPinCode {
            device: device.to_string(),
            reply: tx,
        });
        match rx.await {
            Ok(Ok(pin)) => Ok(pin),
            _ => Err(zbus::fdo::Error::Failed("cancelled".into())),
        }
    }

    async fn display_pin_code(&self, device: OwnedObjectPath, pincode: String) -> zbus::fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::DisplayPinCode {
            device: device.to_string(),
            pin: pincode,
            reply: tx,
        });
        let _ = rx.await;
        Ok(())
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> zbus::fdo::Result<u32> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::RequestPasskey {
            device: device.to_string(),
            reply: tx,
        });
        match rx.await {
            Ok(Ok(pk)) => Ok(pk),
            _ => Err(zbus::fdo::Error::Failed("cancelled".into())),
        }
    }

    async fn display_passkey(&self, device: OwnedObjectPath, passkey: u32, _entered: u16) -> zbus::fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::DisplayPasskey {
            device: device.to_string(),
            passkey,
            reply: tx,
        });
        let _ = rx.await;
        Ok(())
    }

    async fn request_confirmation(&self, device: OwnedObjectPath, passkey: u32) -> zbus::fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::RequestConfirmation {
            device: device.to_string(),
            passkey,
            reply: tx,
        });
        match rx.await {
            Ok(Ok(())) => Ok(()),
            _ => Err(zbus::fdo::Error::Failed("rejected".into())),
        }
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> zbus::fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(AgentRequest::RequestAuthorization {
            device: device.to_string(),
            reply: tx,
        });
        match rx.await {
            Ok(Ok(())) => Ok(()),
            _ => Err(zbus::fdo::Error::Failed("rejected".into())),
        }
    }

    async fn authorize_service(&self, _device: OwnedObjectPath, _uuid: String) -> zbus::fdo::Result<()> {
        Ok(())
    }

    async fn cancel(&self) {}
}

pub const AGENT_PATH: &str = "/org/btx/Agent";
pub const AGENT_CAPABILITY: &str = "KeyboardDisplay";

/// Register the agent with BlueZ AgentManager. Returns the connection that
/// keeps the agent D-Bus service alive — drop it to unregister.
pub async fn register_agent(
    tx: mpsc::UnboundedSender<AgentRequest>,
) -> anyhow::Result<zbus::Connection> {
    let conn = zbus::connection::Builder::system()?
        .name("org.btx.agent")?
        .serve_at(AGENT_PATH, Agent::new(tx))?
        .build()
        .await?;

    let manager = zbus::Proxy::new(
        &conn,
        "org.bluez",
        "/org/bluez",
        "org.bluez.AgentManager1",
    )
    .await?;

    manager
        .call_method(
            "RegisterAgent",
            &(
                zbus::zvariant::ObjectPath::try_from(AGENT_PATH)?,
                AGENT_CAPABILITY,
            ),
        )
        .await?;

    manager
        .call_method(
            "RequestDefaultAgent",
            &(zbus::zvariant::ObjectPath::try_from(AGENT_PATH)?,),
        )
        .await?;

    Ok(conn)
}
