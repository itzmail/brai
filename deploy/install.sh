#!/usr/bin/env bash
# Installs/upgrades brai binary and (re)starts systemd service.
# Expects /tmp/brai-new to exist (uploaded by CI).
set -euo pipefail

BINARY_DST="/usr/local/bin/brai"
SERVICE_NAME="brai"
NEW_BINARY="/tmp/brai-new"

echo "[brai-install] Starting deployment..."

# Backup current binary if exists
if [ -f "$BINARY_DST" ]; then
    cp "$BINARY_DST" "${BINARY_DST}.bak"
    echo "[brai-install] Backed up existing binary to ${BINARY_DST}.bak"
fi

# Install new binary
sudo install -o root -g root -m 755 "$NEW_BINARY" "$BINARY_DST"
rm -f "$NEW_BINARY"
echo "[brai-install] Installed binary to $BINARY_DST"

# Reload systemd and restart service
sudo systemctl daemon-reload
sudo systemctl restart "$SERVICE_NAME"
sleep 3

# Verify service is running
if sudo systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "[brai-install] Service $SERVICE_NAME is running."
else
    echo "[brai-install] ERROR: Service $SERVICE_NAME failed to start. Rolling back..."
    if [ -f "${BINARY_DST}.bak" ]; then
        sudo install -o root -g root -m 755 "${BINARY_DST}.bak" "$BINARY_DST"
        sudo systemctl restart "$SERVICE_NAME"
        echo "[brai-install] Rollback complete."
    fi
    exit 1
fi

echo "[brai-install] Deployment complete."
