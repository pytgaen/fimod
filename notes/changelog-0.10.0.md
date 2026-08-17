## [0.10.0] — YYYY-MM-DD

### Highlights

- 🐍 **Python standard library in molds** — Monty v0.0.21 brings `collections` (`Counter`, `defaultdict`, `deque`, `namedtuple`), `itertools` (`count`, `repeat`, `pairwise`, `compress`, `islice`, `chain`, `cycle`), `dataclasses` with a mold-defined `@dataclass`, and function decorators. Plain classes work too — they already did, despite what the docs claimed.
- 🛡️ **Bounded, exact pipelines** — dynamic chains now share one runtime budget, recursive step injection is capped, and JSON integers stay exact across the Rust/Monty boundary through the full `u64` range.
- ⚡ **Bounded-memory identity conversions** — NDJSON now streams directly to pretty or compact JSON without materializing the complete document.

### Features

- **monty:** upgrade the embedded engine to v0.0.21, making `collections`, `itertools`, and `dataclasses` importable from molds alongside function decorators and user-defined classes.

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
- **sandbox:** keep `max_memory` enforced across the Monty upgrade. v0.0.20 moved memory accounting out of the interpreter and into globals that only a charging allocator writes, which would have made the limit decorative under fimod's mimalloc — a mold accumulating 3M dicts under a 50MB cap was measured reaching 1.7GB. Fimod now charges those counters around mimalloc itself rather than adopting `monty-alloc`, which forwards to the system allocator at roughly 20% cost per invocation.

### Performance

- **ndjson:** stream identity NDJSON → JSON/JSON compact from files or stdin while preserving empty-line behavior and exact JSON integers.
- **benchmarks:** cover identity JSON → NDJSON/CSV and NDJSON → JSON in both standard and `release-fast` profiles.

### Documentation

- **security:** align the sandbox wording with the local guardrail threat model and remove the unsupported binary-signature claim.
- **comparison:** correct yq CSV/TOML and jq/yq exit-status capabilities, and describe fancy-regex without presenting it as PCRE2.
- **runtime:** document the chain-wide duration budget, dynamic-step ceiling, and exact JSON integer bridge range.
- **monty engine:** update the runtime reference for v0.0.21, including the newly importable standard-library modules and the allocator-backed memory limit.
- **install:** replace the stale pinned `FIMOD_VERSION=0.1.0` example with an explicit `X.Y.Z` placeholder.
- **platforms:** state that prebuilt macOS archives target Apple Silicon and that the pre-1.0 Rust API remains experimental.

### Testing

- **ci:** add fail-closed installer fixtures and a Windows job that exercises the PowerShell installer plus a real binary transformation.
- **sandbox:** cover gradual memory accumulation against `max_memory`, closing the gap that let the enforcement regression build green — only limit parsing was covered before, never the limit itself.

### Housekeeping

- **deps:** migrate Monty from a Git pin to the published `monty` / `monty-types` crates, then upgrade to v0.0.21. The host API breaks along the way: `LimitedTracker` and `NoLimitTracker` are gone, `ResourceTracker` became a concrete struct, the generic parameter it threaded through `MontyRepl` / `ReplProgress` / `RunProgress` disappeared with it, and `ResourceLimits::new()` gave way to its `Default` impl.
- **deps:** drop the `jiter` Git patch as its comment anticipated — v0.0.21 resolves `jiter` 0.16.0 from the registry, leaving no Git source in the lockfile.
- **deps:** refresh the full lockfile and upgrade `serde-saphyr` to 1.1 and `fancy-regex` to 0.19, the only root dependencies left behind. fancy-regex 0.19 makes `Captures` generic over its input type so it can back both `str` and `[u8]` matching; fimod only ever matches against `str`.
- **deps:** document why `get-size2` stays pinned at 0.10.1 — 0.10.2 already moves to `compact_str` 0.10 while Ruff is still on 0.9, putting two `CompactString` types in the graph and breaking Ruff's `derive(GetSize)`.
- **build:** scope the `unsafe_code` derogation to `src/mem_limit.rs` alone via a module-level `#![expect(...)]`, where the allocator charges Monty's memory counters; it stays denied everywhere else.
