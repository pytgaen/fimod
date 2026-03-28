pub mod convert;
pub mod dotpath;
pub mod engine;
pub mod env_helpers;
pub mod exit_control;
pub mod format;
pub mod format_control;
pub mod gatekeeper;
pub mod hash;
pub mod http;
pub mod iter_helpers;
pub mod mold;
pub mod msg;
pub mod regex;
pub mod registry;
pub mod test_runner;
pub mod pipeline;

/// Monty engine version — keep in sync with the `tag` in Cargo.toml when upgrading monty.
pub const MONTY_VERSION: &str = "0.0.8";
