## [0.6.0] — YYYY-MM-DD

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
