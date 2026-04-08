//! `robotcode-analyze` — Batch static analysis for Robot Framework files.
//!
//! Provides [`analyze::analyze`] for running diagnostics on `.robot`/`.resource`
//! files from the CLI, with JSON or human-readable text output.

pub mod analyze;
pub mod cache;
