# Monty v0.0.19 — Impact on fimod

Date: 2026-07-28  
Previous: v0.0.18 → v0.0.19

Upstream:

- Release: <https://github.com/pydantic/monty/releases/tag/v0.0.19>
- Compare: <https://github.com/pydantic/monty/compare/v0.0.18...v0.0.19>

## Summary

Monty v0.0.19 is a feature-heavy but breaking host-API release. The migration
is tractable and an isolated fimod compatibility probe passes the full test
suite, but a plain tag bump does not compile: most boundary types moved to the
new `monty-types` crate, constructors and OS-call resumption changed, fresh
dependency resolution currently selects an incompatible `get-size2` release,
and resource-duration semantics need an explicit fimod decision.

## API surface consumed by fimod

This inventory was regenerated from the current `src/` tree. It contains 21
distinct public Monty symbols; associated suspension-state methods are included
under their owning progress types.

| Monty type/function | Fimod consumers | Role in fimod |
|---|---|---|
| `MontyObject` | `convert.rs`, `engine.rs`, `format.rs`, `pipeline.rs`, `cmd/monty.rs`, all external-function modules, `monty_args.rs` | Python-side IR for pipeline data, arguments, return values, dataclasses, external functions, and REPL output |
| `DictPairs` | `convert.rs`, `engine.rs`, `format.rs`, `regex.rs`, `env_helpers.rs`, tests in `template.rs` and `iter_helpers.rs` | Ordered Python dict/dataclass attribute construction and iteration |
| `MontyDate` | `convert.rs`, `engine.rs` | JSON conversion and sandboxed `date.today()` |
| `MontyDateTime` | `convert.rs`, `engine.rs` | JSON conversion and sandboxed `datetime.now()` |
| `MontyTimeDelta` | `convert.rs` | Serialization of Monty timedeltas |
| `MontyTimeZone` | `convert.rs` | Serialization of Monty time zones |
| `ExcType` | `engine.rs`, `cmd/monty.rs` | Permission, timeout, and memory exception classification |
| `ExtFunctionResult` | `engine.rs` | Return/error channel for host OS-call handling |
| `LimitedTracker` | `engine.rs`, `cmd/monty.rs` | Runtime memory/duration limits for molds and the REPL |
| `MontyException` | `engine.rs`, `cmd/monty.rs` | Compile/runtime/callback error transport and translation |
| `MontyRun` | `engine.rs` | Mold compilation and iterative execution |
| `NameLookupResult` | `engine.rs`, `cmd/monty.rs` | Dynamic resolution of fimod external functions |
| `OsFunctionCall` | `engine.rs` | Typed clock, environment, filesystem, and `open()` suspensions |
| `PrintWriter` | `engine.rs`, `cmd/monty.rs` | stdout or debug-stderr routing for Python `print()` |
| `PrintWriterCallback` | `engine.rs` | `StderrPrint` callback contract |
| `ResourceLimits` | `engine.rs` | Construction of memory and duration limits |
| `RunProgress` | `engine.rs` | Main execution loop over complete/function/OS/name/future states |
| `MontyRepl` | `cmd/monty.rs` | Persistent interactive Monty session |
| `ReplProgress` | `cmd/monty.rs` | REPL suspension/resumption loop |
| `detect_repl_continuation_mode` | `cmd/monty.rs` | Complete versus multiline REPL input detection |
| `ReplContinuationMode` | `cmd/monty.rs` | REPL prompt and pending-snippet state |

Exhaustive matches to re-check on every Monty bump:

- `RunProgress`: `src/engine.rs:938`
- `OsFunctionCall`: `src/engine.rs:1053`
- `ReplProgress`: `src/cmd/monty.rs:115`
- `ReplContinuationMode`: `src/cmd/monty.rs:66`

## Changes impacting fimod

18 of the 21 consumed symbols change directly. Three keep the same public
shape: `RunProgress`, `detect_repl_continuation_mode`, and
`ReplContinuationMode`.

