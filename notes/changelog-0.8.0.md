## [0.8.0] — YYYY-MM-DD

### Highlights

- 🚀 **Installable `fimod-fast`** — releases now publish `fimod`, `fimod-slim`, and `fimod-fast` as first-class assets. Installers keep variants explicit by default, with `FIMOD_SET_DEFAULT=yes` for users who want slim or fast to replace the canonical `fimod` command.
- ⚡ **Faster compatible outputs** — `json-compact`, `ndjson`, `lines`, and `txt` can serialize directly from `MontyObject` at the pipeline exit when no conversion-requiring override is active.
- 🧭 **Cleaner mold/chain contract** — inline `-e` wrappers now follow `transform(data, **_)`, and mixed `-m` / `-e` chains use Clap argument positions instead of manual argv scanning.
- 📚 **Runtime contract clarified** — public docs now say upfront that molds run on Monty, not CPython: Python-shaped syntax and common builtins, selected stdlib support, no PyPI ecosystem, and Rust-powered helpers as part of fimod's API.

### Features

- **distribution:** publish standard, slim, and fast release assets across the stable release matrix. `fimod-fast` is distributed without UPX compression so the speed-oriented build does not trade runtime performance for size.
- **install:** `install.sh` and `install.ps1` accept `FIMOD_VARIANT=standard|slim|fast`. Standard installs `fimod`; slim and fast install `fimod-slim` / `fimod-fast` unless the user explicitly opts into replacing `fimod`.
- **cli:** include the active variant in `--version` (`standard`, `slim`, or `fast`) so downloaded binaries and local builds are diagnosable.

### Bug Fixes

- **mold:** generate inline expression wrappers with `**_` so `fimod -e ...` accepts the same keyword-context contract as reusable molds (`args`, `env`, `headers`, `pipeline`).
- **shape:** derive the mixed `-m` / `-e` chain from Clap's parsed argument indices. Attached short values (`-mfoo`, `-eexpr`) and interleaved non-step flags keep the intended execution order.
- **watch:** replace `notify-debouncer-mini` with raw `notify` events plus one explicit quiet-window debounce. Reads caused by fimod itself no longer retrigger watch mode.

### Performance

- **pipeline:** serialize `MontyObject` directly for `json-compact`, `ndjson`, `lines`, and `txt` outputs when compatible. Avoids the final `MontyObject` → `serde_json::Value` allocation on these hot paths.
- **build:** add `release-fast` and `fimod-fast` for speed-optimized local/release builds (`opt-level = 3`), separate from the size-optimized standard/slim binaries.
- **test:** add opt-in performance smoke tests plus `notes/perf-baseline.md` to keep startup/format regressions visible without making normal CI noisy.

### Documentation

- **mold contract:** standardize examples around `transform(data, ..., **_)` and keyword-only context parameters.
- **runtime:** clarify Monty vs CPython in README and docs site: familiar Python syntax/common builtins, selected stdlib support, no PyPI ecosystem, no full stdlib parity.
- **helpers:** frame Rust-powered helpers as part of fimod's data-shaping API, not as compensation for missing Python features.
- **distribution:** document `standard` / `slim` / `fast` install behavior, `FIMOD_VARIANT`, `FIMOD_SET_DEFAULT`, and the `fimod-fast` speed/size tradeoff.
- **http:** update standard binary size and TLS tradeoff notes after moving to the reqwest 0.13 TLS stack.

### Housekeeping

- **deps:** upgrade `reqwest` to 0.13 with the upstream rustls / AWS-LC TLS stack.
- **deps:** update compatible Cargo dependencies.
- **notes:** refresh internal synthesis, performance baselines, and the 0.8.0 planning note.
- **gitignore:** update local workflow and notes ignores.
