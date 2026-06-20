# Monty v0.0.18 - Impact on fimod

Date: 2026-06-09
Previous: v0.0.17 -> v0.0.18

Sources:
- Release: https://github.com/pydantic/monty/releases/tag/v0.0.18
- Compare: https://github.com/pydantic/monty/compare/v0.0.17...v0.0.18

## Summary

Monty v0.0.18 is a large hardening and feature release with one direct breaking
public API change for fimod: OS calls are now represented by the typed
`OsFunctionCall` enum instead of `OsFunction` plus raw `args` / `kwargs`.

Upgrade status: `src/engine.rs`, sandbox tests, docs, lockfile, and MSRV were
adapted for the bump. Validation must run under Rust 1.95 or newer.

## API surface consumed by fimod

| Monty symbol | Fimod consumers | Role in fimod |
|---|---|---|
| `DictPairs` | `src/convert.rs`, `src/engine.rs`, `src/env_helpers.rs`, `src/format.rs`, `src/iter_helpers.rs`, `src/regex.rs`, `src/template.rs` | Build and inspect Monty dicts while preserving insertion order. |
| `ExcType` | `src/engine.rs` | Build sandbox `PermissionError`; translate `TimeoutError` / `MemoryError` into fimod exit 137. |
| `ExtFunctionResult` | `src/engine.rs` | Resume Monty after host-side OS dispatch with return values or exceptions. |
| `LimitedTracker` | `src/engine.rs` | Apply fimod sandbox resource limits to Monty execution. |
| `MontyDate` | `src/convert.rs`, `src/engine.rs` | Return `date.today()` when clock is allowed; serialize date outputs. |
| `MontyDateTime` | `src/convert.rs`, `src/engine.rs` | Return `datetime.now()` when clock is allowed; serialize datetime outputs. |
| `MontyException` | `src/engine.rs` | Translate Python errors, implement print callback errors, and represent denied OS calls. |
| `MontyObject` | most built-ins plus `src/convert.rs`, `src/engine.rs`, `src/pipeline.rs`, `src/test_runner.rs` | Core bridge value between Rust and Monty. |
| `MontyRun` | `src/engine.rs` | Compile and start mold execution. |
| `NameLookupResult` | `src/engine.rs` | Expose fimod built-ins and `Step` to unresolved-name lookups. |
| `OsFunction` | `src/engine.rs` | Current fimod OS-call discriminator. Removed/replaced in v0.0.18. |
| `PrintWriter` | `src/engine.rs`, `src/cmd/monty.rs` | Route Monty `print()` to stdout or debug stderr callback. |
| `PrintWriterCallback` | `src/engine.rs` | Implement `StderrPrint` for debug mode. |
| `ResourceLimits` | `src/engine.rs` | Convert `SandboxPolicy` into Monty resource caps. |
| `RunProgress` | `src/engine.rs` | Iterative execution loop for function calls, OS calls, futures, name lookup, and completion. |
| `MontyRepl` | `src/cmd/monty.rs` | `fimod monty repl` session state. |
| `NoLimitTracker` | `src/cmd/monty.rs` | REPL execution without fimod sandbox limits. |
| `ReplContinuationMode` | `src/cmd/monty.rs` | REPL prompt and multi-line continuation state. |
| `detect_repl_continuation_mode` | `src/cmd/monty.rs` | REPL syntax-continuation detection. |
| `MontyTimeDelta` | `src/convert.rs` | Serialize `datetime.timedelta` outputs. |
| `MontyTimeZone` | `src/convert.rs` | Serialize `datetime.timezone` outputs. |

## Changes impacting fimod

