# Changelog notes — 0.6.0

Working notes for the next release. Transfer to CHANGELOG.md at release time.

---

## Features

### Forced directives (`!=`)

Molds can lock a directive with `!=` so the CLI cannot override it:

```python
# fimod: output-format!=yaml
```

If the caller passes `--output-format json`, the mold's forced value wins.
`--debug` prints a `[debug] mold forces output-format=yaml` message when this kicks in.

Applies to `input-format` and `output-format`. Can be mixed with plain defaults on the same mold.

### `pipeline` parameter for molds

`transform()` now receives a `pipeline` object as a keyword argument, giving full read/write
access to the running pipeline.

#### Reading execution context

Step fields are read via `step.get('key')` — symmetric with `step.set('key', value)`.
Indexing (`step['key']`) and direct attribute access (`step.key`) are **not** supported
(Monty does not dispatch `__getitem__` on Dataclass; readable attrs are intentionally
emptied to keep a single API surface).

```python
def transform(data, pipeline, **_):
    step = pipeline.current_step()
    step.get('index')          # 0-based index of current step
    step.get('input_format')   # effective input format
    step.get('output_format')  # effective output format
    step.get('input')          # input file path (or None)
    step.get('output')         # output file path (or None)
    step.get('in_place')       # bool
    step.get('slurp')          # bool
    step.get('no_input')       # bool
    step.get('args')           # dict — current = merged (CLI ∪ Step.create.args, spec wins);
                                #        future step = spec args, {} if not injected with args=
    pipeline.length()          # total number of steps
```

Unknown keys raise `Step.get('<key>'): unknown field`.

#### Controlling the current step

Mutation goes through `step.set('key', value)` — `step['key'] = value` is **not** supported
(Monty does not dispatch `__setitem__` on Dataclass instances).

```python
pipeline.current_step().set('exit', 2)                  # set exit code
pipeline.current_step().set('output_format', 'yaml')    # override output format
pipeline.current_step().set('output_file', '/tmp/x.json')
pipeline.current_step().set('input_format', 'csv')      # re-parse body (same effect as set_input_format())
```

#### Reading/modifying a future step

```python
pipeline.step(1).get('output_format')                   # read
pipeline.step(1).set('output_format', 'yaml')           # propagated to the actual serialization of step 1
pipeline.step(1).set('output_file', '/tmp/x.json')      # propagated to MoldOptions.output_path of step 1
pipeline.step(1).set('input_format', 'csv')             # re-parse before step 1 runs
pipeline.step(1).set('args', {'k': 'v'})                # replace step 1's args block
                                                        # (CLI --arg merge still applied at exec)

# nested read/write via dp_get/dp_set on Step receiver
dp_get(pipeline.current_step(), "args.k1")
dp_get(pipeline.step(1), "args.deep.path", "default")
dp_set(pipeline.step(1), "args.strict", True)        # future-only; rejected on current
```

#### Guards

- `pipeline.step(-1)` → error `"pipeline.step(): index must be non-negative"`.
- Pipeline-only methods (`current_step`, `step`, `length`, `insert_next`, `append`) → error
  if called on a Step instance.
- `set(...)` → error if called on a Pipeline or on a `Step.create(...)` spec (only valid on a
  live Step instance from `current_step()` / `step(i)`).

#### Injecting new steps dynamically

`Step.create(...)` is the **only** accepted form (Monty does not support `class` definitions,
so `Step` is a constructor-style Dataclass exposed by fimod).

```python
pipeline.insert_next(Step.create(mold="@registry/normalize"))
pipeline.insert_next(Step.create(mold="@registry/normalize", input_format="json", output_format="yaml"))
pipeline.append(Step.create(expr="data['results']"))
pipeline.append(Step.create(expr="return len(data)", output_format="txt"))
```

`Step.create(mold, expr, input_format, output_format, args)` — exactly one of `mold` or `expr`
required. Bare-kwargs shortcut (`pipeline.insert_next(mold=...)`) and raw dict
(`pipeline.insert_next({"mold": ...})`) are **not** accepted — single contract via
`Step.create(...)`.

#### Backward compatibility

Global functions `set_exit_code()`, `set_input_format()`, `set_output_format()`,
`set_output_file()` continue to work unchanged — they write to the same internal channels.

### `fimod mold show --path`

Inspect a mold file directly by path, without registry lookup:

```bash
fimod mold show --path ./my_script.py
fimod mold show --path ./my_script.py --format json
```

Accepts an optional positional name override. Works with `__main__.py` directories.

### Mold directory: single `.py` fallback

When `-m dir/` points to a directory containing exactly one `.py` file,
fimod uses it automatically (no need to name it explicitly).
Multiple `.py` files → explicit error listing the candidates.

### `fimod mold list` shows registry name

Each entry now displays `[registry-name]` next to the mold name for clarity.

### Registry: new molds for the dynamic-pipeline patterns

Three new registry molds, each fixture-tested under `tests-molds/`, used as
reference implementations in [`docs/guides/dynamic-molds.md`](dynamic-molds):

