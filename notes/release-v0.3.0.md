# fimod v0.3.0

## Highlights

- **🐚 Dynamic shell completions** — context-aware Tab completion for subcommands, flags, format names, `@mold` references, and registry source names. Replaces the static `--completions` flag.
- **📌 Priority-based registry resolution** — registries are searched in priority order (P0→P99) when resolving bare `@mold` references. New `set-priority` command replaces `set-default`.
- **🔀 `-m` and `-e` can be mixed freely** — molds and inline expressions are now interleaved in CLI order instead of being mutually exclusive.
- **📊 CSV array-of-arrays output** — `--output-format csv` now accepts `[[col1, col2], ...]` in addition to the existing `[{col: val}, ...]` format.
- **🔐 SHA-256 download verification** — install scripts verify binary integrity against published checksums.
- **🐍 Monty v0.0.11** — upgraded embedded Python engine: `json.loads()` ~2x faster than CPython (string cache), `json.dumps()` ~1.65x faster (lookup-table escaping), mount fixes, async-in-rust internals.
- **📦 Pure-Rust YAML** — switched from `serde_yaml` (deprecated) to `serde-saphyr`.
- **📄 License change** — LGPL-3.0 → Apache-2.0.

## Features

- **completions:** dynamic shell completions via `clap_complete` `CompleteEnv` — Tab-completes format names, `@mold` references, registry source names. New `fimod completions <shell>` subcommand prints setup instructions. Supports Bash, Zsh, Fish, Elvish, PowerShell.
- **registry:** `set-priority <name> <rank>` command to assign priority ranks (P0, P1, P2, …). Bare `@mold` references are resolved in priority order across all registries. P0 = highest priority (replaces the old "default" concept). Swap semantics by default when both registries already have a rank; `--cascade` to shift others down.
- **registry:** `build-catalog` now takes a directory path as positional argument (`fimod registry build-catalog ./molds`), no registered registry needed. `--registry <name>` resolves the path from a registered source.
- **registry:** duplicate URL/path detection in `registry add` — prevents accidentally registering the same location under two names.
- **registry:** `setup` now migrates legacy "official" registry to "examples" (P99) interactively.
- **registry:** catalog TTL cache (60s) — Tab completion and repeated `mold list` calls skip HTTP entirely when the cached `catalog.toml` is fresh.
- **pipeline:** `-m` and `-e` can now be mixed and interleaved freely in CLI order (previously mutually exclusive).
- **pipeline:** `--input-format http` now exposes `data["url"]` (the requested URL), `data["body_size"]` (response size in bytes), and `data["content_type"]` alongside the existing `status`, `headers`, `body` fields.
- **format:** CSV output supports array-of-arrays (`[[v1, v2], ...]`) — use `--csv-header-names` to provide column headers.
- **install:** SHA-256 checksum verification for downloaded binaries (both `install.sh` and `install.ps1`).
- **install:** registry migration ("official" → "examples") moved from install scripts into `fimod registry setup` (single source of truth).

## Breaking Changes

- **completions:** `--completions <shell>` flag removed. Use `COMPLETE=<shell> fimod` for dynamic completions, or `fimod completions <shell>` for setup instructions. Shell config must be updated (one-time).
- **registry:** `fimod registry build-catalog <name>` is now `fimod registry build-catalog <path>`. Use `--registry <name>` to build from a registered registry.
- **registry:** `fimod registry set-default` removed — use `fimod registry set-priority <name> 0` instead. The `--default` flag on `registry add` is also removed.
- **license:** changed from LGPL-3.0-only to Apache-2.0.

## Bundled Molds

