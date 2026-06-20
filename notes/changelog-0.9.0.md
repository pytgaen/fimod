## [0.9.0] — 2026-06-20

### Highlights

- 🧩 **Typed mold args** — molds can declare typed `# fimod: arg=` metadata for `str`, `int`, `float`, `bool`, and `json` args. Fimod validates and casts `--arg` / step args before calling `transform()`, while undeclared args keep the old string behavior.
- ⚡ **Native identity conversions** — exact `-e data` conversions can bypass Monty and stay in Rust for plain format changes. Top-level JSON arrays now stream directly to NDJSON and CSV instead of materializing the full array when the selected CSV mode allows it.
- 📊 **JSON/NDJSON to CSV control** — `--csv-header` now also acts as an explicit object-row output projection, and `--csv-scan <N>` controls how many object rows define discovered columns (`1` by default, `0` for all rows).
- 🛡️ **Monty v0.0.18 hardening** — fimod now targets Monty's typed OS-call API, keeps file access sandboxed through `PermissionError`, and raises the Rust baseline to 1.95.

### Features

- **mold args:** support typed `# fimod: arg=<name>:<type>` directives with optional markers (`?`) and defaults for optional typed args. Supported V1 types are `str`, `int`, `float`, `bool`, and `json`.
- **engine:** cast mold args after CLI args and runtime step args are merged, before Monty execution. Step args still override CLI args, and undeclared args remain strings for compatibility.
- **registry:** include typed arg metadata in catalog generation and mold listing/browsing surfaces so shared molds can publish their expected runtime contract.
- **sandbox:** raise the default mold runtime limit to 10 minutes.
- **sandbox:** raise the default mold memory limit to 2GB.
- **setup:** add sandbox policy management commands (`defaults --preset`, `show`, `get`, `set`) for the canonical policy file or an explicit `--sandbox-file`.
- **monty:** apply sandbox policy resolution to `fimod monty repl`, including `--sandbox-file <path>` and `--sandbox-file=""`.
- **shape:** route exact identity transforms (`-e data`) through native Rust parse/serialize when no incompatible option is active.
- **identity:** stream top-level JSON/JSON-compact arrays to NDJSON from local files and stdin in the native identity path.
- **identity:** extend the same native top-level JSON array path to CSV output.
- **csv:** let `--csv-header` project object-row CSV output columns, not only describe CSV input columns.
- **csv:** add `--csv-scan <N>` for object-row output column discovery. The default `1` preserves first-row column discovery; `0` scans all rows for a full column union.

### Bug Fixes

- **sandbox:** after the Monty v0.0.18 upgrade, `open()` is treated as a sandbox-controlled OS call and denied with `PermissionError` instead of surfacing as a missing builtin.
- **pipeline:** keep native identity conversion guarded behind the exact safe cases, so debug/check/slurp/no-input/HTTP/in-place and non-array fallbacks still use the regular pipeline behavior.
- **install:** on POSIX, `FIMOD_SET_DEFAULT=yes` now makes `fimod` a relative symlink to `fimod-fast` / `fimod-slim` when supported, with a copy fallback for filesystems that reject symlinks.

### Performance

- **identity:** avoid Monty startup and Python-object conversion for exact `-e data` format conversions that can be handled natively.
- **ndjson:** write streamed top-level JSON array elements as NDJSON records without loading the whole array.
- **csv:** stream JSON array rows to CSV with O(1) row buffering for explicit headers and default/windowed scans; `--csv-scan 0` intentionally buffers rows to compute the full column union.

### Documentation

- **mold scripting:** document typed arg directives, supported types, optional/default syntax, and runtime validation behavior.
- **mold defaults:** document typed arg metadata in the reference contract.
- **formats:** document native identity conversions, JSON array to NDJSON streaming, and JSON/NDJSON to CSV projection/scan behavior.
- **monty engine:** update the runtime and sandbox docs for Monty v0.0.18, typed `OsFunctionCall`, and sandbox-controlled `open()`.
- **cli reference:** document sandbox setup presets and policy editing commands.
- **install:** clarify that `curl | sh` installer variables must be placed on the `sh` side of the pipe, and that `FIMOD_SET_DEFAULT` does not answer registry/sandbox setup prompts.
- **notes:** add implementation notes for typed args, Monty v0.0.18 impact, and JSON array to CSV streaming.

### Testing

- **typed args:** add CLI and mold-contract coverage for typed parsing, defaults, validation failures, runtime step args, and registry/listing metadata.
- **identity:** add CLI coverage for native identity conversion, stdin/file input, output-extension detection, NDJSON streaming, non-array fallback, and guarded fallback cases.
- **csv:** add unit and CLI coverage for explicit output headers, default first-row columns, scan windows, full union scans, missing values, empty arrays, and JSON value stringification.
- **setup:** cover sandbox presets, explicit sandbox files, `show`, `get`, `set`, and invalid sandbox configuration values.
- **monty:** add REPL coverage for `--sandbox-file`, zero-authorization clock denial, and policy-enabled clock access.
- **sandbox:** update sandbox coverage for Monty v0.0.18 `open()` denial behavior.

### Housekeeping

- **deps:** upgrade Monty to v0.0.18 and refresh compatible Cargo dependencies.
- **deps:** patch `jiter` to the upstream commit that moves optional Python bindings to `pyo3 0.29.0`, clearing the `RUSTSEC-2026-0176` / `RUSTSEC-2026-0177` `cargo audit` findings until the next `jiter` crate release.
- **build:** raise the Rust MSRV and CI check to 1.95 for the Monty v0.0.18 baseline.
- **tooling:** add `cargo-audit` to mise-managed tools.
- **notes:** remove stale 0.8.0 planning/synthesis notes after migrating the useful content.
