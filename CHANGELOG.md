# Changelog

All notable changes to fimod are documented here.

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
