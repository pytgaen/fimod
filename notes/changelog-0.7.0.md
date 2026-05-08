## [0.7.0] — YYYY-MM-DD

### Highlights

- 👁️ **Watch mode** — `fimod shape --watch <input>` re-runs the pipeline on every file change. 150 ms debounce, parent-dir watching (atomic-rename safe), errors don't kill the loop. Watches both `-i` and local `-m` paths.
- 🐞 **Debug phases & step labels** — `--debug` prints per-phase timings (`parse: 0.045s`, `execute: 0.123s`, …) and identifies each step in chains (`step 2/4 (label, injected by step 1)`).
- ⚙️ **CLI setup restructure** *(BREAKING)* — `fimod setup completions --shell <shell>` replaces `fimod completions <shell>`. Prints the script directly to stdout for `eval "$(fimod setup completions --shell zsh)"`.
- 🚀 **Performance** — regexes cached per pattern (process-wide); `dp_get` / `dp_has` walk `MontyObject` directly without JSON conversion, fixing O(N²) cost in 100 k-row mold loops.
- 🔧 **Changelog process change** — `cliff.toml` removed; CHANGELOG.md is hand-curated from `notes/changelog-X.Y.Z.md` per release, and the GitHub release workflow extracts its body straight from the matching section.

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

### Refactoring

- **format:** new `InputMode` / `OutputMode` enums replace stringly-typed dispatch on `"raw"` / `"http"` at 7 sites. `set_input_format("raw")` and `set_output_format("http")` now fail fast at the call site instead of bailing later in serialize.
- **cli:** new `CliResult { Done, Exit(i32) }` replaces 7 scattered `process::exit()` calls. `pipeline.rs`, `test_runner.rs`, multi-slurp and `--check` paths are now side-effect-free; only `main()` terminates the process. `#[must_use]` makes silent drops a compile error.
- **shape:** `run_shape_pipeline` extracted from `run_shape` (~290 lines). Required for watch mode. Pure refactor.
- **pipeline:** `process_single_input` 16 positional args bundled into `SingleRunOptions<'_>` (borrows, no extra clones). `build_context_base` extracted so the seven-key context JSON contract lives in one place. Four `unwrap()` calls replaced with `.expect("invariant: …")`.
- **mold/runtime:** new `MoldSource` enum (label, base dir, reload semantics) and `StepOrigin` enum (CLI vs runtime-injected). Backbone for per-step debug labels.
- **reuse:** consolidate helpers — `http::is_url` (3 sites), `paths::sha256_hex` (replaces 7 copies), `monty_args::expect_string` (4 sites), `http::send_get` (factor request scaffold), `sandbox::matches_glob` reused by `env_pattern_matches`, three resolver fns unified into `resolve_remote`.
- **post-review cleanup:** `MoldSource::file()` / `is_local_path()` / `label()` (truncates to ~60 chars), `pipeline::debug_phase()` helper, `run_shape_pipeline` derives flags from `shape`, watch single-line status, watch loop pre-filters by `file_name()` before `canonicalize()`.

### Housekeeping

- **Release process:** `cliff.toml` and the `Generate changelog` job in `.github/workflows/release.yml` removed. CHANGELOG.md is now hand-curated from `notes/changelog-X.Y.Z.md` (deleted on release); the workflow extracts the body from the matching `## [X.Y.Z]` section via awk.
- Archived `notes/fix-0.6.0.md`, `notes/plan-0.6.0.md`, `notes/resume-0.6.0.md`.
- `[lint]` header removed from `molds/.ruff.toml`.

---

**BREAKING CHANGE**: `fimod completions <shell>` is removed. Use `fimod setup completions --shell <shell>`. Legacy `source <(COMPLETE=zsh fimod)` still works.
