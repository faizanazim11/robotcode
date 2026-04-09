//! `robotcode-repl` — Interactive REPL server for Robot Framework keyword evaluation.
//!
//! Provides a JSON-RPC 2.0 server (stdio or TCP) that accepts `evaluate`,
//! `complete`, and `history` requests, forwarding `evaluate` calls to the
//! Python bridge.
//!
//! # Architecture
//!
//! ```text
//! VS Code / client
//!     │  JSON-RPC 2.0 (stdio or TCP)
//!     ▼
//! ReplServer  ─── evaluate ──►  Python bridge (python helper.py)
//!             ─── history  ──►  History store
//!             ─── complete ──►  Completion engine
//! ```

pub mod eval;
pub mod history;
pub mod server;
