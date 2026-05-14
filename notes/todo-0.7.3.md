# TODO 0.7.3

Transient backlog for the 0.7.3 cycle. Items move to user docs / changelog
once shipped.

The 0.7.2 cycle closed the structural refactor (`registry/*` + `cmd/*`
extraction) and shipped `notes/CODE_LAYOUT.md`. Audit `/simplify` on the
post-refactor codebase (cycle 0.7.2) surfaced 15 actionable findings,
grouped into 10 cohesive PRs below.

Two findings were explicitly **excluded from this cycle**:

- **MontyRun compilation cache** (one-shot CLI: cache between process
  invocations is meaningless; the batch/chain hot path doesn't justify
  the upstream-coordination cost).
- **Batch parallelism via `rayon --jobs N`** (deserves a dedicated
  "concurrency" cycle: exit-code semantics, output ordering, sandbox/
  registry-cache contention, flag surface area all need separate
  scoping).

In addition, the **httpmock 0.7 → 0.8.3 bump** (PR #20, Dependabot)
deferred from 0.7.2 lands as part of Pack A — see Pack A below.

## Pack A — Quick wins (1 PR, mostly mechanical)

| Finding | File(s) | Cost |
| --- | --- | --- |
| TOML round-trip via JSON string → direct `toml::from_str::<serde_json::Value>` | `src/format.rs:145-150` | 5 lines |
| `cache_base_dir` reimplements HOME/USERPROFILE fallback — promote to `paths::cache_dir()` (also respect `FIMOD_CACHE_DIR`) | `src/registry/catalog.rs:165-173`, `src/paths.rs` | ~15 lines |
| WHAT-comments cleanup (project rule "no comments unless explaining WHY") | `src/pipeline.rs:114`, `src/engine.rs:117-126`, `src/cmd/shape.rs:39` | trivial |
| Inline `path.file_stem().and_then(...)` in `scan_local_molds` — reuse `pipeline::path_stem` | `src/registry/catalog.rs:101-105` | trivial |
| **httpmock 0.7.0 → 0.8.3 bump** + `assert_hits` → `assert_calls` migration (8 occurrences) | `Cargo.toml`, `tests/cli/http_e2e.rs` | mechanical |

All five are zero-risk, can land together. The httpmock bump unblocks
`task outdated` so the next release doesn't need `--allow-outdated`.

Order: do this first to clear the bench before structural work.

## Pack B — Mold script lookup unification (1 PR)

**Triple duplication** of the local-mold lookup convention
(`{name}.py` → `{name}/{name}.py` → `{name}/__main__.py`):

| Site | File | Note |
| --- | --- | --- |
| `resolve_directory_mold` | `src/mold.rs:180-219` | extra "single .py" fallback |
| `resolve_local` | `src/registry/resolve.rs:240-278` | supports nested mold names |
| `find_local_mold_script` | `src/registry/molds.rs:272-286` | strict 3 rules |
| `scan_local_molds` | `src/registry/catalog.rs:91-144` | reimplemented inline while iterating |

Plan: extract `mold::find_script(base: &Path, name: &str) -> Option<(PathBuf, String /*rel*/)>`,
call from all four sites. Single source of truth for the convention;
adding a new fallback rule (e.g. `name/main.py`) becomes a one-line
change.

Combine with `mold::load_defaults(path: &Path) -> MoldDefaults` (5 call
sites in `registry/catalog.rs:112,133,477`, `registry/molds.rs:348,567`)
since both helpers naturally live in `mold.rs` and consolidate the
"resolve + read + parse defaults" trio.

## Pack C — Monty args ergonomics (1 PR)

`monty_args::expect_string` returns `&str`, but ~15 call sites need
owned `String` and reimplement `match &args[N] { MontyObject::String(s) => s.clone(), _ => bail!(...) }`:

- `src/dotpath.rs:135-189` (4×)
- `src/iter_helpers.rs:120-324` (5×)
- `src/format_control.rs:44-126` (4×)
- `src/msg.rs:44-46`, `src/env_helpers.rs:24-27`, `src/engine.rs:337-348` (1× each)

Plan: add `monty_args::take_string(obj: MontyObject, label: &str) -> Result<String>`,
migrate the 15 sites. Optionally also `take_dict`, `take_list` if the
same pattern repeats for non-string types.

## Pack D — CLI plumbing via clap (1 PR)

`cmd/shape.rs:19-62 build_script_refs` re-parses `std::env::args()`
manually to preserve the `-m` / `-e` ordering, with its own handling
for `-mFOO`, `--mold=FOO`, etc. The parser is fragile: doesn't handle
`--` separator, doesn't handle `-m=foo`.

Plan: replace with `clap::ArgMatches::indices_of("mold")` +
`indices_of("expression")`, merge by index. Drops ~30 net lines, gains
correctness on edge cases that clap already handles.

## Pack E — Copy-paste extraction (1 PR)

Two near-identical helpers, three sites each:

- `url_path_only(url: &str) -> &str` (strip `?`/`#`/`/` suffix) —
  `src/pipeline.rs:524-533`, `:922-929`, `:972-978`.
- `write_bytes_to(actual_output: Option<&str>, bytes: &[u8], debug: bool) -> Result<()>` —
  `src/cmd/shape.rs:209-216` (binary batch), `:222-256` (binary single),
  `src/pipeline.rs:744-760`.

Plan: extract both, migrate sites. Pure mechanical refactor.

## Pack F — HTTP client lifecycle (1 PR)

`http.rs::build_client` (~`http.rs:72`) rebuilds the `reqwest::blocking::Client`
on every fetch. For `fimod registry build-catalog` (N sources fetched)
or chain molds with multiple `--input-format http` steps, this adds
measurable connection-pool churn.

Plan: `static CLIENT_POOL: OnceLock<DashMap<(timeout_ms, no_follow), Arc<Client>>>` or `LazyLock` mapping `(timeout, no_follow)` tuples to a shared client. Drop the per-call build.

## Pack G — Template caching (1 PR)

`template.rs:52-71 render` creates `minijinja::Environment::new()` +
`add_template()` on every call. `template_str.clone()` (L83) and
`rel_path.clone()` (L111) compound the cost. `tpl_render_from_mold`
also `canonicalize()`s twice and `read_to_string()`s the target file
per invocation.

Plan (analogous to `regex.rs::REGEX_CACHE`):

- `tpl_render_from_mold`: process-wide cache `(canonical_template_path -> compiled_template)`.
- `tpl_render_str`: bounded LRU cache (e.g. 256 templates) keyed by
  template string, to avoid unbounded growth from data-derived templates.

Target workload: a mold that renders inside `for row in data:` on a
100k-row CSV. Today: 100k recompilations. After: 1.

## Pack H — `run_shape` decomposition (1 PR)

`cmd/shape.rs:64-269 run_shape` is 200+ lines of sequential validation,
branching, and raw-passthrough handling. Eight `if` blocks check
disjoint pre-conditions, then a 90-line `is_raw_output` branch
(lines 167-259) handles the binary short-circuit before falling
through to the normal pipeline.

Plan:

- Extract `validate_shape_args(shape: &ShapeArgs) -> Result<()>` —
  single pass, all the `--watch` + `--in-place` + `--no-input` +
  `--input-list` + `--output-format raw` combos.
- Extract `run_raw_passthrough(shape: ShapeArgs, http_opts: ...) -> Result<CliResult>` —
  the 90-line block lifted intact.
- `run_shape` falls back under 80 lines.

Internal refactor only — no CLI surface change.

## Pack I — Pipeline context typing (1 PR, structural)

`pipeline.rs::execute_chain` (`:115-148`) receives `context_base: &Value`
then re-extracts each field via `.get("input").and_then(...)`. Six
typed fields (`input`, `output`, `in_place`, `slurp`, `no_input`,
`input_format`, `output_format`, …) travel as strings in a JSON map
for no functional reason — the only consumer (`engine.rs:234`)
re-packs them into Dataclass `Step` for Monty.

Combine with param sprawl on `execute_chain` (9 params + `#[allow(clippy::too_many_arguments)]`)
and `run_pipeline_core` (13 params).

Plan:

1. Introduce `PipelineMetadata` struct (typed: `Option<PathBuf>`,
   `Option<DataFormat>`, `bool`, …).
2. Introduce `ChainExecCtx<'a>` grouping `extra_args` / `env_value` /
   `headers_value` / `policy` / `debug` / `msg_level`.
3. Introduce `InputResolution` grouping `csv_opts` / `http_opts` /
   `slurp` / `effective_input_format`.
4. Convert `PipelineMetadata → serde_json::Value` **only** at the
   Monty observation boundary (`engine.rs:234`).
5. Drop `build_context_base`.

Net delta: ~50 LOC removed, types tightened, zero stringification
in the hot path between steps.

## Pack J — Chain step Monty propagation (1 PR, depends on I)

Hot-path round-trip between chain steps:

```text
run_loop result  →  monty_to_json(result)  ──┐
                                            ├─ 2(N-1) full recursive
                  ┌── json_into_monty(value) ┘   walks for N-step chain
execute_chain ───┘
```

Today, every chain transition through `pipeline.rs:322` →
`engine.rs:721` does a full Value↔MontyObject conversion even when the
next step doesn't call `set_input_format` (which is when re-parsing is
genuinely required).

