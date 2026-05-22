![RX-M LLC](https://rx-m.com/rxm-cnc.svg)

# rxm-wetty setup for AWS

## What is rxm-wetty?

rxm-wetty is an RX-M browser-to-Linux-terminal service implemented in Rust. It listens on a designated
port and serves a browser terminal interface. The browser connects over HTTP or HTTPS, upgrades to
WebSocket, and the service bridges the WebSocket session to a local PTY running `ssh 127.0.0.1`.

The older upstream wetty service and container are Node/npm based. This repo now avoids the Node
runtime, global npm installs, and the npm package supply chain for the server-side component.

## Why not use SSH directly?

In enterprise environments, access to port 22 (ssh) may be denied, often by more than one system (e.g.,
zscaler, iptables, gateways, etc.). Some systems block ssh even when found on other ports. In order to
allow students to access EC2 instances on AWS in this type of environment, we need a workaround.

## How does rxm-wetty help?

It is rare that enterprises block access to AWS IPs, because corporate employees still need to browse
commercial and public servers on EC2 using HTTP/S on ports 80/443. rxm-wetty thus allows corporate
students to gain terminal access to RX-M AWS EC2 lab VMs over HTTP/S. Another option is Guacamole, but
this requires a more complex setup.

## How is rxm-wetty run in a lab environment?

Run rxm-wetty on every student system. This involves either executing the installer script in this repo
or building the Rust binary as part of the lab image. This can be done with Ansible, Terraform or an EC2
User Data script. Caveats:

- Unlike normal RX-M lab environments which use ssh keys, a password must be set.
- X11 forwarding does not work over rxm-wetty.
- SFTP does not work over rxm-wetty; it is a browser terminal bridge, not a full SSH client.

## Setup on the AWS Linux box (ubuntu)

The easiest way to complete setup is to:

1. Open the EC2 Console and start the "Launch instance" process.
2. Navigate to the Advanced details section.
3. Paste an install script into the User data field.

The AWS console will then automatically run the script on each system launched.

Script to build and run rxm-wetty on port 80:

```bash
apt-get update -y
apt-get install -y --no-install-recommends ca-certificates curl git openssh-client pkg-config rustc cargo
PASS="${1:-rx-m$(date +%Y%m%d)}"
echo "ubuntu:${PASS}" | chpasswd
sed -i 's/^#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
systemctl restart ssh
git clone https://github.com/rx-m/wetty.git /opt/rxm-wetty
cargo build --manifest-path /opt/rxm-wetty/Cargo.toml --release
install -m 0755 /opt/rxm-wetty/target/release/rxm-wetty /usr/local/bin/rxm-wetty
/usr/local/bin/rxm-wetty -p 80 --host 0.0.0.0 --base /wetty
```

To run with TLS and a self-signed certificate on port 443:

```bash
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/rxm-wetty.key \
  -out /etc/rxm-wetty.cert \
  -subj "/C=US/ST=State/L=City/O=Organization/CN=rx-m.com"
/usr/local/bin/rxm-wetty -p 443 --host 0.0.0.0 --base /wetty \
  --ssl-cert /etc/rxm-wetty.cert --ssl-key /etc/rxm-wetty.key
```

To access the system, browse to: `http(s)://<pub-ip>/wetty`

> The self-signed cert option requires users to accept the browser security warning.

Login with credentials: `ubuntu/rx-myyyymmdd` (password defaults to rx-m and the year, month,
day of system launch). Setting the password to something less predictable in the script is advised.

## Useful commands

After connecting to an AWS instance, bring repos up-to-date:

```bash
sudo apt update
```

Create or change a password for the `ubuntu` user:

```bash
sudo passwd ubuntu
```

Create a new user:

```bash
sudo adduser student
```

Allow password logins in `/etc/ssh/sshd_config`:

```text
PasswordAuthentication yes
```

Remove the Ubuntu cloud image override if present:

```bash
sudo rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
sudo systemctl reload ssh
```

Build rxm-wetty:

```bash
cargo build --release
```

Run rxm-wetty on port 80:

```bash
sudo ./target/release/rxm-wetty -p 80 --host 0.0.0.0 --base /wetty
```

Run rxm-wetty over TLS on port 443:

```bash
sudo ./target/release/rxm-wetty \
  -p 443 \
  --host 0.0.0.0 \
  --base /wetty \
  --ssl-cert ./my-untrusted.cert \
  --ssl-key ./my-untrusted.key
```

## Test

Navigate to the AWS public IP of the EC2 instance: `http(s)://10.20.30.40/wetty`

The public IP can be found from the AWS instance with:

```bash
curl http://checkip.amazonaws.com
```

Supply credentials for the user whose password was just set.

_Copyright (c) 2025-2026 RX-M LLC, Cloud Native Consulting, all rights reserved_
