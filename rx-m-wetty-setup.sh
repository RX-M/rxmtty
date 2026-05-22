#!/bin/env sh
#
# Usage: sudo ./setup.sh [desired-ubuntu-password]
#
# This script installs rxm-wetty as a daemon listening on port 80 on a new
# (untampered with) standard RX-M Ubuntu lab system. To connect to the system
# browse to: http://<ip-of-lab-vm>/wetty
#
# Then login with:
#     user: ubuntu
#     password: rx-myyyymmdd (e.g. rx-m20260127) --or--
#               <password provided on cli>
#
# This is a fast WebSocket based terminal solution for those who cannot use
# ssh. Note that installing rxm-wetty does not disable standard ssh/key based
# login support.
#
# Caveats:
# ==============================================
# 1. This default installer uses HTTP on port 80. Put rxm-wetty behind TLS or
#    pass --ssl-cert/--ssl-key in the systemd unit if HTTPS is required.
# 2. SFTP will not work, this is not ssh. File uploads can be made by copying
#    files from the browser machine to a cloud location (e.g. github) and then
#    pulling the file down with wget from the lab box, for example.
# 3. This solution does not support X11 so you can not forward GUI windows
#    over this connection. Any GUIs used on the lab system will have to be
#    web servers accessed with new browser tabs remotely.

set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo "ERROR: run as root (e.g. sudo ./setup.sh [password])" >&2
  exit 1
fi

PASS="${1:-rx-m$(date +%Y%m%d)}"

WETTY_PORT="${WETTY_PORT:-80}"
WETTY_HOST="0.0.0.0"
WETTY_BASE="/wetty"
PUB_IP=$(curl -s https://icanhazip.com)
WETTY_BIN="/usr/local/bin/rxm-wetty"


echo "[1/6] Updating apt + installing prerequisites..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -y
apt-get install -y --no-install-recommends ca-certificates curl git openssh-client pkg-config rustc cargo


echo "[2/6] Setting password for user ubuntu..."
echo "ubuntu:${PASS}" | chpasswd


echo "[3/6] Enabling SSH password auth..."
sed -i 's/^#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
systemctl restart ssh


echo "[4/6] Building rxm-wetty from Rust source..."
cargo build --manifest-path "./Cargo.toml" --release
install -m 0755 "./target/release/rxm-wetty" "${WETTY_BIN}"


echo "[5/6] Creating systemd service for wetty..."
cat > /etc/systemd/system/wetty.service <<EOF
# systemd unit file /etc/systemd/system/wetty.service
[Unit]
Description=RX-M Wetty Web Terminal
After=ssh.service
[Service]
Type=simple
WorkingDirectory=/root
ExecStart=${WETTY_BIN} -p ${WETTY_PORT} --host ${WETTY_HOST} --base ${WETTY_BASE} --ssh-host 127.0.0.1
TimeoutStopSec=20
KillMode=mixed
Restart=always
RestartSec=2
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable --now wetty


echo "[6/6] Done."
echo "------------------------------------------------------------"
echo "Wetty URL:      http://${PUB_IP}:${WETTY_PORT}${WETTY_BASE}"
echo "Login user:     ubuntu"
echo "Login password: ${PASS}"
echo "Service status: systemctl status wetty --no-pager -l"
echo "Logs:           journalctl -u wetty -f"
echo "------------------------------------------------------------"
