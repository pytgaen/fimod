# Monty v0.0.9 → v0.0.10 — Changelog Analysis

**Date**: 2026-04-10
**Release**: https://github.com/pydantic/monty/compare/v0.0.9...v0.0.10

## Key PRs

### 1. Mounting filesystems (#305) — @samuelcolvin

New `MountTable` system for sandboxed filesystem access:

- `Mount` struct: `host_path`, `sandbox_path`, `mode`
- 3 modes: `ReadOnly`, `ReadWrite`, `OverlayMemory` (copy-on-write in memory)
- Security: path canonicalization, TOCTOU protection, symlink escape detection, PATH_MAX/NAME_MAX enforcement
- New exports: `OsFunction`, `dir_stat`, `file_stat`, `stat_result`, `symlink_stat`

`RunProgress::OsCall` still exists — backwards compatible. The mount system is an **opt-in** layer on top.

**Opportunity for fimod**: could offer `--mount ./data:/data:ro` to let molds read files via `Path(...)`.

### 2. Async in Rust (#280) — @samuelcolvin

Moved async execution logic from Python to Rust. Thread cancellation support. `RunProgress::ResolveFutures` unchanged.

No impact on fimod (we bail on `ResolveFutures`).

### 3. Deduplicate Executor/ReplExecutor (#303) — @davidhewitt

Merged `ReplExecutor` into `Executor`. Internal refactor only — `MontyRepl` public API unchanged (`new`, `feed_run`, `feed_start`).

### 4. Multi-module import (#296) — @ilblackdragon

`import sys, math, re` now works (was limited to single imports). Parser/compiler change only.

### 5. CI/tooling (#301, #308, #311, #312, #313)

No runtime impact.

## Impact on fimod code

| File | Impact | Action |
|------|--------|--------|
| `Cargo.toml` | Tag `v0.0.9` → `v0.0.10` | **Update** |
| `engine.rs` | All types stable (`RunProgress`, `MontyRun`, `NameLookupResult`, etc.) | None |
| `main.rs` (REPL) | `MontyRepl::new`, `feed_run`, `detect_repl_continuation_mode` unchanged | None |
| `convert.rs` | `MontyObject`, `DictPairs` unchanged | None |

## Impact on docs/reference/monty-engine.md

- Version bump `v0.0.9` → `v0.0.10`
- "Not Yet Supported" table: remove single-import limitation, note `import a, b, c` works
- OsAccess section: mention new `MountTable` mechanism, note fimod still returns `None`
- "What This Means" section: add multi-import syntax

## New exports in monty crate (v0.0.10)

- `ExcType`, `CodeLoc`, `StackFrame`
- `MontyDate`, `MontyDateTime`, `MontyTimeDelta`, `MontyTimeZone`
- `InvalidInputError`
- `OsFunction`, `dir_stat`, `file_stat`, `stat_result`, `symlink_stat`
- `ReplFunctionCall`, `ReplNameLookup`, `ReplOsCall`, `ReplProgress`, `ReplResolveFutures`, `ReplStartError`
- `DEFAULT_MAX_RECURSION_DEPTH`, `LimitedTracker`, `ResourceError`, `ResourceLimits`
- `ExtFunctionResult`
