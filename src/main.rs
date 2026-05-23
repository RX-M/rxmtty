use std::{
    io::{Read, Write},
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{
        State,
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
struct Config {
    #[arg(short = 'p', long = "port", default_value_t = 80)]
    port: u16,

    #[arg(long = "host", default_value = "0.0.0.0")]
    host: String,

    #[arg(long = "base", default_value = "/tty")]
    base: String,

    #[arg(long = "ssh-host", default_value = "127.0.0.1")]
    ssh_host: String,

    #[arg(long = "ssh-user", default_value = "ubuntu")]
    ssh_user: String,

    #[arg(long = "ssh-port", default_value_t = 22)]
    ssh_port: u16,

    #[arg(long = "command")]
    command: Option<String>,

    #[arg(long = "ssl-cert")]
    ssl_cert: Option<String>,

    #[arg(long = "ssl-key")]
    ssl_key: Option<String>,
}

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut config = Config::parse();
    config.base = normalize_base(&config.base);
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", config.host, config.port))?;

    let state = AppState {
        config: Arc::new(config.clone()),
    };
    let app = Router::new()
        .route("/", get(redirect_to_base))
        .route(&config.base, get(index))
        .route(&format!("{}/", config.base), get(index))
        .route(&format!("{}/ws", config.base), get(ws_handler))
        .with_state(state);

    display_startup_settings(&config);
    info!("listening on {}{}", addr, config.base);
    if let (Some(cert), Some(key)) = (&config.ssl_cert, &config.ssl_key) {
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key)
            .await
            .context("failed to load TLS certificate or key")?;
        axum_server::bind_rustls(addr, tls)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .context("server failed")?;
    } else {
        axum_server::bind(addr)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .context("server failed")?;
    }

    Ok(())
}

async fn redirect_to_base(State(state): State<AppState>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::LOCATION,
        state
            .config
            .base
            .parse()
            .unwrap_or_else(|_| "/tty".parse().unwrap()),
    );
    (StatusCode::TEMPORARY_REDIRECT, headers)
}

async fn index(State(state): State<AppState>) -> Html<String> {
    Html(render_index(&state.config.base))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!("inbound connection from {}", remote_addr.ip());
    ws.on_upgrade(move |socket| async move {
        if let Err(err) = terminal_session(socket, state.config).await {
            error!("{err:#}");
        }
    })
}

async fn terminal_session(socket: WebSocket, config: Arc<Config>) -> Result<()> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open PTY")?;

    let mut command = if let Some(raw) = &config.command {
        let mut cmd = CommandBuilder::new("sh");
        cmd.arg("-lc");
        cmd.arg(raw);
        cmd
    } else {
        let mut cmd = CommandBuilder::new("ssh");
        cmd.arg("-o");
        cmd.arg("PreferredAuthentications=password,keyboard-interactive");
        cmd.arg("-o");
        cmd.arg("PubkeyAuthentication=no");
        cmd.arg("-o");
        cmd.arg("StrictHostKeyChecking=accept-new");
        cmd.arg("-p");
        cmd.arg(config.ssh_port.to_string());
        cmd.arg("-tt");
        cmd.arg(format!("{}@{}", config.ssh_user, config.ssh_host));
        cmd
    };
    command.env("TERM", "xterm-256color");

    let mut child = pair
        .slave
        .spawn_command(command)
        .context("failed to spawn terminal command")?;
    drop(pair.slave);

    let writer = Arc::new(Mutex::new(
        pair.master
            .take_writer()
            .context("failed to create PTY writer")?,
    ));
    let mut reader = pair
        .master
        .try_clone_reader()
        .context("failed to create PTY reader")?;
    let master = Arc::new(Mutex::new(pair.master));

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (pty_tx, mut pty_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if pty_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let outbound = tokio::spawn(async move {
        while let Some(bytes) = pty_rx.recv().await {
            if ws_tx.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(message) = ws_rx.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if let Ok(resize) = serde_json::from_str::<ResizeMessage>(&text) {
                    if resize.kind == "resize" {
                        let _ = master.lock().map(|m| {
                            let _ = m.resize(PtySize {
                                rows: resize.rows.max(1),
                                cols: resize.cols.max(1),
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        });
                        continue;
                    }
                }

                let mut guard = writer.lock().expect("PTY writer mutex poisoned");
                guard.write_all(text.as_bytes())?;
                guard.flush()?;
            }
            Ok(Message::Binary(bytes)) => {
                let mut guard = writer.lock().expect("PTY writer mutex poisoned");
                guard.write_all(&bytes)?;
                guard.flush()?;
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    outbound.abort();
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[derive(Deserialize)]
struct ResizeMessage {
    #[serde(rename = "type")]
    kind: String,
    cols: u16,
    rows: u16,
}

fn normalize_base(base: &str) -> String {
    let trimmed = base.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/tty".to_string();
    }

    let with_slash = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    with_slash.trim_end_matches('/').to_string()
}

fn display_startup_settings(config: &Config) {
    println!("rxmtty settings:");
    println!("  port: {}", config.port);
    println!("  host: {}", config.host);
    println!("  base: {}", config.base);
    println!("  ssh_host: {}", config.ssh_host);
    println!("  ssh_user: {}", config.ssh_user);
    println!("  ssh_port: {}", config.ssh_port);
    println!(
        "  command: {}",
        config.command.as_deref().unwrap_or("<none>")
    );
    println!(
        "  ssl_cert: {}",
        config.ssl_cert.as_deref().unwrap_or("<none>")
    );
    println!(
        "  ssl_key: {}",
        config.ssl_key.as_deref().unwrap_or("<none>")
    );
}

fn render_index(base: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>RX-M tty</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/css/xterm.min.css">
  <style>
    html, body, #terminal {{
      width: 100%;
      height: 100%;
      margin: 0;
      background: #0b0f14;
    }}
    .xterm {{
      height: 100%;
      padding: 10px;
      box-sizing: border-box;
    }}
  </style>
</head>
<body>
  <div id="terminal"></div>
  <script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/lib/xterm.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/addon-fit.min.js"></script>
  <script>
    const term = new Terminal({{
      cursorBlink: true,
      convertEol: true,
      fontFamily: 'Consolas, "Liberation Mono", Menlo, monospace',
      fontSize: 15,
      theme: {{
        background: '#0b0f14',
        foreground: '#d7dde8',
        cursor: '#f2cc60',
        selectionBackground: '#315a78'
      }}
    }});
    const fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(document.getElementById('terminal'));
    fitAddon.fit();

    const scheme = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${{scheme}}://${{window.location.host}}{base}/ws`);
    ws.binaryType = 'arraybuffer';

    function resize() {{
      fitAddon.fit();
      if (ws.readyState === WebSocket.OPEN) {{
        ws.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
      }}
    }}

    ws.addEventListener('open', resize);
    ws.addEventListener('message', (event) => {{
      if (event.data instanceof ArrayBuffer) {{
        term.write(new Uint8Array(event.data));
      }} else {{
        term.write(event.data);
      }}
    }});
    ws.addEventListener('close', () => term.writeln('\r\n[disconnected]'));
    term.onData((data) => ws.readyState === WebSocket.OPEN && ws.send(data));
    window.addEventListener('resize', resize);
  </script>
</body>
</html>"#
    )
}
