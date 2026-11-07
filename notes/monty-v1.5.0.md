# Monty v1.5.0 — Impact on fimod

Date: 2026-11-06
Previous: v1.0.0 → v1.5.0 (crates.io; intermediate releases v1.0.1, v1.1.0, v1.2.0, v1.3.0, 1.4.0)

## Summary

Feature release series with one compile-time break for fimod: `OsFunctionCall`
gains four variants (`Scan`, `Import`, `ImportStubs`, `GetTypeHints`), which makes
the exhaustive sandbox dispatcher in `src/engine.rs` fail to compile. Everything
else fimod consumes is unchanged or additive. The series brings major user-facing
additions (match statements, `hashlib`, `csv`, `tomllib`, `decimal`, `os.path`,
file imports).

Sources: releases [v1.1.0](https://github.com/pydantic/monty/releases/tag/v1.1.0),
[v1.2.0](https://github.com/pydantic/monty/releases/tag/v1.2.0),
[v1.3.0](https://github.com/pydantic/monty/releases/tag/v1.3.0),
[1.4.0](https://github.com/pydantic/monty/releases/tag/1.4.0),
[v1.5.0](https://github.com/pydantic/monty/releases/tag/v1.5.0). The API diff was
taken from the published `monty`/`monty-types` 1.0.0 and 1.5.0 crate sources
(the GitHub compare API truncates at 300 files).

## Implementation status

Implemented on branch `feat/monty-1.5.0` (uncommitted as of 2026-11-07):

- `monty` / `monty-types` pinned to `=1.5.0`; `dispatch_os_call` handles the four
  new variants through `OsCallOutcome::Unserved` (local imports stay denied by
  decision).
- Regression tests: `test_sandbox_local_module_import_is_not_served` (mold +
  REPL, helper file not executed), `test_monty_15_language_and_stdlib_additions`,
  `test_monty_decimal_result_is_not_serialized`; `test_version_flag` updated.
- Docs updated as listed below; `Decimal` JSON output recorded as a ROADMAP idea.
- Other dependencies bumped in the same branch: compatible `cargo update`
  (fixes RUSTSEC-2026-0346/0347/0348 in `rustls`, `webpki-root-certs` advisory,
  yanked `clipboard-win`), `fancy-regex` 0.20, `rustyline` 19, `serde-saphyr`
  2.1, `httpmock` 0.9 (dev), `minijinja` 3 (template rendering changes; see
  below).

Local verification: `task lint` (fmt, clippy, cargo-deny), `task test`
(240 lib, 451 cli, mold fixtures), `task lint:msrv` (Rust 1.96),
`task doc:build` all pass; `cargo outdated --root-deps-only` reports nothing.

### minijinja 3 — visible template changes

Measured with the same mold on fimod 0.12.1 (minijinja 2.24) and the branch:

| Template | Before | After |
|---|---|---|
| `{{ d\|tojson }}` | `{"a":1,"b":[1,"x"]}` | `{"a": 1, "b": [1, "x"]}` |
| `{{ ["x", 1] }}` | `["x", 1]` | `['x', 1]` |
| `{{ {"a": 1} }}` | `{"a": 1}` | `{'a': 1}` |
| `{{ 2.5\|round }}` / `{{ 0.125\|round(2) }}` | `3.0` / `0.13` | `2.0` / `0.12` (half-even) |
| `{{ 15\|round(-1) }}` | `15` | `20` |
| `{{ "don't stop"\|title }}` | `Don'T Stop` | `Don't Stop` |
| `{{ ['b','P','n']\|max }}` | `n` | `P` (case-insensitive) |
| `{{ 1 / 0 }}` | `inf` | render error (fails the mold) |

All align with Jinja2. Existing tests and public mold fixtures pass unchanged.
Decision (2026-11-07): keep minijinja 3, not classified as a breaking change;
user-facing behavior documented in `docs/guides/mold-scripting.md` (templates).

## API surface consumed by fimod

Regenerated on 2026-11-06 from multiline `use` blocks and qualified paths in
`src/`. The skill's single-line grep misses multiline `use monty_types::{...}`
blocks; a multiline-aware scan was used instead.

| Monty symbol | Fimod consumers | Role |
|---|---|---|
| `MontyRun`, `RunProgress` | `src/engine.rs` | Compile, run and resume molds. |
| `MontyRepl`, `ReplProgress`, `ReplContinuationMode`, `detect_repl_continuation_mode` | `src/cmd/monty.rs` | Interactive REPL execution and continuation. |
| `CompileOptions` | `src/engine.rs`, `src/cmd/monty.rs` | Compilation options. |
| `MontyObject` | 16 modules (`engine`, `convert`, `pipeline`, `format`, `dotpath`, `iter_helpers`, `monty_args`, `env_helpers`, `regex`, `hash`, `gatekeeper`, `msg`, `template`, `exit_control`, `format_control`, `cmd/monty`) | Value exchange between Monty and fimod. |
| `ObjectRef`, `unstable::{MontyNode, node, root_node, child}` | `convert`, `engine`, `iter_helpers`, `dotpath`, `monty_args`, `env_helpers`, `pipeline` | Borrowed graph inspection (unstable API). |
| `MontyDate`, `MontyDateTime`, `MontyTime`, `MontyTimeDelta`, `MontyTimeZone`, `MontyUuid` | `convert`, `engine`, `iter_helpers` | Date/time fidelity, host-instance identity. |
| `OsFunctionCall`, `OsPolicy`, `DateTimeSource`, `RandomStart`, `SleepMode` | `src/engine.rs` | Sandbox host-call dispatch and policy. |
| `ExtFunctionResult`, `NameLookupResult` | `src/engine.rs`, `src/cmd/monty.rs` | Answers to host calls and name lookups. |
| `PrintWriter`, `PrintWriterCallback` | `src/engine.rs`, `src/cmd/monty.rs` | Print routing (stdout/stderr). |
| `ResourceLimits`, `ResourceTracker` | `src/engine.rs`, `src/cmd/monty.rs` | Duration, memory, suspension limits. |
| `BASELINE_MEMORY`, `LIVE_MEMORY` | `src/mem_limit.rs` | Allocator-backed memory accounting. |
| `MontyException`, `ExcType` | `src/engine.rs`, `src/cmd/monty.rs` | Error construction and classification. |

## Changes impacting fimod

| Symbol | Changed? | Nature | Action |
|---|---|---|---|
| `OsFunctionCall` | yes | +4 variants: `Scan(ScanArgs)`, `Import(ImportArgs)`, `ImportStubs(Vec<String>)`, `GetTypeHints(TypeHintsTarget)`; new helper `no_handler_result()` | **Breaking** — `dispatch_os_call` (`src/engine.rs:1178`) must handle them |
| `unstable::MontyNode` | yes | +`Complex`, `Decimal(String)`, `Module { .. }` | Compiles (`other =>` arms); `Decimal`/`complex` mold results fail JSON conversion with "Cannot convert MontyObject variant" |
| `ExtFunctionResult` | yes | +`ModuleFile(ModuleFile)` | None — fimod constructs, does not match |
| `ExcType` | yes | +`TomlDecodeError`, `CsvError`, `Decimal*` | None — fimod matches with fallbacks |
| `ResourceLimits` | yes | +`with_execution_clock`, `tighten_limits` (additive) | None |
| `MontyRepl` | yes | +`set_os_policy`, `tracker_mut`, `next_script_name` (additive) | None |
| `MontyRun`, `RunProgress`, `ReplProgress` | internal only | Result export now runs under the execution clock; conversion errors surface as `Error` | None |
| `MontyObject` | yes | +`complex()`, `decimal()`, `module()`, `as_decimal()`, `as_complex()` | None |
| `unstable::PushValue` | yes | +impls for `u32`, `Option<T>` | None |
| Other consumed symbols | no | — | — |

Dependency effects (`cargo update -p monty -p monty-types`): `fancy-regex`
0.17 → 0.20 inside Monty (fimod's own direct dependency stays 0.19, so two
versions remain as before); `phf` 0.12 added (MIT, already allowed);
`sha2 0.10.9` removed. MSRV unchanged at Rust 1.96.

## Breaking changes

- **`OsFunctionCall` exhaustive match** — `src/engine.rs:1179`
  (`dispatch_os_call`). Proposed handling, consistent with the current
  "no filesystem until mounts" policy and with Monty's own non-iterative
  defaults (`run.rs`):
  - `Scan(_)` → join the filesystem group returning `None` (same as `Iterdir`).
  - `Import(_)`, `ImportStubs(_)`, `GetTypeHints(_)` → `call.no_handler_result()`
    (import → `ModuleNotFoundError`; type hints → `{}`), via a new
    `OsCallOutcome` variant carrying the `ExtFunctionResult`.

  Because fimod drives Monty iteratively, it does **not** inherit Monty's
  automatic fallback: without this handling every unresolved `import x` would
  reach the host. Affects both mold execution and the REPL (shared
  `sandbox_os_call_result`).

## New capabilities unlocked

User-facing, for mold authors:

- `match` statements (v1.2.0).
- New stdlib modules: `hashlib` (1.2), `os.path` (1.2), `csv` (1.2),
  `tomllib` (1.2), `bisect` (1.2), `decimal` (1.4), `fnmatch` (1.5),
  `annotationlib` (1.5); `typing.get_type_hints()` (1.5).
- `complex` type (1.1).
- `dataclasses.field()`, `default_factory`, `__post_init__` (1.2) — current docs
  say `__post_init__` is rejected.
- List slice assignment, set/frozenset ordering comparisons (1.2).
- `re.split()` now includes capture groups (1.2); regex patterns parsed with
  CPython's grammar (1.5); `(?x)` trailing-comment fix (1.3).
- `json.dumps(default=...)` (1.5); `datetime.fromtimestamp()`,
  `date.fromtimestamp()` (1.5); `float(bytes)` (1.5).
- Exceptions record `__cause__` / `__context__` (1.5).
- Performance: O(1) `len`/indexing on ASCII strings, timsort port, inline
  strings, f-string and dict-literal preallocation.

Not unlocked by fimod (sandbox denies them): file imports of local `.py`
modules, `json.load`/`json.dump` and `open()`-based `csv` usage on files.

## Follow-up opportunities (not part of the upgrade)

- **Local module imports**: Monty now asks the host to resolve
  `import helpers` from `.py` files. Serving this (e.g. from the mold's
  directory) would enable shared mold libraries — a sandbox/design decision.
- **`Decimal` / `complex` at the JSON boundary**: today they error at
  output. `Decimal` could serialize as a JSON number string or exact number.
- **`hs_*` vs `import hashlib`**: overlap similar to `re_*` vs `import re`;
  candidate for the legacy-built-in path. Docs list `hs_*` prominently.
- **`fancy-regex` dedupe**: bumping fimod's direct dependency to 0.20 would
  remove the duplicate; it affects legacy `re_*_fancy` behavior.

## Documentation to update with the bump

- `docs/reference/monty-engine.md`: version (l.5), "Match statements — Coming
  soon" (l.53), stdlib table (l.65+), `__post_init__` statement (l.229), REPL
  banner example (l.246), sandbox table (add local-import → `ModuleNotFoundError`).
- `docs/reference/built-ins.md`: `re_split` note (l.50) — captured separators
  are now returned by `re.split`; "Monty 1.0.0 has limitations" (l.56).
- `docs/guides/comparison.md:64`: hashing row now has native `import hashlib`.
- Version pins in `README.md:461`, `docs/index.md:250`,
  `docs/guides/concepts.md:102`, `notes/DESIGN_NOTES.md` (Monty 1.0.0 boundary).

## Upgrade steps

1. `Cargo.toml`: `monty = "=1.5.0"`, `monty-types = "=1.5.0"`;
   `cargo update -p monty -p monty-types`.
2. Handle the four `OsFunctionCall` variants in `dispatch_os_call`.
3. Add sandbox regression tests: `import <local name>` → `ModuleNotFoundError`
   (mold + REPL), `Path.scan`-style call stays denied.
4. Update the documentation listed above.
5. `rtk task lint`, `rtk task test`, `rtk task lint:msrv`.

## Validation performed (scratch copy, not the repository)

A throwaway copy pinned to 1.5.0, with the dispatcher handling above prototyped
(`OsCallOutcome::Unhandled(ExtFunctionResult)`), was used. This is local
evidence on that copy, not on the repository.

- `cargo check --all-targets`: single error, `E0004` on `OsFunctionCall`
  (`src/engine.rs:1179`).
- `cargo test --no-fail-fast` with the prototype: lib 240/240, cli 447/448,
  other targets green. The only failure is `args::test_version_flag`
  (`tests/cli/args.rs:533`), which hardcodes `(Monty 1.0.0)`; the binary
  reports `(Monty 1.5.0)` as expected. Existing sandbox tests
  (`import subprocess`, `import socket`, pathlib, `open`) pass unchanged.
- Smoke checks on the built binary:
  - `match` statement in a mold → works.
  - `hashlib`, `tomllib`, `bisect`, `fnmatch`, `os.path` → work.
  - `import helpers` with `helpers.py` in the cwd → `ModuleNotFoundError: No
    module named 'helpers'` (file imports stay denied).
  - `re.split(r"(\d)", "a1b2c")` → `["a","1","b","2","c"]`.
  - Returning `Decimal("1.10")` → `Cannot convert MontyObject variant to JSON`.

## Risk

Low to medium. The compile break is a single, mechanical match extension, and
the full suite passes apart from a version-string assertion. Medium only because
the series spans ~150 upstream PRs with behavioral changes fimod tests
cover partially: regex parsing now follows CPython's grammar, sort is a timsort
port, container `repr` can raise `MemoryError` instead of truncating, and
`str.splitlines` / `str.center` match CPython more closely. User molds relying on
the previous quirks may see different output. The new `Import` host call is
security-relevant, but the proposed handling keeps it denied.

## Recommendation

Upgrade now. Five minor releases of fixes plus high-value mold features
(match, `hashlib`, `csv`, `tomllib`, `decimal`) for a one-site adaptation.
Keep local-module imports and `Decimal` JSON output as separate design
decisions.