- **`with_threshold`** — compute-then-inject pattern. Computes a percentile
  threshold live and injects a downstream filter via `Step.create(args=...)`
  (P1 typed args). 6 fixtures.
- **`auto_anonymize`** — conditional routing pattern. Inspects CSV headers and
  appends `@anonymize_pii` only when sensitive columns are present. 2 fixtures
  (with-email anonymisation, no-email passthrough).
- **`compact_if_big`** — adaptive output pattern. Flips `output_format` to
  `json-compact` when data exceeds a configurable threshold. 4 fixtures
  (big → compact, small → pretty, non-list passthrough, bad max → exit 1).

---

## Documentation

- `docs/reference/mold-defaults.md`: full `!=` forced directive section with examples.
- `docs/guides/authoring-molds.md`: `!=` tip added to the defaults table.
- `docs/guides/mold-scripting.md`: `!=` tip added near the defaults section.

---

## To-do before release

- [x] Fix doc: `mold-defaults.md` still mentions `[verbose]` in the tip — should be `[debug]`.
- [x] `Step.get(key)` implemented as the read API (Monty does not dispatch `__getitem__` on
  Dataclass — verified empirically). Step Dataclass `attrs` emptied so direct attribute
  access `step.key` also fails — single API surface via `.get()` / `.set()`.
- [x] P1 — `Step.create(args={...})` propage les args au mold injecté.
  Merge avec les `--arg` CLI ; en cas de conflit de clé, la valeur de `Step.create()` gagne.
  Types hétérogènes acceptés (bool, int, dict nested) — pas seulement string→string.
- [x] P2 — Lecture `step.get('args')` (current = merged ; future = spec args, `{}` si absent).
  5 tests verts dans `tests/cli/pipeline.rs` (`test_p2_*`).
- [x] P3 — Mutation `step.set('args', {...})` sur step futur (remplace bloc).
  Erreur `can only be set on a future step` si appelé sur `current_step()`.
  3 tests verts (`test_p3_*`).
- ~~P4 — `dp_get(step, "args.PATH")`~~ — **dropped from 0.6.0 scope**.
- ~~P5 — `dp_set(step, "args.PATH", value)`~~ — **dropped from 0.6.0 scope**.
  Both retired after design review: registry molds today take flat string args
  via CLI `--arg key=value`, so nested-path access on Step is overkill for
  every realistic use case (top-level `args` already covered by P2 `step.get('args')`
  and P3 `step.set('args', dict)`). Can be revisited if a future mold legitimately
  carries nested args.
- [x] Fixes B1, B2, B4, B5 + C1 from `notes/fix-0.6.0.md` (B3 retiré — non-bug,
  voir `fix-0.6.0.md` pour la preuve). 11 nouveaux tests dans `tests/cli/pipeline.rs`,
  tous verts:
  - [x] B1 stale `PipelineStep` / `step['…']` error messages → `Step.set(...)` wording.
  - [x] B2 `output_file` mutation on future step propagated to that step's `ctx.output_file`
        (last write wins; mold can read and override).
  - [x] B4 `pipeline.step(idx)` rejects negative `idx` with explicit error.
  - [x] B5 `in_place` / `slurp` / `no_input` propagated from `MoldContext` to future Step Dataclass
        (no longer hardcoded to `false`).
  - [x] C1 type guards on `dispatch_method` (verify Pipeline vs Step receiver).
- [x] Apply C2 cohérence: `output_format` on future step propagated to `format_override`
  (effective serialization, not only readable attribute).
- [x] Drop legacy bare-kwargs path in `extract_step_spec` (single contract via `Step.create()` — C3).
- [x] Document C4 + full pipeline section refresh in `docs/guides/mold-scripting.md`:
  migrated to `step.get()` / `step.set()` API, added `args` row on read/write tables,
  added `args=` to `Step.create()` table, added snapshot-semantics admonition for
  `pipeline.length()` / `pipeline.step(i)`. Added "Step receiver" subsection in
  `docs/reference/built-ins.md` for `dp_get` / `dp_set`.
- [x] New page `docs/guides/dynamic-molds.md` — philosophy of dynamic molds
  (compute-then-inject, conditional routing, args propagation, adaptive output),
  ecosystem comparison vs jq / yq / miller / Beam, when not to use, snapshot-vs-fan-out
  limitations. Linked in `mkdocs.yml` nav after Mold Scripting.
- [x] `molds/.ruff.toml` — list fimod-injected built-ins (`Step`, `dp_*`, `msg_*`,
  `re_*`, `it_*`, `hs_*`, `gk_*`, `set_*`, `tpl_*`, `env_subst`) under
  `[lint].builtins` so IDE linters (Ruff, Pyrefly) stop reporting `F821 undefined
  name` false positives on registry molds.
- [x] M1: validate `--arg max=N` in `molds/sample_if_large/sample_if_large.py` —
  clear error if missing, non-integer, or non-positive. 2 new fixtures
  (`missing_max`, `non_int_max`).
- [ ] Decide version bump: minor (0.6.0) or patch (0.5.1)?
