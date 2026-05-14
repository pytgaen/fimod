//! CLI subcommand handlers — one module per top-level subcommand.
//!
//! Each module owns the dispatch + business logic for its subcommand,
//! keeping `src/main.rs` slim (just `main()` + top-level dispatch).

pub mod completions;
pub mod monty;
pub mod shape;
