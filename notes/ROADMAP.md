# Fimod — Roadmap

Remaining features to implement.

## Registry mold cache

`[P0]` — Cache downloaded remote molds in `~/.cache/fimod/molds/` to avoid re-fetching on every invocation. Currently every `@name` from a remote registry triggers an HTTP GET. This becomes critical when molds include auxiliary files (templates, etc.) — each file is an extra request. The cache benefits all remote molds, not just those with templates.

### Catalog-level: ETag

The `catalog.toml` is the cache index. On fetch, store the server's `ETag` header. On subsequent requests, send `If-None-Match` — if the server returns `304 Not Modified`, the cached catalog is still valid (no body transferred). GitHub (raw.githubusercontent.com) and GitLab (raw file API) both support ETags. `--no-cache` bypasses the conditional request and forces a full fetch.

### Mold-level: content hash

`build-catalog` computes a deterministic hash per mold directory: sort all file paths alphabetically, hash each file's content, concatenate as `path:{hash}|path:{hash}|...`, and hash the result. One hash per mold in the catalog:

```toml
[molds.dockerfile-uv]
description = "Generate Dockerfile for uv projects"
hash = "a1b2c3d4"
```

Adding, removing, or modifying any file in the mold directory changes the hash. The client compares the catalog hash with the locally cached hash — mismatch triggers a re-download of the entire mold directory.

### Cache layout

```text
~/.cache/fimod/
  catalog/
    {source_hash}/
      catalog.toml         # cached catalog
      etag                 # stored ETag value
  molds/
    {source_hash}/
      {mold_name}/
        mold.py
        templates/
          Dockerfile.j2
        .cache-hash        # hash from catalog at download time
```

### Resolution flow

```text
1. GET catalog.toml + If-None-Match: "{etag}"
2. 304? → catalog unchanged, use cached catalog
   200? → new catalog, store it + new ETag
3. For the requested mold: compare catalog hash vs .cache-hash
4. Match?    → read mold from cache
   Mismatch? → re-download full mold directory, update .cache-hash
```

### CLI

- `--no-cache` — skip ETag check, force re-fetch catalog + mold
- `fimod registry cache clear` — wipe all cached molds
- `fimod registry cache clear @name` — wipe a specific mold

Prerequisite for `tpl_render_from_mold` to be practical with remote registries.

## `tpl_` Helpers (templating)

Data→text generation using Jinja2 templates via the `minijinja` crate. Extends Fimod's data→data pipeline to data→text for generating configs, reports, Dockerfiles, k8s manifests, etc.

Implementation: `minijinja` crate (~100-150 Ko added to binary). Pure Rust, `serde_json::Value`-native, created by the Jinja2/Flask author. All built-in Jinja2 filters (`upper`, `join`, `selectattr`, `tojson`…), loops, conditions, and macros come for free. Combined with `--output-format txt`, covers the data→text use case.

### `tpl_render_str` `[P1]`

`tpl_render_str(template, ctx, auto_escape=False)` — Render a Jinja2 template string with a context dict. No infrastructure change needed. Covers inline templates in molds.

```python
def transform(data, args, env, headers):
    return tpl_render_str("""
FROM python:{{ python_version }}-slim
{% for pkg in packages %}
RUN uv pip install {{ pkg }}
{% endfor %}
""", data)
```

### `tpl_render_from_mold` `[P2]`

`tpl_render_from_mold(path, ctx, auto_escape=False)` — Load a `.j2` file relative to the mold's directory and render it. The registry downloads the full mold directory (including subdirectories like `templates/`). Enables clean separation of logic (Python) and presentation (Jinja2).

```python
def transform(data, args, env, headers):
    tpl = args.get("template", "Dockerfile.j2")
    return tpl_render_from_mold(f"templates/{tpl}", data)
```

Depends on: registry mold cache (downloading a full directory per invocation is not practical without caching).

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

Depends on: `df_*` helpers (slurp already implemented via `-i f1 -i f2 -s`).

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

