use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.bluez",
    default_path = "/"
)]
trait ObjectManager {
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        HashMap<
            zbus::zvariant::OwnedObjectPath,
            HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>,
        >,
    >;
}

#[proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
trait Device {
    fn connect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn alias(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn paired(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn trusted(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;
}

async fn connect_trusted(conn: &Connection) -> Result<()> {
    let manager = ObjectManagerProxy::new(conn)
        .await
        .context("Failed to connect to BlueZ")?;

    let objects = manager
        .get_managed_objects()
        .await
        .context("BlueZ returned no objects — is bluetoothd running?")?;

    let mut targets = Vec::new();

    for (path, interfaces) in &objects {
        if !interfaces.contains_key("org.bluez.Device1") {
            continue;
        }
        let proxy = DeviceProxy::builder(conn)
            .path(path.as_ref())?
            .build()
            .await?;

        let trusted   = proxy.trusted().await.unwrap_or(false);
        let paired    = proxy.paired().await.unwrap_or(false);
        let connected = proxy.connected().await.unwrap_or(false);

        if trusted && paired && !connected {
            let address = proxy.address().await.unwrap_or_default();
            let name = match proxy.alias().await {
                Ok(a) if !a.is_empty() => a,
                _ => proxy.name().await.unwrap_or_else(|_| address.clone()),
            };
            targets.push((path.to_string(), name, address));
        }
    }

    if targets.is_empty() {
        println!("btx-connect: no trusted devices to connect");
        return Ok(());
    }

    for (path, name, address) in targets {
        print!("btx-connect: connecting {} ({}) … ", name, address);
        let proxy = DeviceProxy::builder(conn).path(path.as_str())?.build().await?;
        match proxy.connect().await {
            Ok(()) => println!("ok"),
            Err(e) => println!("failed: {}", e),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Give bluetoothd a moment to settle after system boot
    tokio::time::sleep(Duration::from_secs(2)).await;

    let conn = Connection::system()
        .await
        .context("Cannot connect to D-Bus system bus")?;

    connect_trusted(&conn).await
}
