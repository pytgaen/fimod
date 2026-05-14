# Fimod — Code Layout

Where every file lives and what it owns. Companion to `ARCHITECTURE.md` (how the layers fit together logically) and `DESIGN_NOTES.md` (why we made those decisions).

**Target reader**: a contributor (human or AI) opening the repo for the first time and asking "where do I put this change?". Read this front-to-back once, then keep it open as a map.

---

## Top-level

```
fimod/
├── Cargo.toml            crate manifest + features (watch, http)
├── Cargo.lock            checked in (binary crate)
├── build.rs              extracts Monty git tag → MONTY_VERSION env at build time
├── rust-toolchain.toml   pinned toolchain
├── mise.toml             local tool versions (cargo-nextest, cargo-deny, etc.)
├── Taskfile.yml          high-level commands (task lint / test / build:release / outdated …)
├── deny.toml             cargo-deny config (licenses, advisories, bans)
├── mkdocs.yml            Zensical (mkdocs-material flavor) config for docs site
├── CHANGELOG.md          release history, only touched in `chore(release): X.Y.Z` commits
├── CLAUDE.md             AI-assistant guidance (read by Claude Code on every session)
├── README.md             public landing (crates.io + GitHub)
├── LICENSE.txt           Apache-2.0
├── install.sh / .ps1     curl-piped installers (POSIX + PowerShell)
├── Dockerfile            CI image and reproducible builds
├── .gitlab-ci.yml        GitLab mirror pipeline (parallel to GitHub Actions)
├── .pre-commit-config    optional local hooks (fmt, clippy)
├── .markdownlint.json    markdown lint config
└── .dockerignore         shrinks the Docker build context
```

Top-level dirs:

```
src/             Rust crate (binary + library)
tests/           integration tests (CLI black-box)
tests-molds/     mold fixture tests (declarative input/expected pairs)
docs/            user-facing documentation (rendered by Zensical)
notes/           internal source of truth — VISION, ARCHITECTURE, design, transient cycle notes
scripts/         release helpers (prerelease-local.sh, prerelease-github.sh)
molds/           the public mold registry catalog source (catalog.toml + one dir per mold)
examples/        usage examples shipped alongside the docs (e.g. jq compat shims)
resources/       binary assets (logo, vhs cassettes for demo gifs)
.github/         CI workflows, issue/PR templates, dependabot config
.claude/         per-project Claude Code config (skills, settings)
target/          cargo build cache (gitignored)
dist/            release artifact staging (gitignored)
site/            Zensical build output (gitignored)
tmp/             scratch dir (gitignored)
```

Other dot-dirs (`.agents/`, `.cache/`, `.config/`, `.memsearch/`, `.rtk/`, `.skylos/`, `.ruff_cache/`, `.astx/`, `.vscode/`) are tooling caches — ignorable unless you're debugging the tool itself.

---

## `src/` — the crate

`src/lib.rs` declares every public module so the crate exposes a stable library API alongside the `fimod` binary. `src/main.rs` is the binary entry point and consumes the library through `use fimod::…`.

### Top-level files