| Consumed symbol | Changed? | Nature | Required action |
|---|---:|---|---|
| `MontyObject` | yes | Moved to `monty-types`; consumed variants keep their shape | Import from `monty_types`; class instances returned to the host arrive as `Repr` and cannot be serialized directly by fimod |
| `DictPairs` | yes | Moved to `monty-types`; used constructors/iterators remain | Split imports and replace qualified `monty::DictPairs` paths |
| `MontyDate` | yes | Moved to `monty-types`; fields unchanged | Import-only migration |
| `MontyDateTime` | yes | Moved to `monty-types`; fields unchanged | Import-only migration |
| `MontyTimeDelta` | yes | Moved to `monty-types`; fields unchanged | Import-only migration |
| `MontyTimeZone` | yes | Moved to `monty-types`; fields unchanged | Import-only migration |
| `ExcType` | yes | Moved; `UnicodeEncodeError` added | Import from `monty_types`; existing wildcard arms remain safe |
| `ExtFunctionResult` | yes | Moved; used variants unchanged | Import from `monty_types` and update the `OsCallOutcome` conversion |
| `LimitedTracker` | yes | Moved; duration now accumulates VM time and pauses during host suspensions | Import move plus explicit wall-clock policy review |
| `MontyException` | yes | Moved; public constructor/classifier used by fimod remain | Import-only migration |
| `MontyRun` | yes | `new` gains `CompileOptions` | Pass an explicit option value in `src/engine.rs` |
| `NameLookupResult` | yes | Moved; `Value`/`Undefined` unchanged | Import-only migration |
| `OsFunctionCall` | yes | Moved; `Used` and `take_function_call()` removed | Use `OsCall::resume_with`; delete the `Used` match arm |
| `PrintWriter` | yes | Moved; variants used by fimod unchanged | Import from `monty_types` |
| `PrintWriterCallback` | yes | Moved; callback signatures unchanged | Import-only migration |
| `ResourceLimits` | yes | Moved; `max_allocations` removed, but fimod does not use it | Import-only migration for current calls |
| `RunProgress` | no | Same five variants; its `OsCall` payload exposes the new resumption API | Keep exhaustive match, migrate only the OS-call arm |
| `MontyRepl` | yes | `new` gains `CompileOptions` | Pass an explicit option value in `src/cmd/monty.rs` |
| `ReplProgress` | yes | Variants unchanged; `ReplOsCall::take_function_call()` removed | Use `ReplOsCall::resume_with` |
| `detect_repl_continuation_mode` | no | Signature unchanged | No action |
| `ReplContinuationMode` | no | Same three variants | No action |

## Breaking changes

### Boundary types moved to `monty-types`

Before:

```rust
use monty::{MontyObject, DictPairs, ResourceLimits};
```

After:

```rust
use monty::{MontyRun, RunProgress};
use monty_types::{CompileOptions, DictPairs, MontyObject, ResourceLimits};
```

Fimod must add a direct `monty-types` dependency from the exact same git tag as
`monty`; otherwise identical-looking boundary types can come from different
crate instances and will not unify. This affects `Cargo.toml` and imports in
`src/convert.rs`, `src/dotpath.rs`, `src/engine.rs`, `src/env_helpers.rs`,
`src/exit_control.rs`, `src/format.rs`, `src/format_control.rs`,
`src/gatekeeper.rs`, `src/hash.rs`, `src/iter_helpers.rs`,
`src/monty_args.rs`, `src/msg.rs`, `src/pipeline.rs`, `src/regex.rs`,
`src/template.rs`, and `src/cmd/monty.rs`.

### Constructors require compile options

- `MontyRun::new(code, name, inputs)` becomes
  `MontyRun::new(code, name, inputs, options)` at `src/engine.rs:862`.
- `MontyRepl::new(name, tracker)` becomes
  `MontyRepl::new(name, tracker, options)` at `src/cmd/monty.rs:25`.

`CompileOptions::default()` enables pytest-style introspected assertion
messages. For example, `assert data == 5` now surfaces
`AssertionError: assert None == 5`. Fimod should choose explicitly between
adopting/documenting this improvement and using
`AssertMessageAnnotations::Off` to preserve previous error text.

### OS-call extraction was replaced by handler-based resumption

`OsFunctionCall::Used`, `OsCall::take_function_call()`, and
`ReplOsCall::take_function_call()` are gone. Migrate:

- `src/engine.rs:965-986` to `OsCall::resume_with(print, handler)`;
- `src/cmd/monty.rs:117-123` to `ReplOsCall::resume_with(print, handler)`;
- remove `OsFunctionCall::Used` at `src/engine.rs:1092`.

`RunProgress`, `ReplProgress`, and `ReplContinuationMode` do not gain variants,
so their exhaustive matches otherwise remain valid.

### Fresh dependency resolution is currently broken

A plain v0.0.19 tag bump resolved `get-size2 0.10.3`. That release depends on
`compact_str 0.10`, while `ruff_python_ast 0.0.3` stores
`compact_str 0.9.x`; its `GetSize` derive then fails with `E0277`.

Monty's own v0.0.19 lock uses `get-size2 0.10.1`, which still matches
`compact_str 0.9.x`. The isolated fimod compatibility probe succeeded after:

```bash
cargo update -p get-size2@0.10.3 --precise 0.10.1
```

