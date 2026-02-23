//! Library facade for integration tests.
//!
//! The binary crate (`main.rs`) owns the canonical module tree.
//! This crate re-exports only the subset needed by `tests/` integration tests.

pub mod error;
pub mod models;

/// Re-export only the IPC client for integration tests.
/// The server module has heavy dependencies (bot, telegram, config)
/// that live only in the binary crate.
pub mod ipc {
    pub mod client;
}