```
src/
├── main.rs            binary entry: clap parse → dispatch() → cmd::*
├── lib.rs             module declarations (pub mod …) + MONTY_VERSION const
├── cli.rs             clap definitions: Cli, Commands, ShapeArgs, all subcommand enums,
│                      completion candidate helpers
│
├── cmd/               subcommand handlers — one file per top-level subcommand
├── registry/          registry submodules — split from the former monolithic registry.rs
│
├── pipeline.rs        orchestration: read → parse → execute_chain → serialize → write.
│                      `run_pipeline_core` is the single source of truth.
├── engine.rs          Monty execution loop (start / FunctionCall / OsCall / NameLookup),
│                      external-function dispatch, sandbox enforcement
├── mold.rs            script resolution: MoldSource (File / Url / Inline),
│                      MoldDefaults (docstring → docs, `# fimod:` directives), URL cache
├── format.rs          DataFormat enum, parse/serialize for every format
│                      (JSON, YAML, TOML, CSV, NDJSON, TXT, Lines, Raw, HTTP)
├── http.rs            fetch_url via reqwest (feature-gated `http`)
├── convert.rs         serde_json::Value ↔ MontyObject conversion (the I/O ↔ engine bridge)
├── serde_compat.rs    NativeNumbers wrapper — preserves i64 vs f64 through non-serde_json
│                      serializers (TOML, YAML lose this otherwise)
├── sandbox.rs         sandbox.toml schema and runtime enforcement (deny-list-driven OsCall)
├── paths.rs           XDG-style path helpers ($FIMOD_HOME, config dir, cache dir)
├── monty_args.rs      tiny helper: build the kwargs dict passed to transform(…)
├── test_runner.rs     `fimod mold test` — discovers *.input.* / *.expected.* pairs,
│                      runs the pipeline, diffs the output
└── watch.rs           watch mode (feature-gated `watch`) — debounced rerun on file change
```

### Built-ins — external functions callable from molds

Each file exports `EXTERNAL_FUNCTIONS: &[&str]` (the list of names Monty should resolve as functions) and `dispatch(name, args, …)` (the actual implementation). Adding a helper means extending both, then wiring it into `engine::is_external_function` + `engine::dispatch_external`.

```
src/
├── regex.rs           re_search, re_match, re_findall, re_sub, re_sub_fancy, re_split
├── dotpath.rs         dp_get, dp_set, dp_has, dp_delete, dp_pluck (jq-lite path access)
├── iter_helpers.rs    it_group_by, it_count_by, it_sort_by, it_unique_by, it_take, …
├── hash.rs            hs_md5, hs_sha256, hs_sha512, hs_blake3
├── gatekeeper.rs      gk_fail, gk_assert (mold-level invariants → non-zero exit)
├── msg.rs             msg_info, msg_warn, msg_error — structured stderr logging
├── template.rs        tpl_render_str (minijinja, sandboxed)
├── env_helpers.rs     env_subst — expand ${VAR} against the filtered env passed via --env
├── exit_control.rs    set_exit — let a mold request a custom process exit code
└── format_control.rs  set_input_format, set_output_format, set_output_file — runtime
                       overrides applied between chained steps or before serialization
```

### `src/cmd/` — subcommand handlers

One module per top-level subcommand. The pattern is: each module exposes a thin `dispatch(action)` (or a single `run_*` for `shape`), keeps the business logic concentrated, and is the only place `main.rs` calls into.

```
src/cmd/
├── mod.rs           pub mod completions / mold / monty / registry / setup / shape
├── shape.rs         fimod s / shape — the main pipeline entry point.
│                    Owns run_shape + run_shape_pipeline (~570 lines).
│                    Translates ShapeArgs → pipeline config, handles --watch, --chain, --in-place.
├── registry.rs      façade for `fimod registry <action>` — dispatches list / add / show /
│                    remove / set-priority / build-catalog into `registry::*`
├── mold.rs          façade for `fimod mold <action>` — list / show / test
│                    (`test` is special: returns CliResult::Exit, handled in main::dispatch)
├── monty.rs         `fimod monty repl` — REPL entry, repl_feed helper
├── setup.rs         `fimod setup <category>` — registry_defaults, sandbox_defaults,
│                    all_defaults wizards (absorbed the former registry::setup + src/setup.rs)
└── completions.rs   `fimod setup completions <shell>` — emits the static activation snippet;
                     dynamic completion is handled separately via clap_complete CompleteEnv
```

### `src/registry/` — registry submodules

Was previously a 2180-line `src/registry.rs`. Split into 4 submodules + a thin `mod.rs` that re-exports the public API so external callers (`use fimod::registry::resolve`) stay unchanged.

```
src/registry/
├── mod.rs        pub mod {catalog, config, molds, resolve}
│                 + pub use re-exports for the stable public surface
├── config.rs     Source, SourceType, SourcesConfig — `sources.toml` CRUD.
│                 add / remove / list / show / set_priority / confirm.
├── resolve.rs    `@name` and `@source/name` resolution.
│                 resolve(), resolve_local, resolve_remote, token_for_url
│                 (auth headers via FIMOD_TOKEN_<SOURCE> env).
├── catalog.rs    Catalog, fetch_catalog, build_catalog (scan local sources →
│                 catalog.toml), URL/raw cache, compute_mold_hash,
│                 github_to_raw shim. Owns cache_base_dir + cache_clear/info.
└── molds.rs      `fimod mold list/show` query layer (local scan + remote catalog merge),
                  MoldMatch ranking, completion candidates
                  (complete_mold_names, complete_source_names).
