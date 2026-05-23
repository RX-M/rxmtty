#!/bin/env sh
#
# Usage: sudo ./setup.sh [desired-ubuntu-password]
#
# This script installs rxmtty as a daemon listening on port 80 on a clean
# standard Ubuntu 24.04 server. To connect to the system browse to: 
# http://<ip-of-lab-vm>/tty
#
# Then login (user is set to ubuntu) with password: rx-myyyymmdd (e.g. rx-m20260127) 
# --or--
# <password provided on cli>
#
# This is a fast Rust/WebSocket based terminal solution for those who cannot use
# ssh. Note that installing rxmtty does not disable standard ssh/key based
# login support.
#
# Caveats:
# ==============================================
# 1. This installer uses HTTP on port 80. To run rxmtty with TLS pass the
#    --ssl-cert/--ssl-key switches in the systemd unit.
# 2. SFTP will not work.
# 3. This solution does not support X11.

set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo "ERROR: run as root (e.g. sudo ./setup.sh [password])" >&2
  exit 1
fi

PASS="${1:-rx-m$(date +%Y%m%d)}"
USER="ubuntu"

RXMTTY_PORT="${RXMTTY_PORT:-80}"
RXMTTY_HOST="0.0.0.0"
RXMTTY_BASE="/tty"
PUB_IP=$(curl -s https://icanhazip.com)
RXMTTY_BIN="/usr/local/bin/rxmtty"


echo "[1/5] Updating apt + installing prerequisites..."
export DEBIAN_FRONTEND=noninteractive
apt-get update -y
sudo apt install build-essential -y
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"


echo "[2/5] Setting password for user ${USER}..."
echo "${USER}:${PASS}" | chpasswd
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
systemctl restart ssh


echo "[3/5] Building rxmtty from Rust source..."
git clone https://github.com/RX-M/rxmtty /opt/rxmtty
cargo build --manifest-path /opt/rxmtty/Cargo.toml --release
install -m 0755 /opt/rxmtty/target/release/rxmtty ${RXMTTY_BIN}


echo "[4/5] Creating systemd service for rxmtty..."
cat > /etc/systemd/system/rxmtty.service <<EOF
# systemd unit file /etc/systemd/system/rxmtty.service
[Unit]
Description=RX-M Web Terminal
After=ssh.service
[Service]
Type=simple
WorkingDirectory=/root
ExecStart=${RXMTTY_BIN} -p ${RXMTTY_PORT} --host ${RXMTTY_HOST} --base ${RXMTTY_BASE} --ssh-host 127.0.0.1 --ssh-user ${USER}
TimeoutStopSec=20
KillMode=mixed
Restart=always
RestartSec=2
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable --now rxmtty


echo "[5/5] Done."
echo "------------------------------------------------------------"
echo "RX-M Web Terminal URL:      http://${PUB_IP}:${RXMTTY_PORT}${RXMTTY_BASE}"
echo "Login user:     ${USER}"
echo "Login password: ${PASS}"
echo "Service status: systemctl status rxmtty --no-pager -l"
echo "Logs:           journalctl -u rxmtty -f"
echo "------------------------------------------------------------"
