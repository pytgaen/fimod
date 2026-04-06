# Monty v0.0.11 — Upgrade Analysis

**Date**: 2026-04-10
**Previous version in fimod**: v0.0.10
**Release**: https://github.com/pydantic/monty/releases/tag/v0.0.11

## Changes

### PR #314 — JSON perf improvements

Major rewrite of `json.loads()` and `json.dumps()` internals for significant performance gains.

#### `json.loads()` — ~2x faster than CPython

- **String cache** (`JsonStringCache`): per-run cache of 16,384 slots that deduplicates heap allocations for repeated strings (especially dict keys like `"id"`, `"name"`, `"type"`). Strings between 2–64 bytes are eligible. Lazily initialized — zero cost if `json.loads()` is never called. Integrated with GC (cached values are registered as roots).
- **Optimized dict insertion** (`set_json_string_key`): bypasses the generic `py_eq`/candidate-cloning lookup for JSON object keys (which are always strings). Compares keys directly by string content — much cheaper than full Python equality.
- **Direct dict construction**: `json.loads()` now builds dicts incrementally via `set_json_string_key` instead of collecting pairs into a `Vec` and calling `Dict::from_pairs`. Avoids an intermediate allocation.
- **Duplicate key semantics**: `{"a": 1, "a": 2}` now correctly keeps the last value (CPython-compatible), verified by new test.

Benchmarks (geometric mean across 11 fixtures):

| Operation | Monty vs CPython |
|-----------|-----------------|
| `json.loads()` | **2.02x faster** |
| `json.dumps()` | **1.65x faster** |

#### `json.dumps()` — batch escape strategy

- **Lookup table**: 256-byte static table classifies each byte in O(1) (inspired by serde_json). Contiguous runs of safe bytes are flushed with a single `push_str` instead of character-by-character iteration.
- **`skipkeys` fix**: replaced O(n²) `Vec::remove(i)` loop with a two-pointer compaction pass — O(n) with no shifting. Extensive new tests for edge cases (nested dicts, combined with `indent`/`sort_keys`, bytes keys, empty dicts).
- **`sort_keys` optimization**: `apply_permutation` is now generic (`<T>`) and operates in-place on `(Value, Value)` entries directly, avoiding the clone+allocate path from v0.0.10.
- **Unicode escaping**: new tests for emoji surrogate pairs (`😀` → `\ud83d\ude00`), DEL character, mixed escape sequences.

### PR #310 — Mount fixes for edge cases

Filesystem mount system improvements (used by monty-python/monty-js bindings, **not by fimod**):

- **`mkdir` semantics**: `exist_ok=True` on an existing **file** now raises `FileExistsError` (matches CPython `pathlib.Path.mkdir()`). Previously it silently succeeded.
- **Mount rollback**: if `take_shared_mounts()` fails at slot N, slots 0..N-1 are restored (prevents permanent mount loss on partial failure).
- **Rename validation**: rejects file→directory and directory→non-empty-directory renames (POSIX semantics).
- **Rename**: `MountDirectory` → `MountDir` (internal naming).

## Impact on fimod

### API compatibility: no breaking changes

The public API surface used by fimod is unchanged:
- `MontyObject`, `MontyRun`, `NameLookupResult`, `NoLimitTracker`
- `PrintWriter`, `PrintWriterCallback`, `RunProgress`
- `DictPairs`, `MontyRepl`, `detect_repl_continuation_mode`
- `MontyException`, `ReplContinuationMode`

No changes to `lib.rs`, runner, or any public-facing types.

### Free performance gains

Molds that use `import json` will automatically benefit:
- `json.loads()` ~2x faster (string cache + optimized dict insertion)
- `json.dumps()` ~1.65x faster (lookup table escape + in-place sort)
- `skipkeys=True` is now correct for all edge cases

Note: fimod's own data pipeline uses `serde_json` (Rust-side), not Monty's `json` module. The perf gains only apply to mold scripts that explicitly `import json`.

### Mount fixes: neutral

fimod does not implement filesystem access — all `OsCall` requests return `None`. The mount fixes in PR #310 have no effect.

### No risk

- No API changes → drop-in upgrade
- All changes are internal to Monty's VM, json module, and fs layer
- `apply_permutation<T>` generalization is backwards-compatible

## Upgrade steps

1. Bump `Cargo.toml`: `tag = "v0.0.10"` → `tag = "v0.0.11"`
2. `cargo build` — should compile without changes
3. `cargo test` — all tests should pass
4. `cargo clippy` — no new warnings expected

## What this enables for fimod

No new fimod features are unlocked by this release. It's a pure performance + correctness improvement for mold scripts using `import json`.

Future Monty releases to watch for:
- **Classes** — would enable richer mold patterns
- **Match statements** — Python 3.10+ pattern matching
- **Context managers** — `with` statements
