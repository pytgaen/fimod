# Changelog draft — 0.7.1

Transient draft for the 0.7.1 cycle. Will be promoted to `CHANGELOG.md`
in the `chore(release): 0.7.1` commit (per `notes/release-workflow.md`).

Base: `v0.7.0` → `HEAD` (branch `test/http-e2e-scaffold`),
21 committed commits + 6 uncommitted files from the watch hardening
session.

## Bug fixes

- **watch**: bail at startup when input file is missing — silent hang
  fixed (`7f4ab9f`).
- **watch**: coalesce notify event bursts via second-level debounce
  (default 500 ms, configurable) — fix multi-reruns parasites under
  inotify cross-process latency (`9efdfd3`).
- **watch**: surface input removal mid-watch with
  `[watch] warn: input removed, waiting for it to reappear...` instead
  of swallowing it silently. Atomic saves (rename via tmp + persist)
  stay silent — only sustained absence triggers the warning.

## New

- **watch**: env var `FIMOD_WATCH_QUIET_MS=<ms>` to override the
  second-level debounce window (default 500 ms). Useful when running
  on faster filesystems where 500 ms is excessive UX latency.

## Tests / hardening (no user-facing change)

The 0.7.1 cycle is primarily an **e2e hardening pass**. All hot-path
contracts now have pinning tests; several revealed real bugs (logged
above under "Bug fixes").

### HTTP (`src/http.rs`, scaffolded via `httpmock`)

10 commits, `3b9dcf1` → `96b9202`. Coverage:

- timeout (connection vs read)
- 4xx / 5xx surfacing with status code, not generic "request failed"
- redirect follow chain vs `--no-follow`
- `Content-Type` auto-detection (json, yaml, toml, csv, text/plain,
  missing CT, garbage CT, charset suffix)
- `Authorization` headers (Bearer / Basic) propagation across redirects
- `--http-header` parse rejection before any request is issued
- raw byte passthrough on `--output-format raw` — no UTF-8 re-encoding
- `HTTPS_PROXY` env routing

### Data formats (`src/format.rs`, `src/convert.rs`)

4 commits. Coverage:

- CSV ↔ NDJSON cross-format with mixed shapes, BOM, quoted comma
- `--csv-delimiter` vs `--csv-output-delimiter` divergence
- `txt` non-string fallback to JSON, chained txt → txt-in pipe
- `--in-place` format auto-detection from input path
- malformed input → error path (no panic) per parser

### Mold execution (`src/mold.rs`, `src/pipeline.rs`)

4 commits. Coverage:

- `transform(data, args, env, headers, **_)` keyword-arg contract
- `--arg name=value` pinned: all values pass as strings (no coercion)
- `--env PATTERN` glob filter — no match yields empty dict
- `set_output_format(...)` from inside a mold (incl. `"raw"` rules)

### Watch (`src/watch.rs`)

9 tests covering the full failure-mode surface. 6 added in this
session (uncommitted), 3 prior (`535663d` and earlier).

Tests:

- atomic save (vim/VSCode/IntelliJ rename pattern)
- debounce: 5 rapid writes → 1 rerun
- missing input at startup → clean error, no hang
- file deletion + recreation → recovers with warning
- mold file change → triggers rerun
- transient mold panic → watcher survives, reruns after fix
- malformed input mid-watch → watcher survives, reruns after fix
- SIGINT / SIGTERM (Unix) → exit by signal under 3 s, no zombie
- `FIMOD_WATCH_QUIET_MS` env var override is honored

## Docs

- `docs/guides/cli-reference.md` — watch section: document the
  two-level debounce, the `FIMOD_WATCH_QUIET_MS` env var, and the
  `[watch] warn: input removed...` UX.

## Tooling *(uncommitted)*

- task `outdated` + `cargo-outdated` via mise (root-deps freshness
  check, fails CI if any are stale).

## Release notes / decisions to make

- **Semver bump**: confirmed **0.7.1** (patch). No user-facing `feat`
  removed, the warning + env var are additive observability/tuning
  improvements that fit under "fix" semantics.
- **Uncommitted files** to commit before release:
  - `src/watch.rs` (warning logic + env var)
  - `tests/cli/watch.rs` (6 new tests)
  - `docs/guides/cli-reference.md` (env var doc)
  - `notes/todo-0.7.1.md` (closed snapshot of the cycle — replaces
    the previous `todo-0.8.0.md`)
  - `notes/todo-0.7.2.md` (next cycle: registry/main split refactos)
  - `notes/changelog-0.7.1.md` (this file)
  - `Taskfile.yml`, `mise.toml` (cargo-outdated tooling) — possibly
    a separate `chore(deps)` commit, kept out of the watch series.
