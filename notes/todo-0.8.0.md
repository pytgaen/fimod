# TODO 0.8.0

Transient backlog for the 0.8.0 cycle. Items move to user docs / changelog
once shipped.

## E2E test coverage gaps

Data and mold paths have unit tests but no real end-to-end coverage on the
hot edges. Several modules are exercised only indirectly through string
assertions on stdout/stderr.

### HTTP (`src/http.rs`)

The whole `--input-format http` path is uncovered. Tests grep for the
string "http" but never issue a real request — they only assert error
messages or use URLs as plain data.

**Goal is not coverage — it is forcing the design through real edge
cases.** A fake HTTP server in tests reveals dormant bugs that unit tests
on individual functions cannot: header propagation, redirect chains,
malformed responses. Use wiremock (matches `reqwest::blocking`).

Edge cases to exercise:

- timeout (`--timeout`) — connection vs read timeout behavior
- non-2xx status (401, 404, 500) → error surfaced cleanly with the
  status code, not a generic "request failed"
- redirects: default follow chain (3xx → 2xx) vs `--no-follow`
  (3xx returned as-is)
- `Content-Type` auto-detection (`content_type_to_format`):
  application/json, yaml, toml, csv, text/plain, missing CT, garbage CT,
  CT with charset suffix (`application/json; charset=utf-8`)
- authentication via `--header Authorization: Bearer ...` and
  `--header Authorization: Basic ...` — header reaches the server
  unchanged across redirects (or is stripped on cross-origin redirect,
  whichever the design picks — the test pins the contract)
- malformed `--header` input rejected by `parse_header` before any
  request is issued
- `fetch_url_bytes` raw byte path (binary passthrough via
  `set_output_format("raw")`) — bytes arrive unmodified, no UTF-8
  re-encoding
- proxy / `HTTPS_PROXY` env handling if relied upon

### Data formats (`src/format.rs`, `src/convert.rs`)

Round-trip coverage exists per format, but cross-format invariants are
thin:

- CSV ↔ JSON ↔ NDJSON with mixed types, quoted delimiters, BOM
- `--csv-delimiter` vs `--csv-output-delimiter` divergence
- TXT format: `Value::String` bare emission vs non-string fallback to JSON
- `DataFormat::Txt` chained with `-i` (txt-out → txt-in pipe)
- `--in-place` with format auto-detection from path
- malformed input → error path (not panic) for each parser

### Mold execution (`src/mold.rs`, `src/pipeline.rs`)

- `transform(data, args, env, headers, **_)` keyword-arg contract:
  mold declaring only `data` still works; extra unknown kwargs ignored
- `--arg name=value` typing: int, float, bool, string, JSON
- `--env PATTERN` glob filtering: `*`, `PREFIX_*`, `EXACT`,
  comma-separated, no match
- `set_output_format(...)` from inside a mold (incl. `"raw"` rules)
- `headers` parameter: list for CSV, `None` otherwise
- inline `-e` auto-wrap into `transform(...)`
- mold from registry (`@name`) vs local path vs stdin
- chained molds (`@a | @b`) — data shape preserved across boundary

## Split `src/registry.rs`

2180 lines, 69 top-level symbols. Already hard to navigate; the planned
work (Bitbucket backend per `notes/ARCHITECTURE.md:163`, FS mounts,
extra resolution paths) will push it past 3000 and into "code nobody
wants to touch" territory. Architectural debt in large files compounds
non-linearly — split now, before the next features land on top.

Proposed layout (`src/registry/` module, `mod.rs` re-exports the
current public API so external callers stay unchanged):

| File         | Lines (current)             | Responsibility                                      |
| ------------ | --------------------------- | --------------------------------------------------- |
| `config.rs`  | 14–435                      | `Source`, `SourcesConfig`, `sources.toml` CRUD,     |
|              |                             | `add` / `remove` / `list` / `show` / `set_priority` |
| `resolve.rs` | 440–800                     | `@name` / `@src/name` resolution, env registries,   |
|              |                             | `resolve_local`, `resolve_remote`, auth headers     |
| `catalog.rs` | 800–1046, 1936–2008,        | `Catalog`, `fetch_catalog`, `build_catalog`,        |
|              | 2011–2180, + cache 952–1145 | `scan_local_molds`, `github_to_raw`,                |
|              |                             | `compute_mold_hash`, cache helpers                  |
| `molds.rs`   | 1147–1794                   | `list_molds`, `show_mold`, `MoldMatch`, completions |

The `setup` wizard (lines 1803–1927) leaves the registry module entirely
and merges into the existing `src/setup.rs` (currently a 86-line CLI
wrapper that calls `registry::setup`). Result: `src/setup.rs` becomes
autonomous (~210 lines), the trampoline indirection
`registry::setup() ← setup::registry_defaults()` disappears.

Order of operations:

1. Add `pub mod registry;` with the new submodules, keeping all `pub fn`
   signatures intact. No call-site changes outside `registry/`.
2. Move blocks one at a time, running `rtk task lint && rtk task test`
   between each move. Pure-move commits — no rename, no signature
   change, no logic edit.
3. After all moves are green, do a separate cleanup pass: visibility
   tightening (`pub` → `pub(crate)` where possible), helper consolidation.

Rule: do this **before** the Bitbucket / FS-mount work, so the new
backends land in a clean `resolve.rs` instead of compounding the mess.

## Split `src/main.rs`

