# rxm-wetty
RX-M wetty build for lab boxes, rewritten as a Rust service instead of a
Node/npm application.

`rxm-wetty` serves a browser terminal at `/wetty` and upgrades the browser
connection to WebSocket. Each browser session is bridged to a local PTY running
`ssh ubuntu@127.0.0.1` by default, so students still log in with the normal Linux account and
password while the server side no longer depends on Node, npm, or the
`wettyoss/wetty` container.

## Build

```bash
cargo build --release
```

The binary is created at:

```bash
target/release/rxm-wetty
```

## Run

```bash
sudo ./target/release/rxm-wetty -p 80 --host 0.0.0.0 --base /wetty
```

Useful options:

```text
-p, --port <PORT>          Listening port, default 80
--host <HOST>              Listening address, default 0.0.0.0
--base <PATH>              Browser base path, default /wetty
--ssh-host <HOST>          SSH target, default 127.0.0.1
--ssh-user <USER>          SSH login user, default ubuntu
--ssh-port <PORT>          SSH target port, default 22
--ssl-cert <PATH>          Optional TLS certificate PEM
--ssl-key <PATH>           Optional TLS private key PEM
--command <COMMAND>        Run a shell command instead of local ssh
```

## EC2 User Data

The following script can be used when standing up EC2 instances. Drop this in the
`Advanced details` -> `User data` field and the instance will build and run
`rxm-wetty`.

```bash
#!/bin/bash

until curl -s --head http://169.254.169.254/latest/meta-data/ >/dev/null; do
  echo "Waiting for metadata service..."
  sleep 2
done

apt-get update -y
apt-get install -y --no-install-recommends ca-certificates curl git openssh-client pkg-config rustc cargo
PASS="${1:-rx-m$(date +%Y%m%d)}"
echo "ubuntu:${PASS}" | chpasswd
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
systemctl restart ssh
git clone https://github.com/rx-m/wetty.git /opt/rxm-wetty
cargo build --manifest-path /opt/rxm-wetty/Cargo.toml --release
install -m 0755 /opt/rxm-wetty/target/release/rxm-wetty /usr/local/bin/rxm-wetty
/usr/local/bin/rxm-wetty -p 80 --host 0.0.0.0 --base /wetty
```
