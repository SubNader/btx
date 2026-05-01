#!/bin/sh
set -e

REPO="SubNader/btx"
BIN_DIR="${HOME}/.local/bin"
SERVICE_DIR="${HOME}/.config/systemd/user"

arch=$(uname -m)
case "$arch" in
  x86_64)  ARCH="x86_64" ;;
  aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac

BASE="https://github.com/${REPO}/releases/latest/download"

mkdir -p "$BIN_DIR" "$SERVICE_DIR"

echo "Downloading btx ($ARCH)..."
curl -fsSL "${BASE}/btx-${ARCH}"         -o "${BIN_DIR}/btx"
curl -fsSL "${BASE}/btx-connect-${ARCH}" -o "${BIN_DIR}/btx-connect"
chmod +x "${BIN_DIR}/btx" "${BIN_DIR}/btx-connect"

cat > "${SERVICE_DIR}/btx-connect.service" <<'EOF'
[Unit]
Description=btx connect trusted Bluetooth devices at startup
After=bluetooth.target graphical-session.target
Wants=bluetooth.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/btx-connect
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
EOF

systemctl --user daemon-reload
systemctl --user enable btx-connect.service

echo "Done. Make sure ${BIN_DIR} is in your PATH, then run: btx"
