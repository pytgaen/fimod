# Fimod — Design Notes

Design notes: choices made, their reasons, and known constraints.

## Design Decisions

### Single Pipeline

The entire flow goes through: **Read → Parse → Convert → Execute mold → Convert back → Serialize → Write**.

The intermediate representation between formats is always `serde_json::Value`. Monty operates on `MontyObject` (Python dicts).

### Security: parsing in Rust, logic in Python

All parsing/serialization (serde) remains entirely in Rust. Monty only manipulates Python dicts. This is a deliberate security boundary: user scripts never have access to the filesystem or the network.

### Global `args` dict (not individual variables)

`--arg name=value` injects a global `args` dict, not individual Python variables:
- Explicit: `args["threshold"]` rather than a magic `threshold`
- Does not change the `transform(data)` signature
- Works with `-e` and `-m` without adaptation

### External functions via iterative loop

External functions (regex, etc.) use Monty's iterative loop `start()` / `state.run()`, not the simple `runner.run()`. When Python calls an external function, Monty pauses execution, Rust dispatches the call, then resumes.

### `fancy-regex` rather than `regex`

Monty does not support `import re`. Rust's `regex` crate is not PCRE2-compatible (no backreferences, lookahead, lookbehind). `fancy-regex` offers the best compromise: pure Rust, no C dependency, covers the most used PCRE2 features.

### 2-letter prefixes for external functions

Convention `re_`, `dp_`, `it_`, `hs_` to avoid collisions with Python builtins and make the origin explicit.

### CSV: `preserve_order` enabled

`serde_json/preserve_order` in `Cargo.toml` so that CSV column order is preserved through the pipeline.

### Debug on stderr

`--debug` prints to stderr with a `[debug]` prefix. In debug mode, Python's `print()` also goes to stderr via `StderrPrint` (implements `PrintWriter`). This never corrupts stdout.

### Optional subcommand

CLI uses `Option<Commands>`: `None` = shape mode (default pipeline), `Some(Mold{..})` = mold registry management.

## CI / Build

### Gitea Actions — GHES constraints

`@v4` actions are not supported on Gitea (GHES):
- `actions/upload-artifact@v3` ✅ — `@v4` → `GHESNotSupportedError`
- `actions/download-artifact@v3` ✅
- `actions/cache@v3` ✅

Marketplace actions that run in a Docker container (e.g., `orhun/git-cliff-action`) fail because the container lacks network access for `apt-get`. Always download binaries manually via `curl` in a normal step.

To extract a binary from a tarball, prefer `find + xargs cp` over `tar --strip-components` because the internal structure of archives varies across releases:
```bash
curl -sSfL "URL" | tar xz -C /tmp/
find /tmp -name 'binary' -type f | head -1 | xargs -I{} cp {} /usr/local/bin/binary
```

pip cache (`~/.cache/pip`): always run `pip install` even on a cache hit — the pip cache speeds up downloading but does not replace installation.

### Taskfile — zig via mise

`cargo-zigbuild` looks for `zig` in the PATH, but `task` does not propagate PATH modifications via `env:`. Solution: use the environment variable `CARGO_ZIGBUILD_ZIG_PATH` which points directly to the binary:
```yaml
env:
  CARGO_ZIGBUILD_ZIG_PATH:
    sh: mise which zig 2>/dev/null || which zig
```

### Local tooling (mise.toml)

All build tools are managed by mise: `rust`, `zig`, `upx`, `uv`. `rust-toolchain.toml` pins the cross-compilation targets (read by rustup and mise). Windows packaging uses `uv run python3 -c "import zipfile; ..."` to avoid any system dependency.

## Watchpoints

- **Unstable Monty API**: Monty is a git dependency (not a stable crate). The `MontyRun::new` API and error types can change between commits.
- **`num-bigint`** in `convert.rs`: `i64::try_from(BigInt)` conversion is used for large integers.
- **Python syntaxes not supported by Monty**: f-strings, `del d["key"]`, `**dict` unpacking, chained assignment `d[k1][k2] = v`. To be checked periodically with [Monty](https://github.com/pydantic/monty) releases.