- **added:** `bq_insert`, `bq_select` — BigQuery example molds.
- **moved to [fimod-powered](https://github.com/pytgaen/fimod-powered):** `download`, `gh_latest`, `poetry_migrate`, `skylos_to_gitlab` — install with:
  ```bash
  fimod registry add fimod-powered https://github.com/pytgaen/fimod-powered
  ```

## Fixes

- **test-runner:** `fimod mold test` now resolves the mold's base directory, fixing `tpl_render_from_mold()` failures in molds loaded by the test runner.

## Internal

- **deps:** upgraded Monty from v0.0.9 to v0.0.11 (v0.0.10: filesystem mounting, multi-module imports, async-in-rust; v0.0.11: JSON perf — string cache for `json.loads()`, lookup-table escaping for `json.dumps()`, mount edge-case fixes).
- **deps:** replaced `serde_norway` with `serde-saphyr` (pure-Rust YAML, replaces deprecated `serde_yaml` fork).
- **deps:** `clap_complete` now uses `unstable-dynamic` feature for runtime completions.
- **MSRV:** set to Rust 1.75.
- **project:** renamed `ressources/` → `resources/` (typo fix).
- **docs:** updated README, index, quick-tour, mold-gallery to reflect mold migration to fimod-powered. Expanded mold-gallery to showcase all bundled molds.
- **examples:** removed orphaned Skylos test data (`gl-code-quality.json`, `skylos-report.json`).
- **tests:** removed `test_readme_skylos_to_gitlab` integration test (mold moved to fimod-powered).
- **docs CI:** deploy on version tags + `workflow_dispatch`, not just `main` push.
- **format:** CSV `value_to_field` uses `Cow<str>` to avoid unnecessary string clones.
- **registry:** removed `default` field from `SourcesConfig` — P0 in `[priority]` replaces the old concept. Legacy `default = "name"` in `sources.toml` is auto-migrated to `priority.name = 0` on first load.
- **registry:** extracted `confirm()` helper for interactive prompts (deduplication).
- **registry:** `set_priority` swap branch cleaned up (no more variable shadowing).
- **registry:** catalog TTL cache (60s) — avoids HTTP round-trips when the cached `catalog.toml` is fresh. Refreshes mtime on 304 responses.

## Migration Guide

### Shell completions

Replace your existing completion config:

```bash
# Before (v0.2.0)
fimod --completions bash > ~/.local/share/bash-completion/completions/fimod

# After (v0.3.0)
echo 'source <(COMPLETE=bash fimod)' >> ~/.bashrc
# or for Zsh:
echo 'source <(COMPLETE=zsh fimod)' >> ~/.zshrc
```

Run `fimod completions <shell>` to see the exact instruction for your shell.

### Registry `build-catalog`

```bash
# Before
fimod registry build-catalog my-registry

# After
fimod registry build-catalog ./path/to/molds       # direct path
fimod registry build-catalog --registry my-registry # from registered name
```

### `set-default` → `set-priority`

```bash
# Before
fimod registry set-default corp
fimod registry add corp https://... --default

# After
fimod registry set-priority corp 0
fimod registry add corp https://...
fimod registry set-priority corp 0   # separate step
```

The `default` field in `sources.toml` is automatically migrated to `priority.name = 0` on first load.

### "official" → "examples" registry

Run `fimod registry setup` — it will detect the old "official" registry and offer to migrate it to "examples" (P99). The install scripts no longer handle this migration.

## Commit message

```
feat: monty v0.0.11, migrate molds to fimod-powered registry

- Dynamic shell completions (clap_complete), CSV array-of-arrays output, serde_yaml → serde-saphyr, SHA-256 install verification, mixed -m/-e pipeline, registry priorities with set-priority command.
- Upgrade Monty from v0.0.9 to v0.0.11 (v0.0.10: mount filesystem, multi-module imports, async-in-rust; v0.0.11: json.loads ~2x faster via string cache, json.dumps ~1.65x faster via lookup-table escaping but not used by fimod).
- Move production molds (download, gh_latest, poetry_migrate, skylos_to_gitlab) to the fimod-powered registry. Update docs, examples, and catalog.toml to reference the external registry.
- Add companion files support for remote molds with templates/data files.
- Fix registry resolution fallthrough when catalog exists but mold not found.
- Fix fimod-powered registry URL in setup.
- Fix fimod mold test not resolving mold base directory for tpl_render_from_mold().
```
