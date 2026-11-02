use anyhow::Context as _;
use clap::{Parser, Subcommand};
use rand::RngCore as _;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "luban", version, about = "Luban CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Luban server and open the browser UI.
    Ui {
        /// Bind address for the local server (defaults to `127.0.0.1:0`).
        #[arg(long)]
        addr: Option<SocketAddr>,

        /// Print the URL but do not open a browser.
        #[arg(long, default_value_t = false)]
        no_open: bool,
    },
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let mut out = String::with_capacity(bytes * 2);
    for b in buf {
        out.push(hex_char(b >> 4));
        out.push(hex_char(b & 0x0f));
    }
    out
}

fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => '0',
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = luban_server::shell_env::apply_runtime_shell_env_defaults() {
        eprintln!("warning: failed to apply shell environment defaults: {err:#}");
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Command::Ui { addr, no_open } => ui(addr, no_open).await,
    }
}

async fn ui(addr: Option<SocketAddr>, no_open: bool) -> anyhow::Result<()> {
    let addr = addr.unwrap_or_else(|| {
        std::env::var("LUBAN_SERVER_ADDR")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or_else(|| "127.0.0.1:0".parse().expect("valid socket addr"))
    });

    let token = random_hex(32);

    let server = luban_server::start_server_with_config(
        addr,
        luban_server::ServerConfig {
            auth: luban_server::AuthConfig {
                mode: luban_server::AuthMode::SingleUser,
                bootstrap_token: Some(token.clone()),
            },
        },
    )
    .await?;
    let url = format!("http://{}/auth?token={}", server.addr, token);

    println!("{url}");
    if !no_open && let Err(err) = open::that(&url) {
        tracing::warn!(error = %err, "failed to open browser");
    }

    tracing::info!(addr = %server.addr, "luban ui started (press Ctrl+C to stop)");
    tokio::signal::ctrl_c()
        .await
        .context("failed to install Ctrl+C handler")?;
    Ok(())
}
