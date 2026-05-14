# Changelog

All notable changes to fimod are documented here.

## [0.7.3] — 2026-05-14

Internal-only release: hot-path caches (`reqwest::Client`, compiled minijinja `Environment`s), zero-copy `MontyObject` threading between chain steps, and a broad helper/pipeline-metadata cleanup. No user-facing CLI or mold-API changes.

### Bug Fixes

- **format:** parse TOML directly into `serde_json::Value` — was needlessly round-tripping through a JSON-string intermediate.

### Performance

- **http:** cache `reqwest::blocking::Client` per `(timeout, no_follow)` tuple. Builds an internal connection pool that was previously discarded between calls. Wins on `fimod registry build-catalog` (N source fetches) and chain molds with multiple `--input-format http` steps.
- **template:** cache compiled minijinja `Environment` process-wide, keyed by `(template_source, auto_escape)`. Eliminates N×compile work on per-row template rendering. Cache is unbounded (same rationale as `regex.rs::REGEX_CACHE`); LRU bound deferred to follow-up if it bites.
- **template:** switch template cache key to `(u64 hash, bool)` with original `Arc<str>` stored as part of the value for collision verification. Lookup allocates zero bytes on the common path; insertion clones once into `Arc<str>` (O(1)-cloneable thereafter).
- **engine:** thread `MontyObject` between chain steps instead of converting to `serde_json::Value` and back. Only chain-exit (I/O serialize boundary) and `set_input_format` mid-chain still pay the round-trip.

### Refactoring

- **pipeline:** introduce `PipelineMetadata` and `ChainExecCtx`; drop the ad-hoc `context_base` `serde_json::Object`. `execute_chain` arg count drops from 9 to 5; `run_pipeline_core` from 13 to 6.
- **engine:** embed `PipelineMetadata` by reference into `MoldOptions` so the 5 chain-wide fields (`input`, `output`, `in_place`, `slurp`, `no_input`) live in one place instead of being duplicated.
- **pipeline:** add `execute_chain_to_value(...) -> ChainOutput` helper for the chain-exit `MontyObject → Value` boundary, deduplicating the conversion between `run_pipeline_core` and `cmd::shape` slurp.
- **pipeline:** extract `url_path_only` (strip `?query`/`#fragment` from URL — 3 call sites) and `write_bytes_to` (raw output path — 3 call sites) helpers.
- **cmd/shape:** extract `validate_post_input_list` and `run_raw_passthrough` from `run_shape`. Function drops from 200+ to ~110 lines and reads linearly.
- **mold:** add `find_script(name, base)` helper as the single source of truth for the 3-rule local-mold lookup (`name.py` → `name/last.py` → `name/__main__.py`); deduplicate `resolve_local` and `find_local_mold_script`.
- **mold:** add `load_defaults(path)` helper (returns `Result<MoldDefaults>`); 5 call sites in `registry/catalog.rs` and `registry/molds.rs` collapse from inlined `fs::read_to_string + parse_mold_defaults` dances.
- **monty_args:** add `expect_string_owned(obj, label)` helper. Eight call sites in `format_control.rs`, `template.rs`, `engine.rs` collapse from 4-line match blocks to 1 line each.
- **paths:** promote `cache_dir()` to `paths.rs` so the `HOME` / `USERPROFILE` / `.` fallback convention lives in one place. Drop `registry::cache_base_dir` re-export.
- **registry:** use `unwrap_or_default()` in path-stem extraction (standard idiom).

### Housekeeping

- **deps:** bump `httpmock` 0.7.0 → 0.8.3 (major dev-dep bump pre-announced as deferred in 0.7.2) and migrate the 8 call sites in `tests/cli/http_e2e.rs` from `assert_hits` to `assert_calls` (deprecated in 0.8). Unblocks `task outdated` for this release cycle.
- **comments:** trim WHAT-comments per project rule (`pipeline.rs::execute_chain`, `engine.rs::MoldOptions`, `cmd/shape.rs::build_script_refs`).
- **notes:** close `notes/todo-0.7.2.md` cycle backlog (content shipped); open `notes/todo-0.7.3.md` cycle backlog.
- **style:** rustfmt cleanup in `template.rs` (cargo fmt joined a 2-line `let template_str = expect_string_owned(...)` after surrounding-file modifications).
- **deps deferred:** `reqwest` 0.12 → 0.13 (major bump, full HTTP surface to re-validate including timeout/redirect/proxy/auth-across-redirects/raw bytes paths covered by `tests/cli/http_e2e.rs`) — deferred to a dedicated `chore(deps)` PR in a later cycle. Release cut via `/release-workflow --allow-outdated`.

