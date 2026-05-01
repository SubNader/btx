# btx

Terminal UI for managing Bluetooth devices on Linux via BlueZ.

![btx screenshot placeholder](https://via.placeholder.com/800x400?text=btx)

## Features

- Connect, disconnect, and pair devices
- Toggle trusted (autoconnect) per device
- Battery level display
- Signal strength indicator
- Scan for nearby devices
- `btx-connect` — headless startup connector for trusted devices

## Requirements

- Linux with BlueZ (`bluetoothd` running)
- Rust toolchain (`cargo`)
- D-Bus system bus access

## Install

```sh
./setup.sh
```

This builds both binaries, installs them to `~/.local/bin`, and enables the `btx-connect` startup service.

## Usage

```sh
btx
```

| Key        | Action                        |
|------------|-------------------------------|
| `↑↓` / `jk` | Navigate devices             |
| `Enter`    | Open action menu              |
| `s`        | Scan for nearby devices       |
| `r`        | Refresh device list           |
| `q` / `Esc`| Quit                          |

### Action menu

| Action            | Description                                      |
|-------------------|--------------------------------------------------|
| Connect           | Connect to the selected device                   |
| Disconnect        | Disconnect the selected device                   |
| Pair              | Initiate pairing (device must be in pairing mode)|
| Toggle autoconnect| Mark/unmark device as trusted for startup connect|
| Remove / unpair   | Remove device (must re-pair to use again)        |

## Startup connect

`btx-connect` runs at login via a systemd user service. It connects all paired + trusted devices automatically.

```sh
# Check last run
journalctl --user -u btx-connect.service

# Disable
systemctl --user disable btx-connect.service
```

Devices are marked trusted from within `btx` using the **Toggle autoconnect** action.

## Uninstall

```sh
systemctl --user disable --now btx-connect.service
rm ~/.local/bin/btx ~/.local/bin/btx-connect
rm ~/.config/systemd/user/btx-connect.service
systemctl --user daemon-reload
```
