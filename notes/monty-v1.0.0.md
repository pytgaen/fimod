# Monty v1.0.0 — Impact on fimod

Date: 2026-09-25
Previous: v1.0.0-beta.2 → v1.0.0

## Summary

Final 1.0 release after beta.2: no compile-time migration identified for fimod. The 13 upstream commits add source positions to suspensions, change snapshot encoding, and refine Python `copy` and memory accounting. Fimod does not persist Monty snapshots.

## Implementation status

The local checkout now pins `monty` and `monty-types` to `=1.0.0` and has a regenerated lockfile. No fimod Rust API adaptation was needed. Public engine documentation and design notes now name v1.0.0. The existing beta.2 migration remains uncommitted alongside this upgrade.

`minicbor` and `minicbor-serde` enter through Monty v1.0.0. Their `BlueOak-1.0.0` license was explicitly allowed in `deny.toml` after user approval; see the [SPDX license record](https://spdx.org/licenses/BlueOak-1.0.0.html).

Local verification: `rtk cargo build`, `rtk task lint`, `rtk task test` (689 passed, 0 failed; 12 performance tests and one doctest ignored by default), and `rtk task lint:msrv` all passed. This is local validation, not a release or distribution check.

Sources: [release](https://github.com/pydantic/monty/releases/tag/v1.0.0), [beta.2 → v1.0.0 comparison](https://github.com/pydantic/monty/compare/v1.0.0-beta.2...v1.0.0).

## API surface consumed by fimod

Regenerated from multiline imports and qualified `monty::` / `monty_types::` uses in `src/` on 2026-09-25. The table lists 31 named symbols; the `unstable` module itself is a namespace, not an additional symbol.

| Monty symbol | Fimod consumers | Role |
|---|---|---|
| `MontyRun`, `RunProgress` | `src/engine.rs` | Compile, run and resume molds. |
| `MontyRepl`, `ReplProgress`, `ReplContinuationMode`, `detect_repl_continuation_mode` | `src/cmd/monty.rs` | Interactive execution and continuation. |
| `CompileOptions` | `src/engine.rs`, `src/cmd/monty.rs` | Configure compilation. |
| `MontyObject` | `src/engine.rs`, `src/convert.rs`, `src/pipeline.rs`, `src/format.rs`, `src/dotpath.rs`, `src/iter_helpers.rs`, `src/monty_args.rs`, `src/env_helpers.rs`, `src/regex.rs`, `src/hash.rs`, `src/gatekeeper.rs`, `src/msg.rs`, `src/template.rs`, `src/exit_control.rs`, `src/format_control.rs`, `src/cmd/monty.rs` | Exchange values between Monty and fimod. |
| `ObjectRef`, `MontyNode` | `src/convert.rs`, `src/engine.rs`, `src/iter_helpers.rs`, `src/dotpath.rs`, `src/monty_args.rs`, `src/env_helpers.rs`, `src/pipeline.rs` | Borrow and inspect the value graph; `MontyNode` belongs to the unstable API. |
| `MontyDate`, `MontyDateTime`, `MontyTime`, `MontyTimeDelta`, `MontyTimeZone` | `src/convert.rs`, `src/engine.rs`, `src/iter_helpers.rs` | Dates, times and JSON serialization. |
| `MontyUuid` | `src/engine.rs` | Host object identity. |
| `NameLookupResult`, `ExtFunctionResult` | `src/engine.rs`, `src/cmd/monty.rs` | Resolve names and answer host calls. |
| `OsFunctionCall`, `OsPolicy`, `DateTimeSource`, `RandomStart`, `SleepMode` | `src/engine.rs` | Apply sandbox policy to OS requests. |
| `PrintWriter`, `PrintWriterCallback` | `src/engine.rs`, `src/cmd/monty.rs` | Route Python output. |
| `ExcType`, `MontyException` | `src/engine.rs`, `src/cmd/monty.rs` | Translate and raise errors. |
| `ResourceLimits`, `ResourceTracker`, `LIVE_MEMORY`, `BASELINE_MEMORY` | `src/engine.rs`, `src/cmd/monty.rs`, `src/mem_limit.rs` | Enforce time and memory limits. |

## Changes impacting fimod

| Consumed symbol / path | Changed? | Nature | Action |
|---|---|---|---|
| `RunProgress`, `ReplProgress` suspension payloads | Yes | `FunctionCall`, `OsCall`, `NameLookup` and future suspension objects expose a new `position: SourceRange`. Their variants and resume methods remain available. | No code adaptation: fimod receives these objects and does not construct or destructure them. Consider source positions later only if diagnostics need them. |
| `ResourceTracker`, `BASELINE_MEMORY` | Indirectly | Monty can charge lazily built type-checker state to the process baseline. No consumed signature or limit field changed. | Recheck memory-limit behavior after the bump. |
| `MontyObject`, `MontyNode`, `MontyUuid`, `MontyException`, `OsPolicy` | Serialization only | CBOR snapshot encoding and byte-oriented serde annotations; the consumed in-memory Rust shapes remain compatible in the inspected diff. | No fimod migration; it does not call Monty dump/load APIs. |
| All other symbols above | No relevant change found | No changed signature, enum variant or required construction field in the upstream comparison. | Compile and run the focused tests to confirm. |

## Breaking changes

None identified for the consumed API. The new suspension fields would matter to code constructing those upstream structs directly; fimod does not do that. Monty snapshot format changed (`DUMP_VERSION` 12 → 13), but fimod does not persist snapshots.

## New capabilities unlocked

No newly added stdlib module was found between beta.2 and v1.0.0. Existing `copy` and `deepcopy` behavior was refined; a nested-comprehension compiler panic was fixed. These are runtime fixes, not new fimod features. The source position on host suspensions could improve future diagnostics, but fimod does not expose it today.

## Upgrade steps

1. Exact pins for both Monty crates and the lockfile: done.
2. Engine documentation, design notes and license allowlist: done.
3. Build, full lint/test suite and Rust 1.96 MSRV check: passed. The suite includes host-object, copy, sandbox and memory-limit regression tests.
4. Before a PR or release, move the uncommitted migration onto a dedicated branch and follow the repository's Git workflow.

## Risk

**Low to medium.** No consumed public API break was found, and local build/tests passed on v1.0.0. Fimod still relies on `monty_types::unstable` graph access and allocator-backed memory accounting; local checks do not cover every platform or release artifact.

## Recommendation

**Local upgrade is ready for review.** The v1.0.0 dependency, documentation and license changes passed the project gates. Keep the existing beta.2 work and this follow-up together when preparing the dedicated branch/PR; no commit, push or release was performed.
