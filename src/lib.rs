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
pub(crate) mod paths;
pub mod pipeline;
pub mod regex;
pub mod registry;
pub mod sandbox;
pub mod serde_compat;
pub mod setup;
pub mod template;
pub mod test_runner;

/// Monty engine version — extracted from the `tag` in Cargo.toml by build.rs.
pub const MONTY_VERSION: &str = env!("MONTY_VERSION");