| Fimod symbol | Changed? | Nature | Action required |
|---|---:|---|---|
| `OsFunction` | yes | Removed from public exports; replaced by `OsFunctionCall`. | Replace import and all `OsFunction::*` matches in `src/engine.rs`. |
| `RunProgress::OsCall` | yes | `OsCall` now has `function_call: OsFunctionCall`; previous `function`, `args`, `kwargs` fields are gone. | In `run_loop`, use `let function_call = call.take_function_call()` or borrow `call.function_call`, then resume. |
| `dispatch_os_call` | yes | Current signature takes `&OsFunction` plus raw args. v0.0.18 gives typed payloads. | Change to `dispatch_os_call(&OsFunctionCall, &SandboxPolicy)`. Match typed variants. |
| `lookup_env` | yes | `os.getenv` args now arrive as `OsFunctionCall::Getenv(GetenvArgs { key, default })`. | Use typed `key` and decide default handling. Recommended: denied env returns `None`; allowed-but-missing returns `default`. |
| filesystem OS calls | yes | New typed variants include `Open`, `AppendText`, `AppendBytes`; all path calls carry `MontyPath`. | Preserve current deny behavior for `Path.*`; choose explicit `PermissionError` for `Open` because `open()` expects `FileHandle`. |
| `MontyObject` | minor | Adds `FileHandle(MontyFileHandle)` and file-related values. | Current catch-all conversions compile, but returning a file handle to fimod output will error. Add a clearer error if needed. |
| `PrintWriter` / `PrintWriterCallback` | no | v0.0.18 keeps the callback shape used by fimod. | No action. |
| `MontyRun::new` / `start` | no | Signatures used by fimod are stable. | No action. |
| `ResourceLimits` / `LimitedTracker` | no | Builder APIs used by fimod remain. | No action. |
| `MontyRepl` / continuation APIs | no | APIs used by `fimod monty repl` remain. | No action expected. |
| `MontyDate*` / `MontyTime*` structs | no | Fields consumed by fimod remain compatible. | No action. |

## Breaking changes

1. `monty::OsFunction` is no longer exported.
   - Before: `src/engine.rs` imported and matched `OsFunction`.
   - After: import `OsFunctionCall` and match variants such as
     `DateToday`, `DateTimeNow(tz)`, `Getenv(args)`, `GetEnviron`, `Open(args)`.
   - Fimod files to adapt: `src/engine.rs` imports, `run_loop`, `dispatch_os_call`,
     `clock_denied_message`, `lookup_env`.

2. `RunProgress::OsCall(call)` no longer exposes `call.function` / `call.args`.
   - Before: `dispatch_os_call(&call.function, &call.args, ctx.policy)`.
   - After: use `call.function_call` or `call.take_function_call()`.
   - Fimod files to adapt: `src/engine.rs`.

3. `open()` is now a Monty builtin.
   - Before: fimod docs/tests expect `open("/etc/passwd")` to be `NameError`.
   - After: `open()` yields `OsFunctionCall::Open(...)`; fimod must deny or serve it.
   - Fimod files to adapt: `src/engine.rs`, `tests/cli/sandbox.rs`,
     `docs/reference/monty-engine.md`.

4. Monty v0.0.18 release notes say it bumps Monty's Rust baseline to Rust 1.95.
   - Fimod uses `rust = "stable"` in `mise.toml` and `channel = "stable"` in
     `rust-toolchain.toml`, so this should be acceptable if CI stable is current.
   - Still verify local/CI toolchain before release.

## New capabilities unlocked

- `open()` builtin with buffered file read/write methods, routed through OS calls.
- Context manager support, including `with open(...) as f:`.
- Better GC behavior for cycles and timezone objects.
- More resource hardening: recursion/depth bounds, JSON integer bounds, f-string precision limits, iterator-size preallocation bounds.
- Better panic resistance in parsing, integer operations, tuple traversal, `list.sort()` mutation during key callbacks, duplicate coroutine handling, and traceback source-line handling.
- REPL fix for local/global store operations inside comprehension expressions.
- External function values compare by name, which may make mold-side comparisons of fimod built-ins less surprising.

No new normal stdlib module appears user-facing for fimod beyond the `open()`
builtin and existing module behavior. `gc.collect()` is behind Monty's
`test-hooks` feature and should not be documented as available in fimod unless
that feature is enabled.

## Upgrade steps

1. Adapt `src/engine.rs` for `OsFunctionCall`.
2. Update sandbox tests for `open()`:
   - no longer expect `NameError`;
   - expect fimod's chosen sandbox denial behavior.
3. Update `docs/reference/monty-engine.md`:
   - replace legacy `OsAccess` wording with current `OsFunctionCall` behavior;
   - document `open()` as syntactically available but sandbox-controlled.
4. Bump `Cargo.toml` from `tag = "v0.0.17"` to `tag = "v0.0.18"`.
5. Run `cargo update -p monty`.
6. Run `rtk cargo build`, `rtk cargo test`, and `rtk cargo clippy --all-targets -- -D warnings`
   or the repo gates `rtk task lint` and `rtk task test`.

## Risk

Medium after adaptation: the compile-breaking `OsFunctionCall` migration is
small and covered by sandbox tests, but the Rust baseline is now 1.95 and
`open()` changes from `NameError` to a sandbox `PermissionError`.

## Recommendation

Keep the bump. v0.0.18 contains substantial crash hardening, resource-bound
fixes, and Python feature improvements relevant to mold safety. The remaining
release risk is ensuring CI/release builders use Rust 1.95 or newer.
