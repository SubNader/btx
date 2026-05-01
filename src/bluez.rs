use anyhow::{Context, Result};
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.bluez",
    default_path = "/"
)]
pub trait ObjectManager {
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
pub trait Device {
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
pub trait Battery {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<u8>;
}

#[proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
pub trait Adapter {
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

use crate::model::BtDevice;

pub async fn fetch_devices(conn: &Connection) -> Result<Vec<BtDevice>> {
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

pub async fn find_adapter_path(conn: &Connection) -> Result<String> {
    let manager = ObjectManagerProxy::new(conn).await?;
    let objects = manager.get_managed_objects().await?;
    for (path, interfaces) in &objects {
        if interfaces.contains_key("org.bluez.Adapter1") {
            return Ok(path.to_string());
        }
    }
    anyhow::bail!("no bluetooth adapter found")
}

pub async fn set_trusted(conn: &Connection, path: &str, trusted: bool) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.set_trusted(trusted).await?;
    Ok(())
}

pub async fn connect_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.connect().await?;
    Ok(())
}

pub async fn disconnect_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.disconnect().await?;
    Ok(())
}

pub async fn pair_device(conn: &Connection, path: &str) -> Result<()> {
    let proxy = DeviceProxy::builder(conn).path(path)?.build().await?;
    proxy.pair().await?;
    Ok(())
}

pub async fn remove_device(conn: &Connection, adapter_path: &str, device_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    let path = zbus::zvariant::ObjectPath::try_from(device_path)?;
    proxy.remove_device(path).await?;
    Ok(())
}

pub async fn start_discovery(conn: &Connection, adapter_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    proxy.start_discovery().await?;
    Ok(())
}

pub async fn stop_discovery(conn: &Connection, adapter_path: &str) -> Result<()> {
    let proxy = AdapterProxy::builder(conn).path(adapter_path)?.build().await?;
    proxy.stop_discovery().await?;
    Ok(())
}
