//! `robotcode-debugger` — Debug Adapter Protocol (DAP 1.51) server.
//!
//! Implements the [Debug Adapter Protocol](https://microsoft.github.io/debug-adapter-protocol/)
//! for debugging Robot Framework tests through any DAP-capable client (VS Code, IntelliJ, etc.).

pub mod dap_types;
pub mod debugger;
pub mod launcher;
pub mod protocol;
pub mod server;
