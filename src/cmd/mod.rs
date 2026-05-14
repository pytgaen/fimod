//! CLI subcommand handlers — one module per top-level subcommand.
//!
//! Each module owns the dispatch + business logic for its subcommand,
//! keeping `src/main.rs` slim (just `main()` + top-level dispatch).

pub mod completions;
pub mod mold;
pub mod monty;
pub mod registry;
pub mod setup;
pub mod shape;
