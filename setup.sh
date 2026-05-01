#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="$HOME/.local/bin"
SERVICE_DIR="$HOME/.config/systemd/user"
SERVICE_NAME="btx-connect.service"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Building..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

echo "==> Installing binaries to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cp "$SCRIPT_DIR/target/release/btx"         "$INSTALL_DIR/btx"
cp "$SCRIPT_DIR/target/release/btx-connect" "$INSTALL_DIR/btx-connect"
chmod +x "$INSTALL_DIR/btx" "$INSTALL_DIR/btx-connect"

echo "==> Installing systemd user service..."
mkdir -p "$SERVICE_DIR"
cat > "$SERVICE_DIR/$SERVICE_NAME" <<EOF
[Unit]
Description=btx connect trusted Bluetooth devices at startup
After=bluetooth.target graphical-session.target
Wants=bluetooth.target

[Service]
Type=oneshot
ExecStart=$INSTALL_DIR/btx-connect
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
EOF

systemctl --user daemon-reload
systemctl --user enable "$SERVICE_NAME"

echo ""
echo "Done."
echo "  btx            — launch the TUI"
echo "  btx-connect    — connect trusted devices (runs at login via systemd)"
echo ""
echo "To check startup connect logs:"
echo "  journalctl --user -u btx-connect.service"
