# Fimod — Roadmap

Remaining features to implement.

## Power user

- **Multi-document YAML** `[P2]` — support for `---` separators
- **`--jobs N`** `[P2]` — parallelize batch processing (`-i *.json`) over N threads. Files are independent, so the gain is immediate. Implementation: `rayon` or `std::thread::spawn` + result channel. Watch out: stdout interleaving and partial error handling to be addressed.
- **Multi-file output** `[P3]` — a mold that generates N files (jsonnet style)
- **Large files streaming** `[P3]` — line-by-line processing without loading everything into memory. Limited to line-oriented formats (NDJSON, CSV, TXT/lines); tree formats (JSON, JSON5, YAML, TOML) require full loading by nature.

## `dt_` Helpers (date/time)

`[P2]` — Same pattern as `re_`, `dp_`, `hs_`. One of the most common gaps in data pipelines.

| Helper | Example |
| ------ | ------- |
| `dt_parse(s)` | Parse an ISO 8601 date → manipulable object |
| `dt_format(d, fmt)` | Format a date (`"%Y-%m-%d"`, `"%d/%m/%Y"`) |
| `dt_now()` | Current UTC date/time |
| `dt_diff(a, b)` | Difference in seconds between two dates |
| `dt_add(d, days=N)` | Add/subtract a duration |

Implementation: `chrono` crate (often already a transitive dependency). Use cases: normalize dates in CSV/JSON exports, filter by time range, add timestamps.

## `df_` Helpers (diff/patch)

