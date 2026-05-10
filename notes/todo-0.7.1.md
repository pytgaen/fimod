# TODO 0.7.1 — closed

Snapshot of the 0.7.1 cycle. All items below are done; this file is the
historical record for the release. Items move to user docs / changelog
on the `chore(release): 0.7.1` commit.

## E2E test coverage gaps

Data and mold paths had unit tests but no real end-to-end coverage on
the hot edges. Several modules were exercised only indirectly through
string assertions on stdout/stderr.

### HTTP (`src/http.rs`)

The whole `--input-format http` path was uncovered. Tests grepped for
the string "http" but never issued a real request.

**Goal was not coverage — it was forcing the design through real edge
cases.** A fake HTTP server (`httpmock`) revealed dormant bugs that
unit tests on individual functions could not.

- [x] timeout (`--timeout`) — connection vs read timeout behavior
- [x] non-2xx status (401, 404, 500) → error surfaced cleanly with the
  status code, not a generic "request failed"
- [x] redirects: default follow chain (3xx → 2xx) vs `--no-follow`
- [x] `Content-Type` auto-detection (`content_type_to_format`):
  application/json, yaml, toml, csv, text/plain, missing CT, garbage
  CT, CT with charset suffix
- [x] authentication via `--http-header Authorization: Bearer ...` and
  `Basic ...` — header reaches the server unchanged across redirects
- [x] malformed `--http-header` input rejected by `parse_header` before
  any request is issued
- [x] `fetch_url_bytes` raw byte path (binary passthrough via
  `set_output_format("raw")`) — bytes arrive unmodified
- [x] proxy / `HTTPS_PROXY` env handling

### Data formats (`src/format.rs`, `src/convert.rs`)

- [x] CSV ↔ JSON ↔ NDJSON with mixed types, quoted delimiters, BOM
- [x] `--csv-delimiter` vs `--csv-output-delimiter` divergence
- [x] TXT format: `Value::String` bare emission vs non-string fallback
- [x] `DataFormat::Txt` chained with `-i` (txt-out → txt-in pipe)
- [x] `--in-place` with format auto-detection from path
- [x] malformed input → error path (not panic) for each parser

### Mold execution (`src/mold.rs`, `src/pipeline.rs`)

- [x] `transform(data, args, env, headers, **_)` keyword-arg contract
- [x] `--arg name=value` typing: int, float, bool, string, JSON
  (pinned: all values pass as strings, no auto-coercion)
- [x] `--env PATTERN` glob filtering: `*`, `PREFIX_*`, `EXACT`,
  comma-separated, no match
- [x] `set_output_format(...)` from inside a mold (incl. `"raw"` rules)
- [x] `headers` parameter: list for CSV, `None` otherwise
- [x] inline `-e` auto-wrap into `transform(...)`
- [x] mold from registry (`@name`) vs local path vs stdin
- [x] chained molds (`@a | @b`) — data shape preserved across boundary

## Watch mode tests (`src/watch.rs`)

- [x] **Debounce**: 5 rapid writes within `DEBOUNCE_MS` → exactly 1
  re-run.
  (Done: revealed two real bugs — `AnyContinuous` events triggering
  reruns + first-level debouncer flushing multi-batches under inotify
  cross-process latency. Fixed by `Any`-only filter + second-level
  500 ms quiet window in `src/watch.rs`.)
- [x] **Atomic save**: editor pattern `write tmp + rename → in.json`
  (vim, VSCode, IntelliJ all do this).
- [x] **File deletion + recreation**: `rm in.json && cp new in.json` →
  watcher recovers (or fails with a clear message — pin whichever the
  design picks).
  (Done: design = warn + continue. `src/watch.rs` tracks input file
  presence transitions across batches; `(true → false)` logs
  `[watch] warn: input removed, waiting for it to reappear...`,
  recreate triggers a normal rerun via existing canonicalize-match
  filter. Atomic save (rename) reste silencieux car résolu dans la
  fenêtre quiet window — transition `(true, true)`.)
- [x] **Mold file changes**: `collect_watch_files` claims to include
  the mold; verify it. Edit `mold.py` while watching → re-run fires.
- [x] **Transient mold error**: mold panics on run #2 → watcher does
  NOT exit, re-runs cleanly on run #3 with fixed input.
- [x] **Malformed input**: input becomes invalid JSON mid-watch →
  error to stderr, watcher stays alive.
- [x] **SIGINT / SIGTERM**: graceful shutdown, no zombie child.
  (Done: Unix-only tests pin default = exit by signal (signo 2 / 15)
  under 3 s. No subprocess in `run_shape_pipeline` → "no zombie"
  trivially satisfied.)
- [x] **Missing input at startup**: `--watch -i nonexistent.json` →
  clear error, not a silent hang.
  (Done: revealed real bug — silent hang. Fixed by bailing at startup
  if input file does not exist.)
- [x] **`FIMOD_WATCH_QUIET_MS` env var**: pin that overriding the
  default quiet window changes coalescing behavior.
  (Done: env=50 + 2 writes spaced 800 ms → ≥ 3 runs (no coalescing).
  Pins that the env var is read.)

## Notes

- `wiremock`/`httpmock` added as a `dev-dependency` only.
- e2e tests live in `tests/cli/<module>.rs`, matching existing layout.
- Mold fixture tests already cover transform behavior on golden data
  (`tests-molds/`); the gap above was on the surrounding pipeline.
