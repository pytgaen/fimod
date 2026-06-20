# Sandbox Config - Plan

- **Date**: 2026-06-20
- **Status**: active planning note for the sandbox configuration surface.
- **Companion**: `notes/sandbox-runtime.md` covers how the resolved policy is
  consumed by the Monty execution boundary.

## Scope

This note is about the full sandbox configuration surface: bootstrapping,
inspecting, editing, and applying named presets to sandbox policy files. It does
not define new Monty host capabilities. Runtime behavior, `OsCall` dispatch,
filesystem mounts, and `monty run` belong in `sandbox-runtime.md`.

The product need is not just first-run onboarding. Users need one CLI namespace
that can manage the default sandbox policy and an explicit file used later with
`fimod s --sandbox-file <PATH>` or `fimod monty repl --sandbox-file <PATH>`.

## Current Contract

The canonical sandbox file is:

```text
~/.config/fimod/sandbox.toml
```

Runtime resolution is owned by `SandboxPolicy::resolve()`:

1. `--sandbox-file <PATH>`
2. `FIMOD_SANDBOX_FILE`
3. `~/.config/fimod/sandbox.toml`
4. zero-authorization policy with hard-coded limits

The shipped schema is the flat `[sandbox]` table:

```toml
[sandbox]
allow_clock  = true
max_duration = "10m"
max_memory   = "2GB"
allow_env    = []
```

Field meanings:

| Field | Type | Missing field default |
|---|---|---|
| `allow_clock` | bool | `false` |
| `max_duration` | duration or `"unlimited"` | hard-coded `10m` |
| `max_memory` | size or `"unlimited"` | hard-coded `2GB` |
| `allow_env` | list of glob patterns | `[]` |

`fimod setup sandbox defaults` writes the recommended policy to the canonical
file. It refuses to overwrite unless `--force` is provided, and `--if-needed`
preserves an existing policy. Installers rely on
`fimod setup all defaults --if-needed`; shell installers should keep delegating
prompt/env-var behavior to the binary.

## Config CLI Surface

Keep the surface under the existing setup namespace:

```text
fimod setup sandbox defaults [OPTIONS]
fimod setup sandbox show [OPTIONS]
fimod setup sandbox get <KEY> [OPTIONS]
fimod setup sandbox set [OPTIONS]
```

All commands target the canonical file by default. Commands that read or edit a
specific policy accept `--sandbox-file <PATH>`.

`--sandbox-file ""` is rejected here. Empty string is a runtime-only escape
hatch for `fimod s`.

## `defaults`

Current command:

```bash
fimod setup sandbox defaults --yes
```

Extend it with named presets:

```bash
fimod setup sandbox defaults --preset recommended --yes
fimod setup sandbox defaults --preset strict --yes
fimod setup sandbox defaults --preset permissive --yes
fimod setup sandbox defaults --sandbox-file ./ci-sandbox.toml --preset strict --yes
```

Presets:

| Preset | Policy |
|---|---|
| `recommended` | `allow_clock = true`, `max_duration = "10m"`, `max_memory = "2GB"`, `allow_env = []` |
| `strict` | `allow_clock = false`, `max_duration = "30s"`, `max_memory = "512MB"`, `allow_env = []` |
| `permissive` | `allow_clock = true`, `max_duration = "30m"`, `max_memory = "4GB"`, `allow_env = ["LANG", "LC_*", "TZ", "USER", "HOME"]` |

`recommended` remains the default. `permissive` is still bounded: it exposes
common local environment keys and raises limits, but it does not disable
resource limits.

`--force`, `--if-needed`, prompts, and env-var answers keep their current
meaning. `--sandbox-file <PATH>` writes the preset to that explicit path instead
of the canonical file.

## `show` And `get`

Users need read-only inspection before changing policy files:

```bash
fimod setup sandbox show
fimod setup sandbox show --sandbox-file ./ci-sandbox.toml
fimod setup sandbox get max-memory
fimod setup sandbox get allow-env --sandbox-file ./ci-sandbox.toml
```

