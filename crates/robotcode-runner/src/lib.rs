//! `robotcode-runner` — CLI runner tools for Robot Framework.
//!
//! Provides wrappers around `python -m robot`, `python -m robot.rebot`,
//! `python -m robot.libdoc`, `python -m robot.testdoc`, and a native
//! test-discovery engine using the Rust RF parser.

pub mod discover;
pub mod libdoc;
pub mod rebot;
pub mod run;
pub mod testdoc;
