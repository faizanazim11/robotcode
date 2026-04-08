//! RobotCode CLI entry point.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use robotcode_analyze::analyze::{AnalyzeArgs, OutputFormat};
use robotcode_debugger::server::DapServer;
use robotcode_jsonrpc2::{LspService, Server, Transport};
use robotcode_language_server::RobotCodeServer;
use robotcode_runner::{
    discover::DiscoverArgs, libdoc::LibdocArgs, rebot::RebotArgs, run::RunArgs,
    testdoc::TestdocArgs,
};
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
    /// Start the Debug Adapter Protocol server.
    Debug(DebugArgs),
    /// Run Robot Framework tests (`python -m robot`).
    Run(RunCliArgs),
    /// Post-process RF output files with Rebot (`python -m robot.rebot`).
    Rebot(RebotCliArgs),
    /// Generate library documentation (`python -m robot.libdoc`).
    Libdoc(LibdocCliArgs),
    /// Generate test documentation (`python -m robot.testdoc`).
    Testdoc(TestdocCliArgs),
    /// Discover Robot Framework tests (Rust-native, no Python required).
    Discover(DiscoverCliArgs),
    /// Run batch static analysis on Robot Framework files.
    Analyze(AnalyzeCliArgs),
}

// ── language-server ───────────────────────────────────────────────────────────

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

// ── debug ─────────────────────────────────────────────────────────────────────

/// Arguments for the `debug` (DAP server) subcommand.
#[derive(Debug, Parser)]
struct DebugArgs {
    /// Use stdio transport (default).
    #[arg(long, conflicts_with = "tcp")]
    stdio: bool,

    /// Listen for a DAP client on TCP PORT.
    #[arg(long, value_name = "PORT", conflicts_with = "stdio")]
    tcp: Option<u16>,
}

// ── run ───────────────────────────────────────────────────────────────────────

/// Arguments for the `run` subcommand.
#[derive(Debug, Parser)]
struct RunCliArgs {
    /// Path to the Python interpreter.
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,

    /// Arguments forwarded to `python -m robot`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// ── rebot ─────────────────────────────────────────────────────────────────────

/// Arguments for the `rebot` subcommand.
#[derive(Debug, Parser)]
struct RebotCliArgs {
    /// Path to the Python interpreter.
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,

    /// Arguments forwarded to `python -m robot.rebot`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// ── libdoc ────────────────────────────────────────────────────────────────────

/// Arguments for the `libdoc` subcommand.
#[derive(Debug, Parser)]
struct LibdocCliArgs {
    /// Path to the Python interpreter.
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,

    /// Arguments forwarded to `python -m robot.libdoc`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// ── testdoc ───────────────────────────────────────────────────────────────────

/// Arguments for the `testdoc` subcommand.
#[derive(Debug, Parser)]
struct TestdocCliArgs {
    /// Path to the Python interpreter.
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,

    /// Arguments forwarded to `python -m robot.testdoc`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// ── discover ──────────────────────────────────────────────────────────────────

/// Arguments for the `discover` subcommand.
#[derive(Debug, Parser)]
struct DiscoverCliArgs {
    /// Output raw JSON instead of human-readable text.
    #[arg(long)]
    json: bool,

    /// Recurse into subdirectories.
    #[arg(long, short = 'r')]
    recursive: bool,

    /// Paths to discover (files or directories).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
}

// ── analyze ───────────────────────────────────────────────────────────────────

/// Arguments for the `analyze` subcommand.
#[derive(Debug, Parser)]
struct AnalyzeCliArgs {
    /// Output JSON instead of human-readable text.
    #[arg(long)]
    json: bool,

    /// Exit with code 1 on errors.
    #[arg(long)]
    fail_on_error: bool,

    /// Exit with code 2 on warnings (when no errors).
    #[arg(long)]
    fail_on_warning: bool,

    /// Path to the Python interpreter (reserved for future use).
    #[arg(long, value_name = "PATH")]
    python: Option<PathBuf>,

    /// Paths to analyze (files or directories).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
}

// ── main ──────────────────────────────────────────────────────────────────────

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
            run_language_server(transport, args.python).await?;
        }

        Commands::Debug(args) => {
            let server = DapServer::new();
            if let Some(port) = args.tcp {
                server.serve_tcp(port).await?;
            } else {
                server.serve_stdio().await?;
            }
        }

        Commands::Run(args) => {
            let exit_code = robotcode_runner::run::run(RunArgs {
                python: args.python,
                args: args.args,
            })
            .await?;
            std::process::exit(exit_code);
        }

        Commands::Rebot(args) => {
            let exit_code = robotcode_runner::rebot::rebot(RebotArgs {
                python: args.python,
                args: args.args,
            })
            .await?;
            std::process::exit(exit_code);
        }

        Commands::Libdoc(args) => {
            let exit_code = robotcode_runner::libdoc::libdoc(LibdocArgs {
                python: args.python,
                args: args.args,
            })
            .await?;
            std::process::exit(exit_code);
        }

        Commands::Testdoc(args) => {
            let exit_code = robotcode_runner::testdoc::testdoc(TestdocArgs {
                python: args.python,
                args: args.args,
            })
            .await?;
            std::process::exit(exit_code);
        }

        Commands::Discover(args) => {
            let report = robotcode_runner::discover::discover(DiscoverArgs {
                paths: args.paths,
                recursive: args.recursive,
            })
            .await?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for suite in &report.suites {
                    println!("Suite: {} ({})", suite.name, suite.source.display());
                    for test in &suite.tests {
                        println!("  Test: {} (line {})", test.name, test.line);
                    }
                    for kw in &suite.keywords {
                        println!("  Keyword: {} (line {})", kw.name, kw.line);
                    }
                }
            }
        }

        Commands::Analyze(args) => {
            let fmt = if args.json {
                OutputFormat::Json
            } else {
                OutputFormat::Text
            };
            let report = robotcode_analyze::analyze::analyze(AnalyzeArgs {
                paths: args.paths,
                python: args.python,
                output_format: fmt,
                fail_on_error: args.fail_on_error,
                fail_on_warning: args.fail_on_warning,
            })
            .await?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Files analyzed: {}, Errors: {}, Warnings: {}",
                    report.files_analyzed, report.errors, report.warnings
                );
                for fd in &report.diagnostics {
                    for diag in &fd.diagnostics {
                        println!(
                            "  {}:{}: {}",
                            fd.path.display(),
                            diag.range.start.line + 1,
                            diag.message
                        );
                    }
                }
            }

            let exit_code = report.exit_code;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
    }

    Ok(())
}

// ── Language server helper ────────────────────────────────────────────────────

async fn run_language_server(transport: Transport, python: Option<PathBuf>) -> Result<()> {
    info!(%transport, "Starting RobotCode language server");

    let (service, socket) =
        LspService::new(move |client| RobotCodeServer::with_python(client, python.clone()));

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