## [0.7.2] — 2026-05-14

### Highlights

- 🔧 **Internal restructure (no user-visible changes)** — `src/main.rs` and `src/registry.rs` had grown to 1261 and 2180 lines respectively. Split into `src/cli.rs` + `src/cmd/{shape,registry,mold,monty,setup,completions}.rs` for the CLI layer, and `src/registry/{config,resolve,catalog,molds}.rs` for the registry. Public library API preserved via `pub use` re-exports in `registry/mod.rs`.
- 📚 **Code layout documented** — new `notes/CODE_LAYOUT.md` maps every file in the project to its responsibility, with a "where do I put this change?" decisional table. `notes/ARCHITECTURE.md` refreshed to reflect the post-split module map and extension points.

### Bug Fixes

- **cli:** add `value_hint` annotations on path arguments (`-i`, `-o`, `-m`, `--input-list`, `--sandbox-file`) so dynamic shell completion (`COMPLETE=zsh fimod ...`) proposes filesystem paths instead of falling back to defaults.

### Refactoring

- **registry:** scaffold `src/registry/` module and extract `config` submodule (`Source`, `SourceType`, `SourcesConfig`, `sources.toml` CRUD).
- **registry:** extract `resolve` submodule (`@name` / `@source/name` resolution, env registries, auth headers).
- **registry:** extract `catalog` submodule (`Catalog`, `fetch_catalog`, `build_catalog`, URL/raw cache, `compute_mold_hash`, `github_to_raw`).
- **registry:** extract `molds` submodule (`list_molds`, `show_mold`, `MoldMatch`, completion helpers).
- **registry:** absorb the `setup` wizard into `src/setup.rs`, removing the `registry::setup` trampoline.
- **registry:** tighten visibility post-split (`pub` → `pub(crate)` for internal items; `pub` reserved for the contract exposed by `registry/mod.rs`).
- **cli:** extract clap CLI definitions (`Cli`, `Commands`, `ShapeArgs`, all subcommand enums, completion helpers) into `src/cli.rs`. `main.rs` now consumes them as `use cli::*;`.
- **cmd:** scaffold `src/cmd/` module and extract the `monty` subcommand handler (REPL).
- **cmd:** extract the `completions` subcommand handler into `cmd/completions.rs`.
- **cmd:** extract the `shape` subcommand handler (`run_shape`, `run_shape_pipeline`, ~570 lines) into `cmd/shape.rs`.
- **cmd:** move `src/setup.rs` to `src/cmd/setup.rs` (homing all subcommand handlers under `cmd/`).
- **cmd:** extract `registry` and `mold` dispatchers as thin `cmd::*` façades that route into `registry::*` and `mold::*`.

### Documentation

- New `notes/CODE_LAYOUT.md` — full project map (top-level, `src/`, `cmd/`, `registry/`, `tests/`, `tests-molds/`, `docs/`, `notes/`, `scripts/`, `.github/`, `molds/`) plus a "where do I put this change?" decisional table.
- `notes/ARCHITECTURE.md` — mermaid module map, Layers table, and Extension Points table refreshed for the `cli.rs` + `cmd/*` + `registry/*` split. New pointer to `CODE_LAYOUT.md` in the "What's NOT in this doc" section.
- `CLAUDE.md` — list `notes/CODE_LAYOUT.md` alongside the other notes/ files so future sessions discover it.

### Housekeeping