- **Devcontainer Feature** `[P2]` — separate repo `fimod-devcontainer-feature`, publishes an OCI Feature on GHCR. Allows `"features": { "ghcr.io/pytgaen/fimod-devcontainer-feature/fimod:1": {} }` in a `devcontainer.json` without a Dockerfile. The install.sh script detects the architecture and downloads the binary from GitHub Releases.
- **aqua-registry** `[P2]` — submit a PR to [aqua-registry](https://github.com/aquaproj/aqua-registry) to make fimod installable via `aqua g -i pytgaen/fimod`. Benefits: per-project tool pinning via a committed `aqua.yaml`, ideal for CI/CD and teams; aqua is on winget (`winget install aquaproj.aqua`), covers Linux/macOS/Windows. The PR is mechanical: a YAML file describing the release assets. **Bonus: once in the aqua-registry, mise can also use fimod via its aqua backend (`mise use aqua:pytgaen/fimod`), cleaner than the ubi backend.**
- **Manpage** `[P3]` — via `clap_mangen`
- **Module system** `[P3]` — imports between molds via `# fimod: import=utils,helpers`. Fimod resolves the referenced files and concatenates them before Monty compilation (no native Python import — Monty doesn't support it). Uses the existing `# fimod:` mechanism (`parse_mold_defaults()`). Resolution: same directory as the mold, then registry. *(De-prioritized: at the beginning of use cases, monolithic molds will be more than enough)*.
- **Schema validation** `[P3]` — validate input/output against a JSON Schema

## Future ideas

- **PyO3 Python API** `[P3]` — expose fimod as a native Python module via PyO3. Requires extracting the pipeline core into a separate `lib.rs` from `main.rs`. Would allow `import fimod; fimod.shape(data, mold="@pick_fields")` from Python. Significant effort, to consider if Python integration demand materializes.
- **Global config file** `[P3]` — `~/.config/fimod/config.toml` (or `$FIMOD_CONFIG`) to centralize HTTP defaults: custom User-Agent, default headers (e.g. auth tokens), timeout, proxy. To implement when other defaults (proxy, retries) justify a real config file.
- **PyPI distribution** `[P3]` — distribute fimod via PyPI following the `ruff`/`uv` model: platform-specific packages + meta-package. Pure CI/packaging effort.
- **npm distribution** `[P3]` — distribute fimod via npm following the `@biomejs/biome` model: platform-specific packages as `optionalDependencies`. Pure CI/packaging effort.
- **mise** `[P3]` — document installation via mise's ubi backend (`mise use ubi:pytgaen/fimod`); switch to `aqua:pytgaen/fimod` once the aqua PR is merged. No work on fimod's side, purely documentation.

---

## Summary Table

Priority legend: **P1** critical, **P2** important, **P3** nice-to-have.
Impact legend: **+++** transforms the product, **++** strong improvement, **+** minor improvement.
Complexity legend: 🟢 simple, 🟡 moderate, 🔴 complex.

| # | Feature | Prio | Impact | Complexity | Estimated time | Dependencies |
| - | ------- | ---- | ------ | ---------- | -------------- | ------------ |
| 1 | Registry mold cache | P0 | ++ | 🟡 | 2d | — |
| 2 | `tpl_render_str` (Jinja2 inline) | P1 | ++ | 🟢 | 1d | `minijinja` crate |
| 3 | `tpl_render_from_mold` (Jinja2 file) | P2 | ++ | 🟢 | 1d | #1, #2 |
| 4 | `dt_` Helpers (date/time) | P2 | ++ | 🟢 | 1d | `chrono` crate |
| 5 | `df_` Helpers (diff/patch/merge) | P2 | +++ | 🟢 | 1d | `json-patch` crate |
| 6 | CI registry molds (`@gh-matrix`, `@diff`…) | P2 | ++ | 🟢 | 1-2d | #5 |
| 7 | Dry-Run Check (`@pr-diff-summary`) | P2 | ++ | 🟢 | 0.5d | #5 |
| 8 | Multi-document YAML (`---`) | P2 | + | 🟡 | 1d | — |
| 9 | `--jobs N` (parallel batch) | P2 | ++ | 🟡 | 2d | `rayon` crate |
| 10 | Devcontainer Feature | P2 | + | 🟢 | 1d | separate repo |
| 11 | aqua-registry | P2 | + | 🟢 | 0.5d | PR externe |
| 12 | Manpage (`clap_mangen`) | P3 | + | 🟢 | 0.5d | — |
| 13 | Schema validation (JSON Schema) | P3 | ++ | 🟡 | 2-3d | crate to pick |
| 14 | Module system (`# fimod: import=`) | P3 | ++ | 🟢 | 1d | `parse_mold_defaults()` |
| 15 | Multi-file output | P3 | + | 🔴 | 2-3d | — |
| 16 | Large files streaming | P3 | + | 🔴 | 3-5d | — |

### Suggested next sprint

**Foundations Sprint** (~3d):
Registry mold cache → `tpl_render_str`

**CI/data Sprint** (~2-3d):
`df_` helpers → CI molds (`@pr-diff-summary`, `@diff`…)

**Helpers Sprint** (~1d):
`dt_` helpers
