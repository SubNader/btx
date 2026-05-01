use std::collections::HashMap;
use std::time::Duration;

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

const CONNECT_TIMEOUT_SECS: u64 = 10;

async fn try_connect_one(conn: &Connection, path: &str) -> Result<(), String> {
    let proxy = DeviceProxy::builder(conn)
        .path(path)
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;

    tokio::time::timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS), proxy.connect())
        .await
        .map_err(|_| format!("timed out after {CONNECT_TIMEOUT_SECS}s"))?
        .map_err(|e| e.to_string())
}

async fn connect_trusted(conn: &Connection) {
    let manager = match ObjectManagerProxy::new(conn).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("btx-connect: failed to reach BlueZ — {e}");
            return;
        }
    };

    let objects = match manager.get_managed_objects().await {
        Ok(o) => o,
        Err(e) => {
            eprintln!("btx-connect: bluetoothd not ready — {e}");
            return;
        }
    };

    let mut targets: Vec<(String, String, String)> = Vec::new();

    for (path, interfaces) in &objects {
        if !interfaces.contains_key("org.bluez.Device1") {
            continue;
        }

        let proxy = match DeviceProxy::builder(conn).path(path.as_ref()) {
            Ok(b) => match b.build().await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("btx-connect: skipping {path} — {e}");
                    continue;
                }
            },
            Err(e) => {
                eprintln!("btx-connect: bad path {path} — {e}");
                continue;
            }
        };

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
        return;
    }

    for (path, name, address) in targets {
        print!("btx-connect: connecting {name} ({address}) … ");
        match try_connect_one(conn, &path).await {
            Ok(()) => println!("ok"),
            Err(e) => eprintln!("failed: {e}"),
        }
    }
}

#[tokio::main]
async fn main() {
    // Give bluetoothd a moment to settle after system boot
    tokio::time::sleep(Duration::from_secs(2)).await;

    match Connection::system().await {
        Ok(conn) => connect_trusted(&conn).await,
        Err(e) => eprintln!("btx-connect: cannot connect to D-Bus — {e}"),
    }
}