- **task:** fix `task outdated` invocation — was calling `cargo-outdated` as a standalone binary (which fails parsing the `--root-deps-only` flag); now correctly invokes `cargo outdated` via `mise exec`.
- **notes:** remove closed `notes/todo-0.7.1.md` cycle (content migrated to user docs in the 0.7.1 release).
- **deps:** bump `clap_complete` 4.6.4 → 4.6.5 (patch).
- **deps deferred:** `httpmock` 0.7.0 → 0.8.3 (major dev-dep bump, breaking matcher API `assert_hits` → `assert_calls`, MSRV bumped to 1.88) — migration deferred to 0.7.3 (PR #20). Release cut via `/release-workflow --allow-outdated`.

## [0.7.1] — 2026-05-14

### Highlights

- 👁️ **Watch hardening** — input-removal warning, second-level event coalescing, atomic-save (rename) detection. New env var `FIMOD_WATCH_QUIET_MS=<ms>` to tune the debounce window.
- 🧪 **E2E coverage expansion** — full HTTP module (timeout, 4xx/5xx, redirects, content-type auto-detect, auth across redirects, proxy, raw bytes), CSV/NDJSON/TXT cross-format, mold contract pinning (`transform` kwargs, `--arg`, `--env`, `set_output_format`), watch failure modes (delete/recreate, mold panic, malformed input mid-watch, SIGINT/SIGTERM).

### Bug Fixes

- **watch:** bail at startup when input file is missing — was hanging silently.
- **watch:** coalesce notify event bursts via second-level debounce (default 500 ms) to absorb cross-process inotify latency. Configurable via `FIMOD_WATCH_QUIET_MS=<ms>`.
- **watch:** surface input removal mid-watch with `[watch] warn: input removed, waiting for it to reappear...`. Atomic saves (rename via tmp + persist) stay silent — only sustained absence triggers the warning.

### Documentation

- `docs/guides/cli-reference.md` — watch section: two-level debounce, `FIMOD_WATCH_QUIET_MS` env var, input-removal UX.

### Housekeeping

- task `outdated` + `cargo-outdated` via mise (root-deps freshness check). Note: the Taskfile entry currently invokes `cargo-outdated` as a standalone binary instead of via `cargo outdated`, which fails with clap; to be fixed in a follow-up `chore` PR.
- **deps:** bump `assert_cmd` 2.2.2, `clap_complete` 4.6.4 (patch).
- **deps deferred:** `reqwest` 0.13 (major bump, HTTP surface to re-validate), `httpmock` 0.8 (dev dep, breaking matcher API), `clap_complete` 4.6.5 (patch) deferred to dedicated `chore(deps)` PRs in a later cycle.

## [0.7.0] — 2026-05-08

### Highlights

- 👁️ **Watch mode** — `fimod shape --watch <input>` re-runs the pipeline on every file change. 150 ms debounce, parent-dir watching (atomic-rename safe), errors don't kill the loop. Watches both `-i` and local `-m` paths.
- 🐞 **Debug phases & step labels** — `--debug` prints per-phase timings (`parse: 0.045s`, `execute: 0.123s`, …) and identifies each step in chains (`step 2/4 (label, injected by step 1)`).
- ⚙️ **CLI setup restructure** *(small BREAKING)* — `fimod setup completions --shell <shell>` replaces `fimod completions <shell>`. Prints the script directly to stdout for `eval "$(fimod setup completions --shell zsh)"`.
- 🚀 **Performance** — regexes cached per pattern (process-wide); `dp_get` / `dp_has` walk `MontyObject` directly without JSON conversion, fixing O(N²) cost in 100 k-row mold loops.

### Features

- **watch:** `fimod shape --watch <input>` re-runs the pipeline on changes. Watches parent dirs of `-i` and local `-m` files (atomic-rename safe), filters events by canonicalized target path, debounces 150 ms via `notify-debouncer-mini`. Status on stderr (`[watch] run #N ok (Xms)` / `failed (Xms)\n  <err>`); errors don't exit the loop. First run immediate at startup. Output destinations unchanged. Gated behind feature `watch` (enabled by default).
- **cli:** `--watch` (`-w`) flag with combo validation. Refused: `--in-place`, `--no-input`, `--input-list`, `--output-format raw`, missing `-i`, multiple `-i` (batch), HTTP input.
- **debug:** per-phase timings on stderr — `[debug] parse: …s`, `execute: …s`, `serialize: …s`, `total: …s`. Always seconds with millisecond precision.
- **debug:** step identification in chain output and errors — `[debug] step N/M (label)` replaces the single `[debug] mold: file(...)` line. Errors prefixed `Error: in step N/M (label): …`. Runtime-injected steps annotated `injected by step P`.
- **cli:** `fimod setup completions --shell <shell>` prints completion script to stdout. Auto-detects `$SHELL` when `--shell` is omitted. Generated via `clap_complete::env::Shells::builtins`, preserving dynamic completions for `@mold` and registry source names.
- **env:** `--env` glob patterns accept `*SUFFIX` and `*INNER*` (previously only `*` and `PREFIX*`). Side effect of consolidating `sandbox::matches_glob` into the env-pattern path.

### Bug Fixes

- **setup:** `fimod setup registry defaults --force` actually honors `--force` (was silently dropped by `force: _` in the Registry match arm). When set, removes any source whose URL matches the canonical `fimod-powered` / `examples` URLs before re-installing them under the canonical name and priority. Supported way to take ownership when those community registries have been renamed or repointed by hand. The deprecated `fimod registry setup` alias still passes `force=false`.
- **install:** `install.sh` correctly detects non-interactive contexts (Docker without `-it`, GitHub Actions, `ssh` without `-t`) by attempting to open `/dev/tty` (`(: </dev/tty) 2>/dev/null`) instead of just testing existence (`[ -e /dev/tty ]`). Fixes the registry/sandbox prompt branches that previously crashed on `cannot open /dev/tty`.

### Performance

- **regex:** compiled regexes cached process-wide via `OnceLock<Mutex<HashMap<String, Arc<Regex>>>>`, keyed by pattern. First call compiles and stores; subsequent calls return an `Arc` clone. Was rebuilding `fancy_regex::Regex` on every `re_*` invocation — 100 k recompilations of the same pattern in a 100 k-row CSV mold loop.
- **dotpath:** `dp_get` / `dp_has` walk `MontyObject` directly via new `get_at_path_monty`. Previously converted the entire input tree to `serde_json::Value` on every invocation — O(N²) on top-level data of size N. `dp_get` now clones only the leaf value; `dp_has` doesn't clone at all.

### Documentation

- `docs/guides/cli-reference.md` — new "Watch mode" section (typical usage, output destinations, error survival, watched-files scope, refused-combos table, debounce/parent-dir/reload semantics, feature-flag opt-out).
- `README.md` — advertised binary size updated to ~2.9 MB.

### Housekeeping

- `[lint]` header removed from `molds/.ruff.toml`.
- **deps:** bump `notify` 6 → 8, `notify-debouncer-mini` 0.4 → 0.7, `toml` 0.8 → 1 (rename feature `indexmap` → `preserve_order`).

## [0.6.0] — 2026-05-03

### Highlights

- ✨ **Forced directives `!=`** — molds can lock `input-format` / `output-format` so the CLI cannot override them.
- 🔧 **Dynamic pipelines** — `transform()` receives a `pipeline` object: read/mutate the current and future steps, inject new steps via `Step.create(...)`.
- 📦 **Five new registry molds** showcasing dynamic patterns: `with_threshold` and `sample_if_large` (compute-then-inject), `auto_anonymize` (conditional routing), `compact_if_big` (adaptive output), `checkpoint` (fail-fast on empty data).
- 🔍 **`fimod mold show --path`** — inspect a mold by file path, no registry lookup required.
- 📁 **Single-`.py` directory fallback** — `-m dir/` auto-resolves the unique `.py` file inside.

### Features

- **mold:** forced directives `!=` lock `input-format` / `output-format` against CLI overrides. `--debug` traces with `[debug] mold forces output-format=yaml`. Mixable with plain defaults on the same mold.
- **pipeline:** `transform(data, pipeline, **_)` exposes the running pipeline. Methods: `pipeline.current_step()`, `pipeline.step(i)`, `pipeline.length()`, `pipeline.insert_next(...)`, `pipeline.append(...)`.
- **pipeline:** read step fields via `step.get('key')` — `index`, `input_format`, `output_format`, `input`, `output`, `in_place`, `slurp`, `no_input`, `args`. Unknown keys raise `Step.get('<key>'): unknown field`.
- **pipeline:** mutate the current step via `step.set('key', value)` — `exit`, `output_format`, `output_file`, `input_format` (re-parses body, same effect as `set_input_format()`).
- **pipeline:** future-step mutations propagate to the actual execution: `output_format` to format override, `output_file` to `MoldOptions.output_path`, `input_format` re-parses before that step runs, `args` replaces the step's args block (CLI `--arg` merge still applied at exec).
- **pipeline:** dynamic step injection via `pipeline.insert_next(Step.create(...))` and `pipeline.append(Step.create(...))`. Single contract: `Step.create(mold | expr, input_format, output_format, args)` — exactly one of `mold` or `expr` required. Bare-kwargs and raw-dict shortcuts are not accepted.
- **pipeline:** `Step.create(args={...})` propagates typed args (bool, int, nested dict — not only string→string) to the injected mold; merged with CLI `--arg`, spec wins on key conflict.
- **pipeline:** legacy globals `set_exit_code()`, `set_input_format()`, `set_output_format()`, `set_output_file()` continue to work unchanged.
- **mold:** `fimod mold show --path <file>` inspects a mold by direct path (no registry lookup). Optional positional name override; works with `__main__.py` directories.
- **mold:** `-m dir/` resolves automatically when the directory contains exactly one `.py` file. Multiple `.py` files raise an explicit error listing the candidates.
- **mold:** `fimod mold list` now displays `[registry-name]` next to each entry.
- **registry:** new `with_threshold` mold — compute-then-inject pattern; computes a percentile threshold live and injects a downstream filter via `Step.create(args=...)`. 6 fixtures.
- **registry:** new `auto_anonymize` mold — conditional routing; inspects CSV headers and appends `@anonymize_pii` only when sensitive columns are present. 2 fixtures.
- **registry:** new `compact_if_big` mold — adaptive output; flips `output_format` to `json-compact` when data exceeds a configurable threshold. 4 fixtures.
- **registry:** new `sample_if_large` mold — compute-then-inject; injects a sampling step after itself when a list exceeds `--arg max=N` (`strategy=head|tail`). Strict arg validation (missing / non-integer / non-positive raise clear errors).
- **registry:** new `checkpoint` mold — fail-fast guard; exits with non-zero code when data is empty (`null`, `""`, `[]`, `{}`), passes data through unchanged otherwise. Configurable `--arg label=...` and `--arg exit_code=N`.

### Documentation

- New page `docs/guides/dynamic-molds.md` — philosophy of dynamic molds (compute-then-inject, conditional routing, args propagation, adaptive output), comparison vs jq / yq / miller / Beam, when not to use, snapshot-vs-fan-out limitations. Linked in `mkdocs.yml` nav after Mold Scripting.
- `docs/guides/mold-scripting.md` — full pipeline section refresh: migrated to `step.get()` / `step.set()` API, added `args` row on read/write tables, added `args=` to the `Step.create()` table, added a snapshot-semantics admonition for `pipeline.length()` / `pipeline.step(i)`.
- `docs/reference/mold-defaults.md` — full `!=` forced-directive section with examples.
- `docs/guides/authoring-molds.md` and `docs/guides/mold-scripting.md` — `!=` tip added near the defaults section.

### Housekeeping

- `molds/.ruff.toml` — list fimod-injected built-ins (`Step`, `dp_*`, `msg_*`, `re_*`, `it_*`, `hs_*`, `gk_*`, `set_*`, `tpl_*`, `env_subst`) under `[lint].builtins` so IDE linters (Ruff, Pyrefly) stop reporting `F821` false positives on registry molds.
- `.markdownlint.json` — disable MD056 (false positive on pipes inside code spans within tables).
- 11 new integration tests in `tests/cli/pipeline.rs` covering pipeline injection, current/future-step mutation, args propagation, and type guards.

## [0.5.0] — 2026-04-25

### Highlights

- 🔒 Sandbox isolation layer — `sandbox.toml` with allow/deny rules, clock control, hard limits (2 min / 1 GB), exit code 137. Four-level config resolution: `--sandbox-file` > env > `~/.config/fimod/sandbox.toml` > zero-auth.
- ⚙️ New `fimod setup` subcommand — `setup registry defaults`, `setup sandbox defaults`, `setup all defaults` replace `fimod registry setup` (deprecated, removed in 0.10.0).
- 📦 Installer dual-path — `install.sh` and `install.ps1` detect the version and route to new commands for ≥ 0.5.0, legacy API for < 0.5.0.
- ⬆️ Monty v0.0.14 → v0.0.17 — `hasattr`, `setattr`, chain assignment.

### Features

- **sandbox:** add sandbox isolation layer (`sandbox.toml`, allow/deny rules, clock control)
- **setup:** new `fimod setup` subcommand with `setup registry defaults` and `setup sandbox defaults`
- **install:** dual-path `install.sh` — routes to new setup commands for ≥ 0.5.0, legacy for < 0.5.0
- **paths:** centralize `config_dir()` in paths module shared by registry, sandbox, setup

### Housekeeping

- Upgrade Monty v0.0.14 → v0.0.17 (`hasattr`, `setattr`, chain assignment)
- Bump `rustls-webpki` from 0.103.12 to 0.103.13
- Bump `rand` from 0.8.5 to 0.8.6

## [0.4.0] — 2026-04-17

### Highlights

- ⬆️ **🐍 Monty v0.0.11 → v0.0.14** — natural JSON support in `MontyObject`, `ExternalExceptionData`, `u32` `CodeLoc` (fix panic on files >65k lines).
- ✨ **New dotpath built-ins** — `dp_has`, `dp_delete` complete the dotpath toolkit.
- ✨ **New iter built-ins** — `it_count_by`, `it_min_by`, `it_max_by`; `it_sort_by` gains a `reverse` flag.
- 📦 **New `filter_fields` mold** — keep/drop fields by dotpath (with nested path support).

### Features

- **built-ins:** `dp_has(data, path)` tests path existence; `dp_delete(data, path)` removes key/index (silent no-op on missing, shifts array elements).
- **built-ins:** `it_count_by(array, key)` returns counts per field value (insertion order).
- **built-ins:** `it_min_by` / `it_max_by` return the element with smallest/largest field value (stable: first tie wins).
- **built-ins:** `it_sort_by(array, key, reverse=True)` for descending order.
- **mold:** `filter_fields` — keep or drop fields using dotpath patterns; handles nested paths and arrays.

### Bug Fixes

- **serde:** wrap `serde_json::Value` in `NativeNumbers` before handing it to non-serde_json serializers (TOML, YAML, minijinja). Monty v0.0.14 transitively enables serde_json's `arbitrary_precision` feature, which would otherwise cause numbers to be emitted as `{"$serde_json::private::Number": "..."}` instead of bare `i64`/`u64`/`f64`.

### Documentation

- Updated `built-ins.md` with new dotpath/iter functions.
- Updated `monty-engine.md` version references.
- Expanded `mold-gallery.md` with `filter_fields`.

### Housekeeping

- New `src/serde_compat.rs` with `NativeNumbers` newtype + tests.
- Notes: `monty-v0.0.12.md`, `monty-v0.0.13.md`, `monty-v0.0.14.md`, `tohl-spec.md`.
- Renamed `notes/monty-0.0.11.md` → `notes/monty-v0.0.11.md` for consistency.

## [0.3.1] — 2026-04-12

### Bug Fixes

- **iter:** `it_group_by` preserves insertion order (switched to `IndexMap`) — grouped output follows data order instead of alphabetical.
- **mold:** `# fimod:` directives support quoted arg descriptions (`"..."` / `'...'`) so commas don't split entries.
- **cli:** `--mold=` / `--expression=` long-form flags now recognized by the pre-parse pass.
- **engine:** intercepted `OsCall` emits a `[debug]` message instead of failing silently.

### Features

- **mold:** `fimod mold show --output-format json` for machine-readable output.

### Documentation

- Expanded `formats.md` (lines vs txt vs NDJSON, shell-friendly recipes).
- Updated `cli-reference.md` and `built-ins.md`.

### Housekeeping

- Added `indexmap` dependency.
- Removed dead `parse_data` / `parse_file` from `pipeline.rs`.

## [0.3.0] — 2026-04-10

### Highlights

- 🔀 Migrate production molds to dedicated repo [fimod-powered](https://pytgaen.github.io/fimod-powered/) (`gh_latest`, `download`, `poetry_migrate`, `skylos_to_gitlab`). Use `fimod registry setup` to migrate
- 👀 **fimod-powered** - New molds showcasing Jinja2 templating (MiniJinja engine): `html_report`, `dockerfile`
- **📌 Priority-based registry resolution** - registries searched in priority order (P0→P99). New `set-priority` command replaces `set-default`.
- **🐚 Dynamic shell completions** - context-aware Tab completion for subcommands, flags, format names, `@mold` references, and registry source names.
- ⬆️ Update to **🐍 Monty v0.0.11**/cle

### Features

- **completions:** dynamic shell completions via `clap_complete` `CompleteEnv` — Tab-completes format names, `@mold` references, registry source names. New `fimod completions <shell>` subcommand.
- **registry:** `set-priority <name> <rank>` command for priority-based resolution. Bare `@mold` references resolved in priority order across all registries. Swap semantics by default; `--cascade` to shift others down.
- **registry:** `build-catalog` now takes a directory path as positional argument. `--registry <name>` resolves from a registered source.
- **registry:** duplicate URL/path detection in `registry add`.
- **registry:** `setup` migrates legacy "official" registry to "examples" (P99) and adds fimod-powered (P10).
- **registry:** catalog TTL cache (60s) — Tab completion and repeated calls skip HTTP when cache is fresh.
- **registry:** companion files support — remote molds with templates/data files are downloaded alongside the main script.
- **pipeline:** `-m` and `-e` can now be mixed and interleaved freely in CLI order.
- **pipeline:** `--input-format http` exposes `data["url"]`, `data["body_size"]`, `data["content_type"]`.
- **format:** CSV output supports array-of-arrays (`[[v1, v2], ...]`) with `--csv-header-names`.
- **install:** SHA-256 checksum verification for downloaded binaries.
- **monty:** upgrade from v0.0.9 to v0.0.11 (json perf, mount fixes, filesystem mounting, multi-module imports).

### Bug Fixes

- **registry:** fix resolution fallthrough when catalog exists but mold not found.
- **registry:** fix fimod-powered registry URL in setup (missing `/tree/main/molds`).
- **test-runner:** `fimod mold test` now resolves mold base directory for `tpl_render_from_mold()`.

### Breaking Changes

- **completions:** `--completions <shell>` flag removed. Use `COMPLETE=<shell> fimod` or `fimod completions <shell>`.
- **registry:** `fimod registry build-catalog <name>` is now `fimod registry build-catalog <path>`. Use `--registry <name>`.
- **registry:** `fimod registry set-default` removed — use `fimod registry set-priority <name> 0`.
- **license:** LGPL-3.0-only → Apache-2.0.
- **deps:** `serde_yaml` replaced by `serde-saphyr` (pure-Rust YAML).

## [0.2.0] — 2026-04-02

### Highlights

- **⬆️ Monty v0.0.9** — `import datetime`, `import json`, named keyword args (`key=`), nested subscript assignment. Date/DateTime/TimeDelta are auto-serialized as ISO 8601 strings in the output.
- **🧩 Jinja2 templating** — new `tpl_render_str` and `tpl_render_from_mold` built-ins for data→text generation via MiniJinja (inline strings or `.j2` files).
- **⚡ Mold cache** — registry molds are cached locally with ETag and content hash validation. No re-download on repeat runs.

### Features

- **monty:** upgrade to Monty v0.0.9 — `datetime`, `json` modules, named keyword args, nested subscript assignment
- **convert:** serialize Date, DateTime, TimeDelta, Timezone as ISO 8601 strings
- **template:** add Jinja2 templating engine (tpl_render_str, tpl_render_from_mold) — data→text generation via MiniJinja, inline strings or .j2 files, path traversal security, badge_md and git_changelog demo molds
- **cache:** add registry mold cache with ETag and content hash validation
- **engine:** pass `args`, `env`, `headers` as keyword arguments — molds only need to declare what they use (`def transform(data, args, **_):`)

### Changed

- **molds:** update all bundled mold signatures to use `**_` kwargs pattern
- **docs:** update monty-engine.md and mold-scripting.md for v0.0.9 capabilities
- **build:** extract MONTY_VERSION from Cargo.toml via build.rs (no more manual sync)
- **install:** migrate 'official' registry to 'examples' on upgrade
- **install:** skip version fetch when `FIMOD_SKIP_DOWNLOAD=1`

### Bug Fixes

- **core:** extract pipeline logic into lib.rs and pipeline.rs

## [0.1.2] — 2026-03-25

### Fixed

- **Registry: FIMOD_REGISTRY visibility** — `fimod registry list` and `fimod mold list` now display entries defined via the `FIMOD_REGISTRY` environment variable

### Changed

- **Installer: piped input support** — `install.sh` reads the registry setup prompt from `/dev/tty` when piped via `curl | sh`
- **Installer: reduced prompts** — Simplified confirmation questions in `install.sh` and `install.ps1`

## [0.1.1] — 2026-03-21

### Fixed

- **Windows: config path crash** — `registry.rs` now falls back to `USERPROFILE` when `HOME` is not set, preventing a panic on first run under Windows ([#3467])
- **Windows: cache path crash** — `mold.rs` applies the same `USERPROFILE` fallback for the mold cache directory ([#3468])

### Changed

- **Installers prompt before registry setup** — `install.sh` and `install.ps1` now ask for confirmation before running `fimod registry setup`, instead of executing it automatically
- **Quick Start guide: Windows installation** — Added two Windows installation options (PowerShell script and ubi-based) with platform-specific tabs and PATH configuration notes
- **README: Windows PATH instructions** — Explicit PowerShell commands to configure PATH when the installer cannot modify it automatically

## [0.1.0] — 2026-03-21

Initial release — fimod, a Rust CLI that transforms structured data files by
executing Python mold scripts via Monty (Pydantic's embedded Python engine).
No system Python installation required.

- Multi-format I/O: JSON, YAML, TOML, CSV/TSV, NDJSON, TXT, Lines
- Inline expressions (`-e`) and mold file (`-m`) execution
- Single pipeline: Read → Parse → Convert → Execute → Serialize → Write
- Intermediate representation: `serde_json::Value` / `MontyObject`

- `dp_get` / `dp_set` — safe nested dotpath access
- `re_search`, `re_match`, `re_findall`, `re_sub`, `re_split` — regex (+ fancy-regex variants)
- `it_keys`, `it_values`, `it_flatten`, `it_group_by`, `it_sort_by`, `it_unique`, `it_unique_by`
- `hs_md5`, `hs_sha256`, `hs_sha1` — hashing
- `gk_fail`, `gk_assert`, `gk_warn` — validation gates with exit code control
- `msg_print`, `msg_info`, `msg_warn`, `msg_error` — stderr logging
- `env_subst` — `${VAR}` template substitution
- `set_exit`, `set_format`, `set_input_format`, `set_output_file` — pipeline control

- HTTP input with raw response envelope (`--input-format http`, `--no-follow`, `--http-header`)
- Binary pass-through via `set_format("raw")` + `set_output_file()`
- Pipeline chaining (multiple `-e` / `-m`), slurp mode (`--slurp`), batch processing
- Mold registry: `~/.config/fimod/sources.toml` + `FIMOD_REGISTRY`, remote catalogs (`@name`, `@source/name`)
- `--check` mode for validation pipelines (exit 0/1 on truthy/falsy result)
- `--no-input` mode for data generation
- `--in-place` rewrite, `--compact` output, `--debug` mode
- Shell completions: bash, zsh, fish, powershell (`--completions <SHELL>`)
- CSV options: delimiter, output-delimiter, header control
- `--arg name=value` and `--env PATTERN` for parameterized molds
- Guides, reference, examples (JSON, YAML, CSV, HTTP) and cookbook — MkDocs Material site
