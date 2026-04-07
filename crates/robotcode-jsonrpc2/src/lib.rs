//! Thin JSON-RPC 2.0 wrapper around `tower-lsp`.
//!
//! Re-exports the essential tower-lsp types so downstream crates only need to
//! depend on this crate and not directly on `tower-lsp`.

// Re-export tower-lsp types so consumers can use them without a direct dependency.
pub use tower_lsp::jsonrpc::{Error, ErrorCode, Result};
pub use tower_lsp::lsp_types;
pub use tower_lsp::{Client, LanguageServer, LspService, Server};

/// Marker trait analogous to Python's `@rpc_method` decorator.
///
/// Implement this on any type that handles a specific JSON-RPC method so that
/// the type system can enforce correct dispatch registration.
pub trait RpcMethod: Send + Sync + 'static {
    /// The JSON-RPC method name (e.g. `"textDocument/didOpen"`).
    fn method_name() -> &'static str
    where
        Self: Sized;
}

/// Transport variant for the language server.
///
/// Mirrors the `--stdio` / `--tcp PORT` CLI flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// Read/write on stdin/stdout — the default LSP transport.
    Stdio,
    /// Listen on the given TCP port.
    Tcp(u16),
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transport::Stdio => write!(f, "stdio"),
            Transport::Tcp(port) => write!(f, "tcp:{port}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_display_stdio() {
        assert_eq!(Transport::Stdio.to_string(), "stdio");
    }

    #[test]
    fn transport_display_tcp() {
        assert_eq!(Transport::Tcp(6006).to_string(), "tcp:6006");
    }

    #[test]
    fn transport_equality() {
        assert_eq!(Transport::Tcp(1234), Transport::Tcp(1234));
        assert_ne!(Transport::Tcp(1234), Transport::Tcp(5678));
        assert_ne!(Transport::Stdio, Transport::Tcp(1234));
    }

    struct PingHandler;

    impl RpcMethod for PingHandler {
        fn method_name() -> &'static str {
            "$/ping"
        }
    }

    #[test]
    fn rpc_method_name() {
        assert_eq!(PingHandler::method_name(), "$/ping");
    }
}
