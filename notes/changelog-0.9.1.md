## [0.9.1] — YYYY-MM-DD

### Highlights

- 🛡️ **Bounded, exact pipelines** — dynamic chains now share one runtime budget, recursive step injection is capped, and JSON integers stay exact across the Rust/Monty boundary through the full `u64` range.
- ⚡ **Bounded-memory identity conversions** — NDJSON now streams directly to pretty or compact JSON without materializing the complete document.

### Bug Fixes

- **pipeline:** apply `max_duration` to the complete chain, including dynamically injected steps, instead of resetting the timer for every Monty execution.
- **pipeline:** stop recursive `pipeline.insert_next()` / `pipeline.append()` growth after 1,024 injected steps with an explicit error.
- **json:** preserve integers from `i64::MIN` through `u64::MAX` as numeric JSON values when converting to and from Monty, including direct serializers.
- **shape:** allow multi-input raw pass-through with `--output-format raw -O`, using each input filename as the destination.
- **shape:** install streamed file outputs only after successful conversion, preserving existing files, Unix permissions, symbolic-link targets, and hard-linked inputs.
- **sandbox:** reject overflowing minute and hour duration values instead of panicking or wrapping.
- **http:** detect MIME types case-insensitively in parsed and structured HTTP inputs, and keep unknown `text/*` response bodies textual.
- **http:** reject HTTP error statuses before reading potentially large response bodies.
- **library:** return an explicit error when `run_pipeline()` receives a configuration with no mold or expression step.

### Security

- **install:** require a checksum manifest, exactly one checksum for the selected asset, and a matching SHA-256 before extracting POSIX or Windows downloads.
- **runtime:** bound the process-wide regex, template, and HTTP client caches with fixed-capacity LRU eviction.

### Performance

- **ndjson:** stream identity NDJSON → JSON/JSON compact from files or stdin while preserving empty-line behavior and exact JSON integers.
- **benchmarks:** cover identity JSON → NDJSON/CSV and NDJSON → JSON in both standard and `release-fast` profiles.

### Documentation

- **security:** align the sandbox wording with the local guardrail threat model and remove the unsupported binary-signature claim.
- **comparison:** correct yq CSV/TOML and jq/yq exit-status capabilities, and describe fancy-regex without presenting it as PCRE2.
- **runtime:** document the chain-wide duration budget, dynamic-step ceiling, and exact JSON integer bridge range.
- **install:** replace the stale pinned `FIMOD_VERSION=0.1.0` example with an explicit `X.Y.Z` placeholder.
- **platforms:** state that prebuilt macOS archives target Apple Silicon and that the pre-1.0 Rust API remains experimental.

### Testing

- **ci:** add fail-closed installer fixtures and a Windows job that exercises the PowerShell installer plus a real binary transformation.

### Housekeeping

- **deps:** upgrade `anyhow`, `clap`, `clap_complete`, `rustyline`, `serde`, `serde_json`, `serde-saphyr`, `toml`, and the transitive `crossbeam-epoch`, restoring green freshness and advisory gates.
