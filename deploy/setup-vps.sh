#!/usr/bin/env bash
# First-time setup script for brai on a fresh Ubuntu VPS.
# Run as: bash setup-vps.sh
set -euo pipefail

USER_NAME="${SUDO_USER:-ubuntu}"
BRAI_CONFIG_DIR="/home/$USER_NAME/.brai"
STORAGE_DIR="/home/$USER_NAME/brai-storage"
APPS_DIR="/home/$USER_NAME/apps"

echo "[setup] Creating directories..."
sudo -u "$USER_NAME" mkdir -p \
    "$BRAI_CONFIG_DIR/data" \
    "$BRAI_CONFIG_DIR/sops" \
    "$STORAGE_DIR/cvs" \
    "$APPS_DIR"

echo "[setup] Creating /etc/brai/ for secrets..."
sudo mkdir -p /etc/brai
if [ ! -f /etc/brai/env ]; then
    echo "[setup] Copying env template to /etc/brai/env — fill in your secrets!"
    sudo cp "$(dirname "$0")/brai.env.example" /etc/brai/env
    sudo chmod 600 /etc/brai/env
    sudo chown "root:$USER_NAME" /etc/brai/env
fi

echo "[setup] Installing systemd service..."
sudo cp "$(dirname "$0")/brai.service" /etc/systemd/system/brai.service
# Substitute actual username in service file
sudo sed -i "s/User=ubuntu/User=$USER_NAME/g" /etc/systemd/system/brai.service
sudo sed -i "s/Group=ubuntu/Group=$USER_NAME/g" /etc/systemd/system/brai.service
sudo sed -i "s|/home/ubuntu|/home/$USER_NAME|g" /etc/systemd/system/brai.service

sudo systemctl daemon-reload
sudo systemctl enable brai

echo ""
echo "[setup] Done. Next steps:"
echo "  1. Edit /etc/brai/env — fill in TELEGRAM_BOT_TOKEN, OPENROUTER_API_KEY, etc."
echo "  2. Copy your brai config: nano $BRAI_CONFIG_DIR/config.toml"
echo "  3. Copy SOPs: cp -r /path/to/repo/sops/* $BRAI_CONFIG_DIR/sops/"
echo "  4. Start: sudo systemctl start brai"
echo "  5. Check: sudo journalctl -u brai -f"