```

Visibility convention post-split: anything that used to be `pub` in the monolithic file but is only consumed inside `registry/*` is now `pub(crate)`. The `pub` items in `mod.rs` are the contract with the rest of the crate.

---

## `tests/` — integration tests

CLI black-box tests run the binary via `assert_cmd`. One file per topic, declared in `tests/cli.rs`.

```
tests/
├── cli.rs           integration test entrypoint — `mod` declarations for every cli/* file
├── molds_test.rs    runs the mold fixture suite under `tests-molds/` (uses test_runner)
├── cli/             one file per topic; recurring helpers live in `cli/helpers.rs`
│   ├── args.rs, batch.rs, chain.rs, cookbook.rs, cross_format.rs, csv.rs,
│   ├── dotpath.rs, env.rs, env_subst.rs, errors.rs, exit_control.rs,
│   ├── fimod_registry_env.rs, gatekeeper.rs, hash.rs, helpers.rs,
│   ├── http_e2e.rs, inline.rs, iter_helpers.rs, json.rs, lines.rs,
│   ├── mold_contract.rs, msg.rs, multi_slurp.rs, ndjson.rs, output_file.rs,
│   ├── pipeline.rs, readme.rs, regex.rs, registry.rs, sandbox.rs,
│   ├── setup.rs, template.rs, txt.rs, watch.rs
│   └── …
└── data/            shared fixtures (input files, expected outputs) consumed by cli/*
```

Convention: an integration test for built-in `X` lives in `tests/cli/X.rs` and references `tests/data/` for non-trivial inputs. Unit tests live next to the code (`#[cfg(test)] mod tests` in `format.rs`, etc.).

---

## `tests-molds/` — mold fixture tests

Declarative tests for the molds shipped in `molds/`. Each directory is one mold; each mold has `*.input.<format>` + `*.expected.<format>` pairs (and optional `*.run-test.toml` to pass CLI args). Run with `cargo test --test molds_test` or `fimod mold test <dir>`.

```
tests-molds/
├── anonymize_pii/, auto_anonymize/, badge_md/, checkpoint/, compact_if_big/,
├── csv_stats/, csv_to_json_records/, dedup_by/, deep_pluck/, env_to_dotenv/,
├── filter_fields/, flatten_nested/, gh_latest/, git_changelog/, group_count/,
├── jq_compat/, json_schema_extract/, log_parse/, markdown_toc/, pick_fields/,
├── poetry_migrate/, rename_keys/, sample_if_large/, skylos_to_gitlab/,
├── sort_json_keys/, split_tags/, validate_fields/, with_threshold/, yaml_merge/
└── …
```

See the `mold-tests` skill (`~/.claude/skills/mold-tests/`) for fixture format details.

---

## `docs/` — user-facing documentation

Rendered by Zensical (mkdocs-material flavor) via `uvx zensical serve` / `build`. Config in `mkdocs.yml`.

```
docs/
├── index.md                landing page
├── cookbook.md             recipe collection (paired with tests/cli/cookbook.rs)
├── mold-gallery.md         showcase of the registry molds
├── acknowledgements.md     third-party credits
├── googlecd5874361c77eec0.html   Search Console verification (do not delete)
├── examples/               format-specific worked examples
│   └── csv.md, http.md, json.md, yaml.md
├── guides/                 narrative how-to docs
│   └── ai-integration.md, authoring-molds.md, cli-reference.md, comparison.md,
│       concepts.md, dynamic-molds.md, mold-scripting.md, quick-start.md, quick-tour.md
├── reference/              factual reference, alphabetized
│   └── built-ins.md, exit-codes.md, formats.md, mold-defaults.md, monty-engine.md
└── assets/                 images, demo gifs, sketchnote
```

Rule: when a roadmap item ships, its content migrates from `notes/ROADMAP.md` to the appropriate `docs/reference/*.md` or `docs/guides/*.md`. Don't leave shipped features documented only in `notes/`.

---

## `notes/` — internal source of truth

Read these before any decision on architecture, tooling, or release flow.

```
notes/
├── VISION.md                  long-term direction, what we refuse to build
├── ARCHITECTURE.md            module map, layer responsibilities, end-to-end flow
├── DESIGN_NOTES.md            concrete design decisions + tooling conventions
├── CODE_LAYOUT.md             this file — where every file lives
├── release-workflow.md        release process detail (skill = .claude/skills/release-workflow)
├── todo-X.Y.Z.md              transient backlog for the current cycle
├── changelog-X.Y.Z.md         drafted changelog for the next release (transient)
└── assets/                    diagrams / sketches consumed by the docs above
```

`todo-*.md` and `changelog-*.md` are **transient**: they live for one release cycle and are removed in the `chore(release): X.Y.Z` commit once their content has migrated (todo → user docs, changelog → CHANGELOG.md).

---

## `scripts/`

```
scripts/
├── prerelease-local.sh    spin up an Incus container, install fimod from the current source,
│                          run the smoke matrix (install / migrations / custom registry /
│                          idempotency / version check). Invoked by /prerelease-workflow local.
└── prerelease-github.sh   trigger the prerelease.yml workflow on GitHub Actions.
                           Invoked by /prerelease-workflow github.
```

Never commit `prerelease-local.sh` modifications inside a feature branch — it's a release-engineering script with its own change cadence.

---

## `.github/`

```
.github/
├── workflows/
│   ├── ci.yml              fmt + clippy + tests on push (Linux + macOS)
│   ├── docs.yml            build Zensical site, deploy to GitHub Pages
│   ├── prerelease.yml      build rc.N artifacts on tag push
│   └── release.yml         build release artifacts + GitHub Release on `v*` tag
├── ISSUE_TEMPLATE/         bug / feature templates
├── PULL_REQUEST_TEMPLATE.md
├── CONTRIBUTING.md         contributor guide (paired with notes/ for AI agents)
└── dependabot.yml          weekly bumps for Cargo + GitHub Actions
```

---

## `molds/` — the public mold registry source

Each directory is one mold (`<name>/<name>.py` + optional `<name>.md`). `catalog.toml` is the index — generated by `fimod registry build-catalog` and committed.

```
molds/
├── catalog.toml         generated; one [[mold]] entry per directory
├── anonymize_pii/, auto_anonymize/, bq_insert/, bq_select/, … (~30 molds)
└── .ruff.toml           shared ruff config for the Python molds
```

When a mold is added or modified, regenerate `catalog.toml` before committing.

---

## `examples/` and `resources/`

```
examples/
├── jq_filter.py, jq_map.py     reference shims showing how to write a mold that emulates jq
└── README.md                   short index

resources/
├── logo/                       PNG/SVG sources for README and docs
└── vhs/                        .tape cassettes for asciinema-style demo gifs (vhs render)
```

---

## "Where do I put this change?" — quick decisional

| Change | Primary file(s) | Tests |
|---|---|---|
| New built-in helper (e.g. `df_diff`) | New `src/<family>.rs` exporting `EXTERNAL_FUNCTIONS` + `dispatch`; wire into `engine.rs::is_external_function` + `dispatch_external` | `tests/cli/<family>.rs` + `tests-molds/<demo>/` |
| New data format | New variant in `format::DataFormat` + `parse` + `serialize` + extension mapping | `format.rs` unit tests + `tests/cli/<format>.rs` |
| New subcommand | New variant in `cli::Commands` + handler in `src/cmd/<name>.rs` + wire into `main::dispatch` / `dispatch_other` | `tests/cli/<name>.rs` |
| New `fimod mold` action | Extend `cli::MoldAction` + handler in `cmd/mold.rs` (or split into a submodule if it grows) | `tests/cli/mold_*.rs` |
| New registry backend (e.g. Bitbucket) | New variant in `registry::config::SourceType` + resolver function in `registry::resolve` | `tests/cli/registry.rs` + `tests/cli/fimod_registry_env.rs` |
| Mold defaults directive | Extend `MoldDefaults` struct + `parse_mold_defaults` in `mold.rs` | `tests/cli/mold_contract.rs` |
| Sandbox capability | New schema field in `sandbox.rs` + enforcement branch in `engine.rs` `OsCall` | `tests/cli/sandbox.rs` |
| New CLI argument | `cli::ShapeArgs` (or relevant subcommand args struct) + threading through `cmd/shape.rs` → `pipeline.rs` | `tests/cli/args.rs` or the topic-specific file |
| Watch mode behavior | `src/watch.rs` (feature-gated) + `cmd/shape.rs` watch branch | `tests/cli/watch.rs` |
| Public mold | New directory under `molds/<name>/`; regenerate `catalog.toml` with `fimod registry build-catalog` | `tests-molds/<name>/` fixtures |
| Release process | `notes/release-workflow.md` + `.claude/skills/release-workflow/SKILL.md` | manual via `/prerelease-workflow` |

---

## See also

- `notes/ARCHITECTURE.md` — how the layers fit together (mermaid module map, end-to-end flow)
- `notes/DESIGN_NOTES.md` — *why* we drew the boundaries the way we did
- `notes/VISION.md` — what we refuse to build
- `CLAUDE.md` (root) — AI-assistant guardrails, build/test commands, code style
- `docs/guides/concepts.md` — user-facing mental model
