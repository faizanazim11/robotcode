//! RobotCode CLI entry point.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use robotcode_jsonrpc2::Transport;
use robotcode_language_server::RobotCodeServer;
use tower_lsp::{LspService, Server};
use tracing::info;

/// RobotCode — Robot Framework IDE toolkit.
#[derive(Debug, Parser)]
#[command(name = "robotcode", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the Language Server Protocol server.
    LanguageServer(LanguageServerArgs),
}

/// Arguments for the `language-server` subcommand.
#[derive(Debug, Parser)]
struct LanguageServerArgs {
    /// Use stdio transport (default).
    #[arg(long, conflicts_with = "tcp")]
    stdio: bool,

    /// Use TCP transport on PORT.
    #[arg(long, value_name = "PORT", conflicts_with = "stdio")]
    tcp: Option<u16>,

    /// Path to the Python interpreter used for the Python bridge.
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,
}

impl LanguageServerArgs {
    fn transport(&self) -> Transport {
        if let Some(port) = self.tcp {
            Transport::Tcp(port)
        } else {
            Transport::Stdio
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::LanguageServer(args) => {
            let transport = args.transport();
            if let Some(ref python) = args.python {
                info!(python = %python.display(), "Python bridge path configured");
            }
            run_language_server(transport).await?;
        }
    }

    Ok(())
}

async fn run_language_server(transport: Transport) -> Result<()> {
    info!(%transport, "Starting RobotCode language server");

    let (service, socket) = LspService::new(RobotCodeServer::new);

    match transport {
        Transport::Stdio => {
            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        Transport::Tcp(port) => {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            info!(%addr, "Listening on TCP");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let (stream, peer) = listener.accept().await?;
            info!(%peer, "Accepted TCP connection");
            let (read, write) = tokio::io::split(stream);
            Server::new(read, write, socket).serve(service).await;
        }
    }

    Ok(())
}
