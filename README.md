# btx

> Terminal Bluetooth manager for Linux — connect, pair, and manage devices from the keyboard.

[![Release](https://img.shields.io/github/v/release/SubNader/btx)](https://github.com/SubNader/btx/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#)
[![Platform](https://img.shields.io/badge/platform-Linux-lightgrey)](https://github.com/SubNader/btx)

```
  📶 btx  bluetooth manager
     Intel Bluetooth  A0:B1:C2:D3:E4:F5

 ▌ ●  🎧  Galaxy Buds+ (E5CF)      ✦ auto    🔋 84%
   ●  ⌨️  Keychron K3               ✦ auto    🔋 61%
   ○  📱  iPhone                    · no auto
   ○  🖱️  MX Master 3               · no auto

     ↑↓/jk navigate  Enter actions  s scan  q quit
```

## Features

- **Connect / disconnect / pair** devices interactively
- **Toggle autoconnect** — mark devices as trusted; they reconnect at every login
- **Battery level** and **signal strength** display
- **Scan** for nearby unpaired devices
- **`btx-connect`** — headless startup service that connects all trusted devices automatically

## Install

**From a release `.deb`** (recommended):

```sh
curl -LO https://github.com/SubNader/btx/releases/latest/download/btx_<version>_amd64.deb
sudo dpkg -i btx_<version>_amd64.deb
```

**From source:**

```sh
git clone https://github.com/SubNader/btx
cd btx
./setup.sh
```

Requires: Rust toolchain, `bluetoothd` running, D-Bus system bus access.

## Usage

```sh
btx
```

### Keys

| Key | Action |
|-----|--------|
| `↑` `↓` / `j` `k` | Navigate devices |
| `Enter` | Open action menu |
| `s` | Scan for nearby devices |
| `r` | Refresh |
| `q` / `Esc` | Quit |

### Action menu

| Action | Description |
|--------|-------------|
| 🔗 Connect | Connect the selected device |
| ⏏️ Disconnect | Disconnect the selected device |
| 🤝 Pair | Pair a new device (put it in pairing mode first) |
| ✦ Toggle autoconnect | Mark/unmark as trusted for startup reconnect |
| 🗑️ Remove / unpair | Remove device — must re-pair to use again |

## Startup connect

`btx-connect` runs at login as a systemd user service and connects all trusted devices automatically.

```
● btx-connect: connecting Galaxy Buds+ (E5CF) (34:82:C5:D4:E5:CF) … ok
● btx-connect: connecting Keychron K3 (AA:BB:CC:DD:EE:FF) … ok
```

```sh
# Check logs
journalctl --user -u btx-connect.service

# Disable
systemctl --user disable btx-connect.service
```

Mark a device as trusted from within `btx` using **Toggle autoconnect**.

## Uninstall

```sh
systemctl --user disable --now btx-connect.service
rm ~/.local/bin/btx ~/.local/bin/btx-connect
rm ~/.config/systemd/user/btx-connect.service
systemctl --user daemon-reload
```
