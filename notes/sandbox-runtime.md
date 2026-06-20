# Sandbox Runtime - Plan

- **Date**: 2026-06-20
- **Status**: active planning note for runtime sandbox behavior and deferred
  Monty host-capability work.
- **Companion**: `notes/sandbox-config.md` covers policy file creation and
  editing.

## Scope

This note is about what fimod does after a sandbox policy is resolved. It covers
the Monty execution boundary, current host capabilities, resource limits, and
future runtime extensions such as filesystem mounts or `monty run`.

## Runtime Invariants

- Rust owns all input/output bytes. Monty receives Python-shaped objects, not
  raw files, sockets, or host handles.
- Host-sensitive operations must pass through Monty's `RunProgress::OsCall`
  boundary, and fimod decides what to return.
- There is one policy resolver and one runtime flag: `--sandbox-file`.
- Resource limits apply even when no policy file exists.
- This is a local CLI guardrail, not a multi-tenant hostile-code sandbox.

## Current Runtime

`shape` resolves the policy before running the mold pipeline:

```rust
SandboxPolicy::resolve(shape.sandbox_file.as_deref())
```

The effective policy feeds two runtime layers:

1. Monty resource limits through `LimitedTracker`.
2. Host capability dispatch through `dispatch_os_call()`.

Hard-coded fallback limits:

| Limit | Default |
|---|---|
| `max_duration` | `10m` |
| `max_memory` | `2GB` |

These defaults are used by zero-authorization and by policy files that omit the
corresponding field. A policy can explicitly set `"unlimited"`.

## Capability Matrix

| Capability | Current `shape` behavior | Config field | Future direction |
|---|---|---|---|
| `date.today()` | allowed only when enabled | `allow_clock` | maybe split today/now later |
| `datetime.now()` | allowed only when enabled | `allow_clock` | maybe split today/now later |
| `os.getenv(KEY)` | returns value only when `KEY` matches | `allow_env` | keep glob syntax |
| `os.environ` | empty dict unless env access is allowed | `allow_env` | maybe filtered dict later |
| `Path.*` filesystem ops | denied, currently returns `None` for legacy path calls | none | `[[mount]]` |
| `open()` | syntactically available, denied with `PermissionError` | none | `[[mount]]` or explicit FS policy |
| network/process modules | unavailable or not exposed by Monty | none | non-goal |
| resource limits | enforced by `LimitedTracker` | `max_duration`, `max_memory` | maybe more limit fields later |

The durable expectation after Monty v0.0.18 is that `open()` is controlled by
the sandbox and denied with `PermissionError`, not `NameError`.

## Runtime-Only Escape Hatch

`--sandbox-file=""` is a runtime special case: it forces zero-authorization
even if `FIMOD_SANDBOX_FILE` or the canonical file exists.

This should remain runtime-only. It is not a valid target for config editing
commands such as `setup sandbox set`.

## Next Runtime Impact

The planned `setup sandbox set` command should not require runtime changes. It
only writes policy files that the existing resolver already understands.

Runtime tests still matter because the config command must write values that
load into the same `SandboxPolicy` semantics.

## Deferred Runtime: Filesystem Mounts

Filesystem access should stay denied until there is an explicit mount model.
The likely extension is additive:

```toml
[sandbox]
allow_clock  = true
max_duration = "10m"
max_memory   = "2GB"
allow_env    = []

[[mount]]
virtual = "/data"
host    = "./data"
mode    = "read-only"

[[mount]]
virtual = "/out"
host    = "./out"
mode    = "read-write"
write_bytes_limit = "10MB"
```

Constraints:

- Keep the flat `[sandbox]` table valid.
- Add `[[mount]]` rather than replacing the current schema with `[limits]`,
  `[clock]`, or `[env]`.
- Deny filesystem access when no mount matches.
- Prevent path traversal and symlink escape.
- Decide separately whether `open()` and `Path.*` share the exact same mount
  checks or need different error behavior.

## Deferred Runtime: `monty run`

`fimod monty run` remains a separate product slice. It should not block sandbox
configuration editing.

Candidate shape:

```bash
fimod monty run script.py
fimod monty run - < script.py
fimod monty run script.py --sandbox-file policy.toml
```

Open decisions:

- Whether V1 ships as pure Monty first, or waits for `--sandbox-file`.
- How script args are exposed: reuse `--arg k=v` as an `args` dict, or expose a
  different convention.
- How process exit should behave for `sys.exit()` or Monty exceptions.
- Whether fimod external functions are available, and if so which families.

Non-goal: `monty run` must not become a replacement for `fimod s -m script.py`
when the job is a data transform. The shape pipeline stays canonical for
data-in, mold-transform, data-out workflows.

## REPL Policy Parity

`fimod monty repl` uses the same policy resolver as `fimod s`:

```bash
fimod monty repl --sandbox-file policy.toml
fimod monty repl --sandbox-file=""
```

Implementation notes:

- `src/cmd/monty.rs` drives `MontyRepl::feed_start()` and handles
  `ReplProgress::OsCall` through the same sandbox helpers used by `engine.rs`.
- `--sandbox-file ""` is the same runtime-only zero-authorization escape hatch
  as `shape`.
- Resource-limit errors are printed in the REPL and the session continues;
  `shape` still exits with code `137`.

## `--no-sandbox`

The existing escape hatch is:

```bash
fimod s --sandbox-file="" ...
```

An alias such as `--no-sandbox` is more readable but adds CLI surface. Keep it
deferred until there is user demand.

## Open Questions

- Should clock access eventually split `date.today()` and `datetime.now()`?
- Should `os.environ` return only allowed env vars instead of always returning
  an empty dict when denied?
- Should runtime shorthand flags ever exist on `shape`, or should users edit a
  policy file through `setup sandbox set`?
- What is the minimum mount feature that is useful without overclaiming
  sandbox security?
