//! DAP message framing and dispatching.
//!
//! The Debug Adapter Protocol uses HTTP-like `Content-Length` framing over a
//! byte stream (stdio or TCP).  This module handles reading/writing those
//! frames and routing requests to a [`DapHandler`] implementation.

use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, warn};

use crate::dap_types::{DapMessage, ProtocolResponse};

// ── DapHandler trait ──────────────────────────────────────────────────────────

/// Trait implemented by the core debugger to handle incoming DAP requests.
pub trait DapHandler: Send + Sync {
    /// Dispatch an incoming [`DapMessage`] and produce an optional response body.
    ///
    /// Returning `Ok(None)` means the handler will send an empty-body success
    /// response.  Returning `Err(e)` sends an error response.
    fn handle(&mut self, message: &DapMessage) -> Result<Option<serde_json::Value>>;
}

// ── DapProtocol ───────────────────────────────────────────────────────────────

/// Content-Length–framed DAP message parser/serialiser.
pub struct DapProtocol;

impl DapProtocol {
    /// Read a single DAP message from `reader`.
    ///
    /// Parses `Content-Length: N\r\n\r\n` headers then reads exactly `N` bytes
    /// of UTF-8 JSON body.
    pub async fn read_message<R>(reader: &mut BufReader<R>) -> Result<DapMessage>
    where
        R: AsyncReadExt + Unpin,
    {
        let content_length = Self::read_headers(reader).await?;

        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).await?;

        debug!(bytes = content_length, "Received DAP message");

        let message: DapMessage = serde_json::from_slice(&body)
            .map_err(|e| anyhow!("Failed to parse DAP message: {e}"))?;

        Ok(message)
    }

    /// Serialize and write a [`DapMessage`] with the correct Content-Length header.
    pub async fn write_message<W>(writer: &mut W, message: &DapMessage) -> Result<()>
    where
        W: AsyncWriteExt + Unpin,
    {
        let body = serde_json::to_vec(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        writer.write_all(header.as_bytes()).await?;
        writer.write_all(&body).await?;
        writer.flush().await?;
        debug!(bytes = body.len(), "Sent DAP message");
        Ok(())
    }

    /// Build an error [`ProtocolResponse`] for a failed request.
    pub fn error_response(seq: i64, request_seq: i64, command: &str, message: &str) -> DapMessage {
        DapMessage::Response(ProtocolResponse {
            seq,
            request_seq,
            success: false,
            command: command.to_owned(),
            message: Some(message.to_owned()),
            body: None,
        })
    }

    /// Build a success [`ProtocolResponse`] for a request.
    pub fn success_response(
        seq: i64,
        request_seq: i64,
        command: &str,
        body: Option<serde_json::Value>,
    ) -> DapMessage {
        DapMessage::Response(ProtocolResponse {
            seq,
            request_seq,
            success: true,
            command: command.to_owned(),
            message: None,
            body,
        })
    }

    // ── Internal ──────────────────────────────────────────────────────────

    /// Read HTTP-like headers and return the `Content-Length` value.
    async fn read_headers<R>(reader: &mut BufReader<R>) -> Result<usize>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut content_length: Option<usize> = None;

        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(anyhow!("Connection closed while reading DAP headers"));
            }

            let line = line.trim_end_matches(['\r', '\n']);

            if line.is_empty() {
                // Blank line signals end of headers.
                break;
            }

            if let Some(rest) = line.strip_prefix("Content-Length: ") {
                content_length = Some(
                    rest.trim()
                        .parse::<usize>()
                        .map_err(|e| anyhow!("Invalid Content-Length value '{rest}': {e}"))?,
                );
            } else {
                warn!(header = %line, "Ignoring unknown DAP header");
            }
        }

        content_length.ok_or_else(|| anyhow!("DAP message missing Content-Length header"))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a Content-Length–framed message byte sequence.
    fn frame(json: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
    }

    #[tokio::test]
    async fn round_trip_event() {
        use crate::dap_types::ProtocolEvent;

        let msg = DapMessage::Event(ProtocolEvent {
            seq: 1,
            event: "initialized".to_owned(),
            body: None,
        });

        let mut buf: Vec<u8> = Vec::new();
        DapProtocol::write_message(&mut buf, &msg).await.unwrap();

        let cursor = std::io::Cursor::new(buf);
        let mut reader = BufReader::new(cursor);
        let parsed = DapProtocol::read_message(&mut reader).await.unwrap();

        let DapMessage::Event(ev) = parsed else {
            panic!("Expected Event");
        };
        assert_eq!(ev.event, "initialized");
        assert_eq!(ev.seq, 1);
    }

    #[tokio::test]
    async fn read_message_parses_request() {
        let json = r#"{"type":"request","seq":1,"command":"initialize"}"#;
        let raw = frame(json);
        let cursor = std::io::Cursor::new(raw);
        let mut reader = BufReader::new(cursor);
        let msg = DapProtocol::read_message(&mut reader).await.unwrap();
        let DapMessage::Request(req) = msg else {
            panic!("Expected Request");
        };
        assert_eq!(req.command, "initialize");
    }

    #[tokio::test]
    async fn write_message_produces_correct_header() {
        use crate::dap_types::ProtocolEvent;

        let msg = DapMessage::Event(ProtocolEvent {
            seq: 42,
            event: "stopped".to_owned(),
            body: None,
        });

        let mut buf: Vec<u8> = Vec::new();
        DapProtocol::write_message(&mut buf, &msg).await.unwrap();

        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));
    }
}