1261 lines, 24 top-level symbols. `run_shape` (L714–919) +
`run_shape_pipeline` (L921–1261) account for ~547 lines of Shape
dispatch logic glued to the `main` entry point. Adding it to
`pipeline.rs` was a bad idea — that file is already 1136 lines and
its responsibility is data-transformation orchestration, not
CLI-args-to-config translation.

Plan: introduce `src/cli.rs` for clap definitions and `src/cmd/` for
subcommand handlers. `main.rs` shrinks to ~80 lines (entry point +
top-level dispatch).

| File                  | Lines moved from main.rs | Responsibility                                          |
| --------------------- | ------------------------ | ------------------------------------------------------- |
| `cli.rs`              | 33–391, 439–483          | `Cli`, `ShapeArgs`, `Commands`, all subcommand enums,   |
|                       |                          | completion candidate helpers (`format_candidates`,      |
|                       |                          | `complete_molds`, `complete_sources`), `MsgLevel`       |
| `cmd/shape.rs`        | 394–437, 714–1261        | `run_shape`, `run_shape_pipeline`, `build_script_refs`  |
| `cmd/registry.rs`     | (façade, new)            | thin dispatcher for `RegistryAction` variants → calls   |
|                       |                          | into `registry::*` (post-split per previous section)    |
| `cmd/mold.rs`         | (façade, new)            | thin dispatcher for `MoldAction` → calls `mold::*`      |
| `cmd/setup.rs`        | (move existing)          | absorbs current `src/setup.rs` (which itself absorbs    |
|                       |                          | `registry::setup` per the registry-split plan)          |
| `cmd/monty.rs`        | 629–712                  | `run_monty_repl`, `repl_feed`                           |
| `cmd/completions.rs`  | 485–523                  | `print_completion_script`, `detect_shell`               |
| `main.rs`             | (residual)               | `main`, `dispatch`, `dispatch_other` (slimmed)          |

Order of operations:

1. Create `src/cli.rs` first, move clap definitions over. No behavior
   change; main.rs imports them. Lint + test.
2. Create `src/cmd/mod.rs` with empty submodules. Move handlers one
   subcommand at a time, lint + test between each move.
3. Slim `dispatch` / `dispatch_other` last, once every handler has
   moved out.

Cross-reference: this split combines with the registry split — the
final layout has `src/cmd/setup.rs` owning the wizard (originally in
`registry.rs::setup`, then `src/setup.rs`, then `src/cmd/setup.rs`).
Sequence the registry split first (smaller blast radius, no callers
to update outside the module), then this one.

## Watch mode tests (`src/watch.rs`)

Current state: **one smoke test + arg-rejection tests, nothing on the
real failure modes.** Watch is a debounced filesystem-event loop with
sub-process semantics — exactly the kind of code where bugs hide
behind timing and editor quirks. Smoke coverage is not enough.

What exists:

- `tests/cli/watch.rs::test_watch_reruns_pipeline_on_input_change` —
  happy path: write `{"x":1}` → run #1, write `{"x":2}` → run #2.
- `tests/cli/args.rs:1017+` — 6 arg-rejection tests
  (`--watch` + `--in-place`, `--no-input`, `-I`, multi-`-i`, http,
  raw output). They check the validator, not the watcher.

Cases to add (each one targets a known-real failure pattern, not
hypothetical coverage):

- **Debounce**: 5 rapid writes within `DEBOUNCE_MS` → exactly 1 re-run
  (not 5). Pin the contract; debounce is the thing most likely to
  regress silently.
- **Atomic save**: editor pattern `write tmp + rename → in.json`
  (vim, VSCode, IntelliJ all do this). The current test uses
  `write_str` which is the *one* pattern editors don't use. If notify
  delivers `Rename` instead of `Modify`, do we still re-run?
- **File deletion + recreation**: `rm in.json && cp new in.json` →
  watcher recovers (or fails with a clear message — pin whichever the
  design picks).
- **Mold file changes**: `collect_watch_files` claims to include the
  mold; verify it. Edit `mold.py` while watching → re-run fires.
- **Transient mold error**: mold panics on run #2 → watcher does NOT
  exit, re-runs cleanly on run #3 with fixed input.
- **Malformed input**: input becomes invalid JSON mid-watch → error
  to stderr, watcher stays alive.
- **SIGINT / SIGTERM**: graceful shutdown, no zombie child, exit code
  is sane (0 or 130, pick one and pin it).
- **Missing input at startup**: `--watch -i nonexistent.json` →
  clear error, not a silent hang.

Implementation notes:

- Build on the existing `ChildGuard` + `poll_until` helpers — they're
  the right shape, just under-used.
- For the debounce test, count re-runs by writing a counter mold
  (`data["n"] = data.get("n", 0) + 1`) and reading the final value,
  not by counting log lines. Log-counting is timing-dependent.
- For atomic save: use `tempfile::NamedTempFile` + `persist()` to get
  the real rename semantics, not a homemade write+rename.
- Run the whole module under `--test-threads=1` if cross-test event
  bleed appears (notify watchers can be jumpy on shared tmpdirs).

## Notes

- Add `wiremock` (or `httpmock`) as a `dev-dependency` only.
- Keep e2e tests in `tests/cli/<module>.rs` to match existing layout.
- Mold fixture tests already cover transform behavior on golden data
  (`tests-molds/`); the gap above is on the surrounding pipeline.