`show` prints the normalized effective file content for the target file. If the
target file is missing, it prints the policy that would be created from the
recommended preset.

`get` prints a single value and exits non-zero for unknown keys.

Supported keys:

| Key | Field |
|---|---|
| `allow-clock` | `allow_clock` |
| `max-duration` | `max_duration` |
| `max-memory` | `max_memory` |
| `allow-env` | `allow_env` |

Output should be script-friendly: one scalar per line for scalar fields, one
pattern per line for `allow-env`.

## `set`

Default target:

```bash
fimod setup sandbox set --max-duration 20m --max-memory 4GB
# edits ~/.config/fimod/sandbox.toml
```

Explicit target:

```bash
fimod setup sandbox set --sandbox-file ./ci-sandbox.toml \
  --deny-clock \
  --max-duration 1m \
  --max-memory 512MB \
  --clear-env
```

Proposed options:

| Option | Behavior |
|---|---|
| `--sandbox-file <PATH>` | File to create/update. Missing option means canonical file. Empty string is rejected. |
| `--allow-clock` | Set `allow_clock = true`. Conflicts with `--deny-clock`. |
| `--deny-clock` | Set `allow_clock = false`. Conflicts with `--allow-clock`. |
| `--max-duration <VALUE>` | Validate with `parse_duration()`, then write the original value. |
| `--max-memory <VALUE>` | Validate with `parse_size()`, then write the original value. |
| `--allow-env <PATTERN>` | Repeatable. Replaces `allow_env` with the provided patterns. |
| `--clear-env` | Set `allow_env = []`. Conflicts with `--allow-env`. |

Behavior:

1. If the target file exists, parse it as the current `[sandbox]` schema.
2. If the target file does not exist, start from the recommended policy.
3. Apply only provided fields.
4. Error if no configuration option was provided.
5. Render a normalized `sandbox.toml` with all four fields.

## Normalized Rendering

All write commands render a normalized policy:

```toml
[sandbox]
allow_clock  = true
max_duration = "10m"
max_memory   = "2GB"
allow_env    = []
```

The implementation should not promise comment-preserving edits. A normalized
rewrite is simpler, testable, and consistent with `setup` owning bootstrap and
policy-editing files.

## `setup all`

`setup all defaults` should remain installer-friendly:

```bash
fimod setup all defaults --if-needed
fimod setup all defaults --preset strict --if-needed
```

If `defaults` gains presets, `setup all defaults --preset <NAME>` should forward
the preset to the sandbox block. It should not gain `set` fields such as
`--max-memory`; those belong to `setup sandbox set`.

## Out Of Scope For This Namespace

A top-level generic config subsystem remains separate:

```text
fimod config ...
```

Do not introduce it just to configure sandbox policy. The sandbox-specific
surface above is enough for this product slice.

## Acceptance Tests

Covered CLI tests:

- `setup sandbox set --max-memory 4GB` creates the canonical file when missing.
- `setup sandbox set --sandbox-file path --max-duration 1m` creates that file.
- Existing policy fields are preserved when unrelated fields are changed.
- `setup sandbox defaults --preset strict` writes the strict preset.
- `setup sandbox defaults --sandbox-file path --preset permissive` writes that file.
- `setup all defaults --preset strict --if-needed` forwards the preset to sandbox setup.
- `setup sandbox show` prints normalized TOML.
- `setup sandbox get max-memory` prints only the configured memory value.
- `setup sandbox get allow-env` prints one pattern per line.
- `--allow-clock` and `--deny-clock` conflict.
- `--allow-env` and `--clear-env` conflict.
- Invalid duration and size values fail with the parser error context.
- `--sandbox-file ""` fails with a clear message.
- `setup sandbox defaults` behavior remains unchanged.

Verification path:

```bash
rtk mise exec -- cargo test --test cli setup
rtk mise exec -- cargo test --lib sandbox
rtk mise exec -- task lint
rtk mise exec -- task test
```