This is a transitive upstream packaging/lock-drift issue. A checked-in fimod
lock can carry the pin, but the upgrade should not rely on an unconstrained
fresh resolve until upstream constrains or fixes it.

### Duration limits no longer cover host suspension time

In v0.0.19, `LimitedTracker` accumulates active VM time and pauses while Monty
is suspended in a fimod external/OS callback. Fimod currently computes the
remaining whole-chain budget before starting each mold, but does not re-check
wall-clock elapsed time after each host dispatch inside a mold. Without an
adapter check at resume boundaries, a slow Rust-side helper can exceed the
documented whole-chain duration budget.

## New capabilities unlocked

- Simple user-defined classes: class variables, instance methods, `__init__`,
  `__repr__`/`__str__`, `type()`/`isinstance()`, and class decorators.
  Inheritance, metaclasses, function/method decorators, and most dunder
  protocols remain unsupported. Classes are useful inside a mold, but molds
  must return serializable data such as a dict; returning an instance directly
  currently fails fimod serialization with `MontyObject::Repr`.
- Partial `unicodedata`: `category`, `name`, `lookup`, `combining`,
  `normalize`, `is_normalized`, and `unidata_version`.
- Text codecs on `str.encode()` / `bytes.decode()`: UTF-8, ASCII, and
  UTF-16/UTF-32 endian variants with a restricted error-handler set.
- Multi-level closure capture, richer f-strings, `iter(callable, sentinel)`,
  and arbitrary iterable unpacking after `*`.
- Module dunders such as `__name__`.
- Optional pytest-style assertion diagnostics.

An isolated fimod probe returned:

```json
{"class_method":42,"unicode_category":"Ll","normalized":"é","codec_roundtrip":"café"}
```

## Compatibility evidence

The real worktree was not modified during the compatibility probe. In a
temporary copy:

1. bumped `monty` to v0.0.19 and added `monty-types` at the same tag;
2. pinned `get-size2` to `0.10.1`;
3. split boundary/runtime imports;
4. supplied `CompileOptions::default()`;
5. migrated both OS-call loops to `resume_with`;
6. removed the obsolete `Used` arm.

Results:

- `cargo check`: pass;
- `cargo test`: 660 passed across lib, bin, CLI, and mold fixtures;
- performance tests: 9 intentionally ignored by the standard suite;
- doc test: 1 intentionally ignored;
- class/unicode/codec probe: pass;
- direct class-instance return: confirmed serialization error, so the docs
  need the explicit "return a dict" caveat.

## Documentation impact after the dependency bump

Do not update the public engine page before the actual bump: it currently
describes the shipped v0.0.18 behavior. In the upgrade PR, update
`docs/reference/monty-engine.md` to:

- name v0.0.19 in the introduction and REPL example;
- move classes from "Not Yet Supported" to "Supported", with the limitations
  and host-boundary caveat above;
- add `unicodedata` to the standard-library table;
- document supported codecs and assertion-message policy;
- replace the obsolete `take_function_call()` code sample with
  `resume_with(...)`;
- clarify whether fimod restores whole-chain wall-clock enforcement around host
  callbacks.

No current upstream number justifies changing fimod's published performance
table.

## Upgrade steps

1. Decide the assertion-message policy and how to preserve the documented
   whole-chain wall-clock duration limit.
2. Add `monty-types` and move both Monty dependencies to tag `v0.0.19`.
3. Refresh the lock, then pin `get-size2` to `0.10.1` unless upstream has
   published a compatible resolution.
4. Split the imports and migrate the two constructors and OS-call loops listed
   above.
5. Add targeted regression tests for classes, direct class-instance output,
   `unicodedata`, codecs, assertion text, and time spent in host callbacks.
6. Update `docs/reference/monty-engine.md`.
7. Run `rtk task lint`, `rtk task test`, `rtk task test:performance`,
   `rtk task test:performance:fast`, and `rtk task doc:build`.

## Risk

**High before adaptation; medium after it.**

The source migration itself is small and the isolated full suite is green.
Risk remains elevated because the plain dependency resolve is currently broken,
the host API changes across the entire execution boundary, and the duration
limit semantics can weaken fimod's sandbox contract if carried over unchanged.

## Recommendation

**Wait — do not perform a tag-only upgrade.**

Open a dedicated upgrade change once fimod is ready to carry either an explicit
`get-size2 0.10.1` lock pin or an upstream packaging fix, and implement the
wall-clock/compile-option decisions in the same PR. If those two conditions are
accepted, v0.0.19 is otherwise ready to integrate: the minimal source migration
and the full current test suite have already been validated in isolation.