`[P2]` — Structural diff and JSON patching via the [`json-patch`](https://crates.io/crates/json-patch) crate (RFC 6902 / RFC 7396, 2.5MA+ dl/month, operates directly on `serde_json::Value`). Same pattern as `re_`, `dp_`, `hs_`.

| Helper | Role |
| ------ | ---- |
| `df_diff(a, b)` | Generates the list of RFC 6902 operations to go from `a` to `b` |
| `df_patch(doc, ops)` | Applies an RFC 6902 patch on `doc`, returns the result |
| `df_merge(doc, overlay)` | RFC 7396 merge patch — merges `overlay` into `doc` |

RFC 6902 operations are standard Python dicts (`{"op": "replace", "path": "/name", "value": "Bob"}`), manipulable and filterable in molds. Use cases: auditing changes between versions, selective field migration, incremental sync, transformation validation. Wrapper molds (`@diff`, `@patch`, `@merge`) will be created on top for direct CLI usage.

## CI-oriented Killer Features

### Structured Dry-Run Check (PR Summary) `[P2]`

Coupled with `df_` (diff) helpers, provide a ready-to-use wrapper mold (e.g., `@pr-diff-summary`) that:

1. Reads the "Before" state and "After" state (via slurp).
2. Generates a summary of differences in a clean Markdown format.
3. Allows the CI to directly use this output to post a detailed and readable comment on a Pull Request (GitOps).

### CI Molds in the registry `[P2]`

Ready-to-use wrapper molds for CI/CD pipelines:

| Mold | Role | Example |
| ---- | ---- | ------- |
| `@gh-matrix` | Generate a GitHub Actions matrix from data | `fimod shape -i config.json -m @gh-matrix` |
| `@gh-annotate` | Transform errors/warnings into `::error file=...::` annotations | `fimod shape -i lint.json -m @gh-annotate` |
| `@gl-codequality` | Convert to GitLab CI Code Quality format | `fimod shape -i results.json -m @gl-codequality` |
| `@diff` | CLI wrapper over `df_diff` | `fimod shape -i before.json -i after.json -s -m @diff` |
| `@patch` | CLI wrapper over `df_patch` | `fimod shape -i data.json --arg patch=changes.json -m @patch` |
| `@merge` | CLI wrapper over `df_merge` | `fimod shape -i base.yaml -i overlay.yaml -s -m @merge` |

Depends on: `df_*` helpers, multi-file slurp.

## ✅ Verbosity & diagnostics

`--debug` kept for Rust traces (script displayed, input/output data, formats). Two new flags to control molds' `msg_*`:

| CLI Flag | Level | `msg_print/info/warn` | `msg_error` | `msg_verbose` | `msg_trace` |
| -------- | :----: | :-------------------: | :---------: | :-----------: | :---------: |
| `--quiet` | 0 | — | ✓ | — | — |
| (default) | 1 | ✓ | ✓ | — | — |
| `--msg-level=verbose` | 2 | ✓ | ✓ | ✓ | — |
| `--msg-level=trace` | 3 | ✓ | ✓ | ✓ | ✓ |

`--quiet` and `--msg-level` are mutually exclusive. `msg_verbose` and `msg_trace` are the two new external functions added.

## Ecosystem

- **Fichier de config global** `[P3]` — `~/.config/fimod/config.toml` (ou `$FIMOD_CONFIG`) pour centraliser les defaults HTTP : User-Agent personnalisé, headers par défaut (ex. tokens d'auth), timeout, proxy. Utile pour CI/CD où les env vars sont contraintes. Même structure que les flags CLI actuels (`http-header`, `timeout`, `no-follow`). À implémenter quand d'autres defaults (proxy, retries) justifieront un vrai fichier de config.
- **Devcontainer Feature** `[P2]` — separate repo `fimod-devcontainer-feature`, publishes an OCI Feature on GHCR. Allows `"features": { "ghcr.io/pytgaen/fimod-devcontainer-feature/fimod:1": {} }` in a `devcontainer.json` without a Dockerfile. The install.sh script detects the architecture and downloads the binary from GitHub Releases.
- **Manpage** `[P3]` — via `clap_mangen`
- **Module system** `[P3]` — imports between molds via `# fimod: import=utils,helpers`. Fimod resolves the referenced files and concatenates them before Monty compilation (no native Python import — Monty doesn't support it). Uses the existing `# fimod:` mechanism (`parse_mold_defaults()`). Resolution: same directory as the mold, then registry. *(De-prioritized: at the beginning of use cases, monolithic molds will be more than enough)*.
- **Schema validation** `[P3]` — validate input/output against a JSON Schema

## Future ideas



---

## Summary Table

Priority legend: **P1** critical, **P2** important, **P3** nice-to-have.
Impact legend: **+++** transforms the product, **++** strong improvement, **+** minor improvement.
Complexity legend: 🟢 simple, 🟡 moderate, 🔴 complex.

| # | Feature | Prio | Impact | Complexity | Estimated time | Dependencies |
| - | ------- | ---- | ------ | ---------- | -------------- | ------------ |
| 1 | `dt_` Helpers (date/time) | P2 | ++ | 🟢 | 1d | `chrono` crate |
| 2 | `df_` Helpers (diff/patch/merge) | P2 | +++ | 🟢 | 1d | `json-patch` crate |
| 3 | CI registry molds (`@gh-matrix`, `@diff`…) | P2 | ++ | 🟢 | 1-2d | #2 |
| 4 | Dry-Run Check (`@pr-diff-summary`) | P2 | ++ | 🟢 | 0.5d | #2 |
| 5 | Multi-document YAML (`---`) | P2 | + | 🟡 | 1d | — |
| 6 | `--jobs N` (parallel batch) | P2 | ++ | 🟡 | 2d | `rayon` crate |
| 7 | Devcontainer Feature | P2 | + | 🟢 | 1d | separate repo |
| 8 | Manpage (`clap_mangen`) | P3 | + | 🟢 | 0.5d | — |
| 9 | Schema validation (JSON Schema) | P3 | ++ | 🟡 | 2-3d | crate to pick |
| 10 | Module system (`# fimod: import=`) | P3 | ++ | 🟢 | 1d | `parse_mold_defaults()` |
| 11 | Multi-file output | P3 | + | 🔴 | 2-3d | — |
| 12 | Large files streaming | P3 | + | 🔴 | 3-5d | — |
| 13 | PyO3 Python API | P3 | ++ | 🔴 | 5d+ | `lib.rs` extraction |

### Suggested next sprint

**CI/data Sprint** (max impact, ~2-3d):
`df_` helpers → CI molds (`@pr-diff-summary`, `@diff`…)

**Helpers Sprint** (~1d):
`dt_` helpers
