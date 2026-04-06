# Fimod — Roadmap

Remaining features to implement.

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

## Power user

- **Multi-document YAML** `[P2]` — support for `---` separators
- **`--jobs N`** `[P2]` — parallelize batch processing (`-i *.json`) over N threads. Files are independent, so the gain is immediate. Implementation: `rayon` or `std::thread::spawn` + result channel. Watch out: stdout interleaving, partial error handling, and interaction with `--in-place` to be addressed.
- **Multi-file output** `[P3]` — a mold that generates N files (jsonnet style)
- **Large files streaming** `[P3]` — line-by-line processing without loading everything into memory. Limited to line-oriented formats (NDJSON, CSV, TXT/lines); tree formats (JSON, JSON5, YAML, TOML) require full loading by nature.

## Ecosystem

- **Devcontainer Feature** `[P2]` — separate repo `fimod-devcontainer-feature`, publishes an OCI Feature on GHCR. Allows `"features": { "ghcr.io/pytgaen/fimod-devcontainer-feature/fimod:1": {} }` in a `devcontainer.json` without a Dockerfile. The install.sh script detects the architecture and downloads the binary from GitHub Releases.
- **aqua-registry** `[P2]` — submit a PR to [aqua-registry](https://github.com/aquaproj/aqua-registry) to make fimod installable via `aqua g -i pytgaen/fimod`. Benefits: per-project tool pinning via a committed `aqua.yaml`, ideal for CI/CD and teams; aqua is on winget (`winget install aquaproj.aqua`), covers Linux/macOS/Windows. The PR is mechanical: a YAML file describing the release assets. **Bonus: once in the aqua-registry, mise can also use fimod via its aqua backend (`mise use aqua:pytgaen/fimod`), cleaner than the ubi backend.**
- **Manpage** `[P3]` — via `clap_mangen`
- **Module system** `[P3]` — imports between molds via `# fimod: import=utils,helpers`. Fimod resolves the referenced files and concatenates them before Monty compilation (no native Python import — Monty doesn't support it). Uses the existing `# fimod:` mechanism (`parse_mold_defaults()`). Resolution: same directory as the mold, then registry. *(De-prioritized: at the beginning of use cases, monolithic molds will be more than enough)*.
- **Schema validation** `[P3]` — validate input/output against a JSON Schema

## Observability

### Local Usage Stats `[P3]`

Track `@mold` usage locally in `~/.config/fimod/usage.toml` — increment a counter + last-used timestamp on each registry mold resolution. No network, no opt-in required.

Add `fimod registry stats` to display usage:

```
@flatten_nested    47 runs   last: 2026-04-01
@pick_fields       23 runs   last: 2026-03-28
@badge_md           3 runs   last: 2026-03-15
```

Useful for the user (which molds do I actually use?) and for maintainers via voluntary sharing. Optional `--json` output for programmatic consumption.

## Workspace & WASM Playground `[P3]`

Split fimod into a Cargo workspace to cleanly separate the transformation core from CLI tooling:

```
fimod-core/     → convert, engine, format, mold, pipeline, regex, dotpath,
                  iter_helpers, hash, template, msg, gatekeeper, exit_control,
                  env_helpers, format_control
fimod-cli/      → main.rs, registry, test_runner, rustyline, clap
fimod-wasm/     → wasm-bindgen wrapper around fimod-core
```

**Why:** the current `lib.rs` re-exports everything including modules with platform-specific deps (`registry` → `home` crate, `main.rs` → `rustyline` → `fd-lock`). These two crates are the only WASM blockers — the rest of the pipeline compiles to `wasm32-unknown-unknown` as-is (verified 2026-04-03). A workspace split makes the core genuinely portable and enables a static-site playground (Monaco editor + WASM, hosted on GitHub Pages) with zero backend.

**Minimal alternative:** `#[cfg(not(target_arch = "wasm32"))]` gates on `registry` and `test_runner` in `lib.rs` + optional deps in `Cargo.toml`. Gets WASM working without restructuring, but doesn't modularize.

## Future ideas

- **PyO3 Python API** `[P3]` — expose fimod as a native Python module via PyO3. Pipeline core already extracted into `lib.rs` / `pipeline.rs`. Would allow `import fimod; fimod.shape(data, mold="@pick_fields")` from Python. Significant effort, to consider if Python integration demand materializes.
- **Global config file** `[P3]` — `~/.config/fimod/config.toml` (or `$FIMOD_CONFIG`) to centralize HTTP defaults: custom User-Agent, default headers (e.g. auth tokens), timeout, proxy. To implement when other defaults (proxy, retries) justify a real config file.
- **PyPI distribution** `[P3]` — distribute fimod via PyPI following the `ruff`/`uv` model: platform-specific packages + meta-package. Pure CI/packaging effort.
- **npm distribution** `[P3]` — distribute fimod via npm following the `@biomejs/biome` model: platform-specific packages as `optionalDependencies`. Pure CI/packaging effort.
- **mise** `[P3]` — document installation via mise's ubi backend (`mise use ubi:pytgaen/fimod`); switch to `aqua:pytgaen/fimod` once the aqua PR is merged. No work on fimod's side, purely documentation.

---

## Summary Table

Priority legend: **P1** critical, **P2** important, **P3** nice-to-have.
Impact legend: **+++** transforms the product, **++** strong improvement, **+** minor improvement.
Complexity legend: 🟢 simple, 🟡 moderate, 🔴 complex.

| # | Feature | Prio | Impact | Complexity | Dependencies |
| - | ------- | ---- | ------ | ---------- | ------------ |
| 1 | `df_` Helpers (diff/patch/merge) | P2 | +++ | 🟢 | `json-patch` crate |
| 2 | CI registry molds (`@gh-matrix`, `@diff`…) | P2 | ++ | 🟢 | #1 |
| 3 | Dry-Run Check (`@pr-diff-summary`) | P2 | ++ | 🟢 | #1 |
| 4 | Multi-document YAML (`---`) | P2 | + | 🟡 | — |
| 5 | `--jobs N` (parallel batch) | P2 | ++ | 🟡 | `rayon` crate |
| 6 | Devcontainer Feature | P2 | + | 🟢 | separate repo |
| 7 | aqua-registry | P2 | + | 🟢 | PR externe |
| 8 | Manpage (`clap_mangen`) | P3 | + | 🟢 | — |
| 9 | Schema validation (JSON Schema) | P3 | ++ | 🟡 | crate to pick |
| 10 | Module system (`# fimod: import=`) | P3 | ++ | 🟡 | `parse_mold_defaults()` |
| 11 | Multi-file output | P3 | + | 🔴 | — |
| 12 | Large files streaming | P3 | + | 🔴 | — |
| 13 | Local usage stats (`registry stats`) | P3 | + | 🟢 | — |
| 14 | Workspace split & WASM playground | P3 | ++ | 🟡 | `wasm-bindgen` |

### Suggested next sprint

**CI/data Sprint** (~2-3d):
`df_` helpers → CI molds (`@pr-diff-summary`, `@diff`…)
