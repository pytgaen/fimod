# TODO 0.7.2

Transient backlog for the 0.7.2 cycle. Items move to user docs /
changelog once shipped.

The 0.7.1 cycle closed the e2e test coverage gaps and the watch mode
hardening. What remains are two structural refactors that were
originally planned for 0.8.0 but make sense to land before any further
feature work.

## Split `src/registry.rs`

2180 lines, 69 top-level symbols. Already hard to navigate; the
planned work (Bitbucket backend per `notes/ARCHITECTURE.md:163`, FS
mounts, extra resolution paths) will push it past 3000 and into "code
nobody wants to touch" territory. Architectural debt in large files
compounds non-linearly — split now, before the next features land on
top.

Proposed layout (`src/registry/` module, `mod.rs` re-exports the
current public API so external callers stay unchanged):

| File         | Lines (current)             | Responsibility                                      |
| ------------ | --------------------------- | --------------------------------------------------- |
| `config.rs`  | 14–435                      | `Source`, `SourcesConfig`, `sources.toml` CRUD,     |
|              |                             | `add` / `remove` / `list` / `show` / `set_priority` |
| `resolve.rs` | 440–800                     | `@name` / `@src/name` resolution, env registries,   |
|              |                             | `resolve_local`, `resolve_remote`, auth headers     |
| `catalog.rs` | 800–1046, 1936–2008,        | `Catalog`, `fetch_catalog`, `build_catalog`,        |
|              | 2011–2180, + cache 952–1145 | `scan_local_molds`, `github_to_raw`,                |
|              |                             | `compute_mold_hash`, cache helpers                  |
| `molds.rs`   | 1147–1794                   | `list_molds`, `show_mold`, `MoldMatch`, completions |

The `setup` wizard (lines 1803–1927) leaves the registry module
entirely and merges into the existing `src/setup.rs` (currently a
86-line CLI wrapper that calls `registry::setup`). Result:
`src/setup.rs` becomes autonomous (~210 lines), the trampoline
indirection `registry::setup() ← setup::registry_defaults()`
disappears.

Order of operations:

1. Add `pub mod registry;` with the new submodules, keeping all
   `pub fn` signatures intact. No call-site changes outside
   `registry/`.
2. Move blocks one at a time, running `rtk task lint && rtk task test`
   between each move. Pure-move commits — no rename, no signature
   change, no logic edit.
3. After all moves are green, do a separate cleanup pass: visibility
   tightening (`pub` → `pub(crate)` where possible), helper
   consolidation.

Rule: do this **before** the Bitbucket / FS-mount work, so the new
backends land in a clean `resolve.rs` instead of compounding the mess.

## Split `src/main.rs`

1261 lines, 24 top-level symbols. `run_shape` (L714–919) +
`run_shape_pipeline` (L921–1261) account for ~547 lines of Shape
dispatch logic glued to the `main` entry point. Adding it to
`pipeline.rs` was a bad idea — that file is already 1136 lines and
its responsibility is data-transformation orchestration, not
CLI-args-to-config translation.

Plan: introduce `src/cli.rs` for clap definitions and `src/cmd/` for
subcommand handlers. `main.rs` shrinks to ~80 lines (entry point +
top-level dispatch).

| File                  | Lines moved from main.rs | Responsibility                                          |
| --------------------- | ------------------------ | ------------------------------------------------------- |
| `cli.rs`              | 33–391, 439–483          | `Cli`, `ShapeArgs`, `Commands`, all subcommand enums,   |
|                       |                          | completion candidate helpers (`format_candidates`,      |
|                       |                          | `complete_molds`, `complete_sources`), `MsgLevel`       |
| `cmd/shape.rs`        | 394–437, 714–1261        | `run_shape`, `run_shape_pipeline`, `build_script_refs`  |
| `cmd/registry.rs`     | (façade, new)            | thin dispatcher for `RegistryAction` variants → calls   |
|                       |                          | into `registry::*` (post-split per previous section)    |
| `cmd/mold.rs`         | (façade, new)            | thin dispatcher for `MoldAction` → calls `mold::*`      |
| `cmd/setup.rs`        | (move existing)          | absorbs current `src/setup.rs` (which itself absorbs    |
|                       |                          | `registry::setup` per the registry-split plan)          |
| `cmd/monty.rs`        | 629–712                  | `run_monty_repl`, `repl_feed`                           |
| `cmd/completions.rs`  | 485–523                  | `print_completion_script`, `detect_shell`               |
| `main.rs`             | (residual)               | `main`, `dispatch`, `dispatch_other` (slimmed)          |

Order of operations:

1. Create `src/cli.rs` first, move clap definitions over. No behavior
   change; main.rs imports them. Lint + test.
2. Create `src/cmd/mod.rs` with empty submodules. Move handlers one
   subcommand at a time, lint + test between each move.
3. Slim `dispatch` / `dispatch_other` last, once every handler has
   moved out.

Cross-reference: this split combines with the registry split — the
final layout has `src/cmd/setup.rs` owning the wizard (originally in
`registry.rs::setup`, then `src/setup.rs`, then `src/cmd/setup.rs`).
Sequence the registry split first (smaller blast radius, no callers
to update outside the module), then this one.
