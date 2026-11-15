# Monty v1.6.0 — Impact on fimod

Date: 2026-11-12
Previous: v1.5.0 → v1.6.0 (crates.io)

## Summary

Mostly internal release (pool/proto/remote work, perf fast paths, CPython parity
fixes). One compile-time break for fimod: `OsFunctionCall` gains the
`CreateExclusive` variant, which makes the exhaustive sandbox dispatcher in
`src/engine.rs` fail to compile. MSRV rises from Rust 1.96 to 1.98. Everything
else fimod consumes is unchanged or additive.

Sources: release [v1.6.0](https://github.com/pydantic/monty/releases/tag/v1.6.0).
The API diff was taken from a local clone (`git diff v1.5.0 v1.6.0`), because the
GitHub compare API truncates patches for this release (228 files).

## Implementation status

Implemented on branch `feat/monty-1.6.0` (2026-11-12):

- `monty` / `monty-types` pinned to `=1.6.0`, `rust-version = "1.98"`;
  `dispatch_os_call` handles `CreateExclusive` in the `open()` arm.
- MSRV references updated (CI, Taskfile, DESIGN_NOTES, CONTRIBUTING — the latter
  was stale at 1.95); DESIGN_NOTES watchpoint records the new variant.
- Docs version mentions bumped. The three `re` limitations in
  `docs/reference/built-ins.md` were rechecked on 1.6.0 and still hold.
- No dedicated regression test: the variant has no Python API, so it is
  unreachable from molds; the shared `open()` arm is covered end to end.

- Other dependencies bumped in the same branch: compatible `cargo update`
  (no major bump available; `cargo outdated --root-deps-only` is clean). Root
  deps: `minijinja` 3.0.5 → 3.1.0, `fancy-regex` 0.20.0 → 0.20.2, `notify`
  8.2.1 → 8.2.2. Transitive: `jiff` 0.2.43, `rustls-pki-types` 1.16.0,
  `webpki-root-certs` 1.0.12 and minor crates; `bitflags` 1 and `defmt*` dropped.
  `generic-array` stays at 0.14.7 (pinned upstream).

Local verification: `task lint` (fmt, clippy, cargo-deny), `task test`
(240 lib, 451 cli, mold fixtures), `task lint:msrv` (Rust 1.98),
`task doc:build` all pass.

### Dependency changes visible to users

- `minijinja` 3.1.0 (Jinja2 compatibility fixes, from its release notes):
  booleans compare and sort like `0`/`1`; `max` and `groupby` return the
  first of equal items; `first`, `last`, `length`, `count`, `items`, `reverse`,
  `join`, `groupby` treat `none`/undefined like Jinja2; `upper`/`lower`/
  `capitalize`/`title` handle final sigma and special case mappings; `reverse`
  and `last` work on maps; negative string/sequence repetition yields an empty
  result; `split(none, n)` no longer returns trailing whitespace; `replace`
  accepts `count`. Fuel accounting changed, but fimod sets no fuel budget.
- `fancy-regex` 0.20.2: soundness fixes in its optimizer (#333) plus 0.20.1
  fixes (atomic lookaheads, inline flags scoped to their group, backrefs inside
  their own group). Only affects the `re_*_fancy` helpers on exotic patterns.

Existing tests and public mold fixtures pass unchanged.

## API surface consumed by fimod

Regenerated from `src/` on 2026-11-12.

| Monty symbol | fimod consumers | Role in fimod |
|---|---|---|
| `monty::MontyRun`, `RunProgress` | `engine.rs` | Compile and drive mold execution (suspend/resume loop) |
| `monty::MontyRepl`, `ReplProgress`, `detect_repl_continuation_mode`, `ReplContinuationMode` | `cmd/monty.rs` | `fimod monty repl` |
| `CompileOptions`, `ExcType`, `MontyException` | `engine.rs`, `cmd/monty.rs` | Compilation and error reporting |
| `ExtFunctionResult`, `NameLookupResult` | `engine.rs`, `cmd/monty.rs` | Host function / name lookup answers |
| `OsFunctionCall` (exhaustive `match`) | `engine.rs` (`dispatch_os_call`) | Sandbox policy for OS calls |
| `OsPolicy`, `DateTimeSource`, `RandomStart`, `SleepMode` | `engine.rs` | Sandbox runtime configuration |
| `ResourceLimits`, `ResourceTracker`, `BASELINE_MEMORY`, `LIVE_MEMORY` | `engine.rs`, `cmd/monty.rs`, `mem_limit.rs` | Duration, memory and suspension limits |
| `PrintWriter`, `PrintWriterCallback` | `engine.rs`, `cmd/monty.rs` | Route mold `print()` to stderr |
| `MontyObject`, `ObjectRef` | most modules | Values carried between mold steps |
| `unstable::{node, root_node, child, MontyNode}` | `convert.rs`, `iter_helpers.rs`, `dotpath.rs`, `pipeline.rs`, `monty_args.rs` | Graph walk for JSON conversion and helpers |
| `MontyDate`, `MontyDateTime`, `MontyTime`, `MontyTimeDelta`, `MontyTimeZone`, `MontyUuid` | `convert.rs`, `engine.rs` | Date/time/UUID conversion and clock calls |

## Changes impacting fimod

| fimod symbol | Changed? | Nature | Action |
|---|---|---|---|
| `OsFunctionCall` | yes | variant `CreateExclusive(CreateExclusiveCallArgs)` added (not `#[non_exhaustive]`) | **Compile break** — handle in `dispatch_os_call` (`src/engine.rs:1179`) |
| `MontyNode::Function` | yes | field `is_async: bool` added | None — fimod never matches or builds `Function` |
| `ReplFunctionCall` | yes | pub field `is_async`, methods `resume_in_turn` / `resume_pending_in_turn` added | None — fimod calls `into_repl()` only |
| `ExtFunctionResult` | yes | helper `not_found_exception()` added | None |
| `MontyObject` | yes | constructor `async_function()` added | None |
| `ResourceTracker` / `ResourceLimits` | doc comment only | — | None |
| `MontyRun`, `RunProgress`, `MontyRepl`, `PrintWriter`, `CompileOptions`, `MontyException`, `unstable::*`, date/time types | no | — | None |
| Crate `rust-version` | yes | 1.96 → 1.98 | Bump fimod MSRV (Cargo.toml, CI, Taskfile, DESIGN_NOTES) |

Lockfile impact: only `monty`, `monty-types`, `monty-macros` move to 1.6.0; no new
transitive dependency.

## Breaking changes

1. `OsFunctionCall::CreateExclusive` — exhaustive `match` in
   `src/engine.rs:1179` (`dispatch_os_call`). Upstream (#1245) adds it as a host
   operation that creates an empty file and returns a file handle; it is a
   prerequisite for `tempfile` and **has no sandbox Python API yet**, so molds
   cannot reach it today. Proposed handling: same as `open()` —
   `call.on_no_handler()` → `PermissionError: Permission denied: '<path>'`.
   Returning `None` (legacy `Path.*` behavior) would hand mold code a non-file
   object, which is exactly why `open()` already raises.
2. MSRV 1.96 → 1.98 (build-level, not user-facing for binary users):
   - `Cargo.toml:12` `rust-version`
   - `.github/workflows/ci.yml:83-89` MSRV job name and toolchain
   - `Taskfile.yml:76` `cargo +1.96 check`
   - `notes/DESIGN_NOTES.md:277` and `:315`
   - `.github/CONTRIBUTING.md:57` — already stale (says 1.95 while the MSRV is 1.96)

## Behavior changes visible to molds

None requires adaptation of existing tests or public molds (prototype run, see
below). For user awareness:

- Argument-count `TypeError` tracebacks now point at the call site, not the
  `def` (#1184).
- `pathlib` pure operations match CPython: `with_name`/`with_stem`/`with_suffix`
  error messages, dot-name `stem`/`suffix` splitting (`.bashrc` has no suffix),
  `PosixPath` repr quoting; non-`str` arguments to `with_*` raise `TypeError`
  (#1213).
- Perf: numeric fast paths for binary arithmetic (#1090), linear nested-scope
  analysis (#1288), builtin constructors reuse immutable inputs (#1266).
- Robustness: namedtuple clones released on memory-limit errors (#1267),
  `JSONDecodeError` doc length checked before copy (#1290), `list`/`tuple`
  `.index(x, start)` starts at the offset (#1270).

## New capabilities unlocked

None for mold authors: no new stdlib module, no new syntax. Async host functions
(`MontyObject::async_function`, REPL-only streaming) are not used by fimod.

## Upgrade steps

1. `Cargo.toml`: `monty = "=1.6.0"`, `monty-types = "=1.6.0"`,
   `rust-version = "1.98"`; `cargo update -p monty -p monty-types`.
2. `src/engine.rs`: add `OsFunctionCall::CreateExclusive(_)` to the `Open` arm;
   extend the `dispatch_os_call` doc comment accordingly.
3. `tests/cli/args.rs` `test_version_flag`: `(Monty 1.6.0)`.
4. MSRV references listed above.
5. Docs version mentions: `docs/reference/monty-engine.md` (lines 5, 256),
   `docs/index.md:250`, `docs/guides/concepts.md:102`, `README.md:461`,
   `docs/reference/built-ins.md:56`.
6. `task lint`, `task test`, `task lint:msrv`, `task doc:build`.

## Prototype verification (2026-11-12)

Throwaway copy of `HEAD` (`9b5db44`) outside the repo, steps 1–3 applied:

- Before step 2: exactly one error, `E0004 OsFunctionCall::CreateExclusive(_) not covered`.
- `cargo test --all-features`: 240 lib, 3 + 3 bin, 451 cli, 2 mold-fixture
  suites — all pass (only `test_version_flag` needed step 3).
- `cargo clippy --all-features --all-targets -D warnings`: clean.
- `cargo +1.96 check`: refused (`monty@1.6.0 requires rustc 1.98`), as expected.
- `cargo +1.98 check --all-features`: passes.

## Risk

Low. Single, mechanical compile fix on a variant unreachable from molds; no
change to the APIs fimod drives (run/REPL loops, conversion graph, limits).
Only cost: MSRV bump to 1.98.

## Recommendation

Upgrade now. Ships perf and robustness fixes with near-zero adaptation cost.
