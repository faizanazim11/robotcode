//! `robotcode-robot` — Robot Framework analysis engine.
//!
//! This crate provides the high-level analysis layer that sits between the
//! raw RF parser ([`robotcode_rf_parser`]) and the language server
//! ([`robotcode_language_server`]).
//!
//! The primary entry point for phase 4 is [`diagnostics::library_doc`], which
//! fetches and caches [`LibraryDoc`] structs via the Python bridge.

pub mod diagnostics;
