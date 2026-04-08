//! DAP stdio/TCP server entry point.

use std::net::SocketAddr;

use anyhow::Result;
use tokio::io::{BufReader, BufWriter};
use tracing::info;

use crate::dap_types::DapMessage;
use crate::debugger::RfDebugger;
use crate::protocol::{DapHandler, DapProtocol};

// ── DapServer ─────────────────────────────────────────────────────────────────

/// DAP server that dispatches to an [`RfDebugger`].
pub struct DapServer {
    debugger: RfDebugger,
    /// Outgoing sequence number counter.
    seq: i64,
}

impl DapServer {
    /// Create a new server with a fresh [`RfDebugger`].
    pub fn new() -> Self {
        Self {
            debugger: RfDebugger::new(),
            seq: 1,
        }
    }

    fn next_seq(&mut self) -> i64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    // ── Transport entry points ─────────────────────────────────────────────

    /// Serve a DAP session over stdio.
    ///
    /// Reads from stdin and writes to stdout until the connection closes.
    pub async fn serve_stdio(mut self) -> Result<()> {
        info!("Starting DAP server on stdio");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut writer = BufWriter::new(stdout);

        self.message_loop(&mut reader, &mut writer).await
    }

    /// Serve a single DAP session over TCP on `port`.
    ///
    /// Accepts the first connection, serves it, then returns.
    pub async fn serve_tcp(mut self, port: u16) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!(%addr, "DAP server listening on TCP");

        let (stream, peer) = listener.accept().await?;
        info!(%peer, "Accepted DAP TCP connection");

        let (read_half, write_half) = tokio::io::split(stream);
        let mut reader = BufReader::new(read_half);
        let mut writer = BufWriter::new(write_half);

        self.message_loop(&mut reader, &mut writer).await
    }

    // ── Core message loop ──────────────────────────────────────────────────

    async fn message_loop<R, W>(&mut self, reader: &mut BufReader<R>, writer: &mut W) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        loop {
            let message = match DapProtocol::read_message(reader).await {
                Ok(m) => m,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("Connection closed") || msg.contains("unexpected end") {
                        info!("DAP client disconnected");
                        return Ok(());
                    }
                    return Err(e);
                }
            };

            if let DapMessage::Request(ref req) = message {
                let request_seq = req.seq;
                let command = req.command.clone();
                let is_disconnect = command == "disconnect";

                let response = match self.debugger.handle(&message) {
                    Ok(body) => {
                        DapProtocol::success_response(self.next_seq(), request_seq, &command, body)
                    }
                    Err(e) => DapProtocol::error_response(
                        self.next_seq(),
                        request_seq,
                        &command,
                        &e.to_string(),
                    ),
                };

                DapProtocol::write_message(writer, &response).await?;

                if is_disconnect {
                    return Ok(());
                }
            }
        }
    }
}

impl Default for DapServer {
    fn default() -> Self {
        Self::new()
    }
}
