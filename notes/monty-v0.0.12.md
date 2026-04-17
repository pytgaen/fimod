# Monty v0.0.12 — upgrade analysis

**Date:** 2026-04-14
**Current:** v0.0.11 → **Target:** v0.0.12
**Risk:** low
**Recommendation:** upgrade now

## Summary of PRs

- #319 — REPL type checking (monty-python layer, not core)
- #320 — JSON load/dump benchmarks (internal)
- #321 — Type-checking performance improvements
- #322 — New `CollectString` / `CollectStreams` print output variants
- #324 — `zip(strict=True)` enforcing equal-length iterables (Python-visible)
- #325 — Datatests / benches moved to separate crates (repo layout only)
- #326 — Replace `vm.cleanup()` with `Drop` impl (internal)
- #328 — Rust coverage for Python tests (CI)

## Impact on fimod

**Imports surveyed** (`src/`): `MontyObject`, `MontyRun`, `MontyRepl`, `NameLookupResult`,
`NoLimitTracker`, `PrintWriter`, `PrintWriterCallback`, `RunProgress`, `DictPairs`,
`detect_repl_continuation_mode`, `ReplContinuationMode`, `MontyException`.

### Breaking changes upstream → fimod impact

| Change | fimod uses? | Action |
|---|---|---|
| `PrintWriter::Collect` → `CollectString` (+ new `CollectStreams`) | No — fimod only uses `Stdout` / `Callback` | none |
| `vm.cleanup()` removed (now `Drop`) | No — fimod never calls it | none |
| `zip(strict=...)` | Python-level feature | opt-in for mold authors |
| Datatests/benches crate move | No | none |

**Conclusion:** no code change required in fimod beyond the tag bump.

### Benefits

- Faster type checking (PR #321) — benefits REPL mode (`fimod repl`).
- Cleaner VM lifecycle via `Drop` — fewer error paths can leak VM state.
- `zip(strict=True)` available in molds — useful for pairing columns/rows safely.

## Upgrade steps

1. `sed -i 's/tag = "v0.0.11"/tag = "v0.0.12"/' Cargo.toml`
2. `cargo build && cargo test && cargo clippy`
3. Update `docs/reference/monty-engine.md` version reference if it pins v0.0.11.

## What this enables for fimod users

- `zip(a, b, strict=True)` in molds to catch length mismatches loudly.
- Slightly faster REPL typing feedback (once fimod opts into type-check API — future work).
