![RX-M LLC](https://rx-m.com/rxm-cnc.svg)


# rxmtty

`rxmtty` is a Rust based web to ssh terminal service. Users browse to `http://somehost/tty` and `rxmtty` provides an
in-browser terminal interface bridged to a local ssh client.

```
User <-> Browser <--websocket--> rxmtty <-> ssh <--ssh-protocol--> sshd <-> Shell on Remote Host
```

`rxmtty` technology stack:

- Rust - fast, safe, minimal server with TLS support and low resource usage (https://www.rust-lang.org/)
- Xterm.js - fast native JavaScript browser-based terminal interface (https://github.com/xtermjs/xterm.js/)
- WebSocket - fast bidirectional browser to server communication (https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API)
- ssh - secure remote shell protocol, handles authentication and session management (https://www.openssh.com/)

`rxmtty` is designed to allow users who only have web browsing capabilities (say for security purposes) to terminal into
a remote host, such as a cloud instance, using a web browser on port 80/443 (or any port you choose).

The `rxmtty` server typically runs on the same system that runs the `sshd` service to be accessed but can also be
configured to forward traffic to a remote `sshd` daemon.

By default, `rxmtty` serves the browser terminal at `/tty` on all local interfaces using port 80. Upon connection,
`rxmtty` upgrades the browser session to WebSocket. The browser session is then bridged to a local PTY running `ssh
ubuntu@127.0.0.1` by default. The browser based user will be prompted for a password and then dropped into a terminal
session on the remote host. Traffic is proxied back and forth over the websocket connection.


## Build

To build the `rxmtty` binary, clone the repository and run:

```bash
cargo build --release
```

The binary is created at:

```bash
target/release/rxm-wetty
```


## Run

To run `rxmtty`, execute the binary with appropriate permissions (e.g. `sudo` if using port 80) and options:

```bash
sudo ./target/release/rxmtty -p 80 --host 0.0.0.0 --base /tty
```

## Options

```text
$ ./rxmtty -h

RX-M browser terminal bridge implemented in Rust

Usage: rxmtty [OPTIONS]

Options:
  -p, --port <PORT>          [default: 80]
      --host <HOST>          [default: 0.0.0.0]
      --base <BASE>          [default: /tty]
      --ssh-host <SSH_HOST>  [default: 127.0.0.1]
      --ssh-user <SSH_USER>  [default: ubuntu]
      --ssh-port <SSH_PORT>  [default: 22]
      --command <COMMAND>    Arbitrary command to run instead of ssh
      --ssl-cert <SSL_CERT>  
      --ssl-key <SSL_KEY>    
  -h, --help                 Print help
  -V, --version              Print version

$ 
```


## Examples

Run rxmtty on port 8080:

```bash
sudo ./target/release/rxmtty -p 8080 --host 0.0.0.0 --base /tty
```

Run rxmtty with TLS on port 443 with a self signed cert:

```bash
$ openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -sha256 -days 365 -nodes -subj "/C=US/ST=State/L=City/O=Organization/CN=rx-m.com"

$ sudo ./target/release/rxmtty \
  -p 443 \
  --host 0.0.0.0 \
  --base /tty \
  --ssl-cert ./cert.pem \
  --ssl-key ./key.pem
```

Find the public IP address of a cloud instance:

```bash
ubuntu@ip-172-31-88-233:~$ curl icanhazip.com

18.212.169.66

ubuntu@ip-172-31-88-233:~$ 
```

Setting up users and passwords:

```bash
sudo adduser student
sudo passwd student
```


## Caveats

1. By default, `rxmtty` runs on port 80. You can add TLS support using the --ssl-cert/--ssl-key options, keep in mind
   that self-signed certs will require users to click through browser security warnings to access the terminal.
2. SFTP will not work, the browser communicates with `rxmtty` over WebSocket, then proxies traffic to the SSH backend.
   As a work around, file uploads can be made by copying files from the browser machine to a cloud location (e.g.
   github, S3, etc.) and then pulling the file down on the terminal host with wget or curl.
3. You can not forward GUI windows over the `rxmtty` connection with X11. Any GUIs used on the remote system will have
   to be web servers accessed with new browser tabs remotely over a separate connection.


## EC2 User Data

The following script can be used when standing up EC2 instances with `rxmtty` preinstalled. Add the below script in the
`Advanced details` -> `User data` field and the instance will build `rxmtty` and run it on boot.

```bash
#!/bin/bash

until curl -s --head http://169.254.169.254/latest/meta-data/ >/dev/null; do
  echo "Waiting for metadata service..."
  sleep 2
done

apt-get update -y
sudo apt install build-essential -y
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
git clone https://github.com/RX-M/rxmtty /opt/rxmtty
cargo build --manifest-path /opt/rxmtty/Cargo.toml --release
install -m 0755 /opt/rxmtty/target/release/rxmtty /usr/local/bin/rxmtty
PASS="${1:-rx-m$(date +%Y%m%d)}"
echo "ubuntu:${PASS}" | chpasswd
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
systemctl restart ssh
/usr/local/bin/rxmtty -p 80 --host 0.0.0.0 --base /tty
```

After launching the instance you should be able to browse to `http://<pub-ip>/tty` and log in with the credentials
`ubuntu/rx-myyyymmdd` (password defaults to rx-m and the year, month, day of system launch). Setting the password to
something less predictable in the script is advised.