Plan: thread `MontyObject` directly between steps. Only convert to
`Value` at the chain entry (parse from input bytes) and chain exit
(serialize to output bytes), or when a step explicitly requests
`set_input_format` mid-chain. Requires changing `run_loop`'s return
signature to `(MontyObject, Option<i32>, Option<String>, Option<String>)`
and adding a small adapter at `execute_chain` for the
`set_input_format` case.

Impact: real (recursive structure walk on every step), measurable on
chains over MB-scale CSVs. Sequence **after** Pack I — once
`PipelineMetadata` is decoupled, the `MontyObject` propagation is a
clean change.

## Suggested ordering

1. **Pack A** first — clear the bench, unblock `task outdated`.
2. **Packs B/C/D/E in parallel** — independent, mechanical, can be
   landed in any order.
3. **Packs F/G** — caching layer additions, also independent.
4. **Pack H** — `run_shape` decomposition.
5. **Pack I** — typed pipeline context (the foundational refactor).
6. **Pack J** — Monty propagation, **must follow I**.

Strict prerequisite chain: J ⟵ I. Everything else is leaf.

## Cross-references

- `notes/CODE_LAYOUT.md` — file map for the post-refactor structure.
- `notes/ARCHITECTURE.md` — module diagram, extension points.
- `notes/DESIGN_NOTES.md` — invariants that any refactor must
  preserve (Monty I/O boundary, `serde_json::Value` as IR, …).
