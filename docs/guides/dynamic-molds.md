# Dynamic Molds

A **dynamic mold** is a mold that does not just transform data — it **reshapes the pipeline itself** based on what the data looks like. It can compute parameters from the live values, inject new steps after itself, or mutate the args of a step that has not run yet.

This is the distinctive trait of fimod compared to other CLI data tools: a mold is both a *transformer* and an *orchestrator*.

---

## A first example

```bash
fimod s -i events.json -m @with_threshold --arg col=duration_ms --arg pct=95
```

The `with_threshold` mold computes the 95th percentile of `duration_ms` from the data it is given, then injects a downstream filter step pre-configured with that threshold. The cut-off value is **decided at runtime, from the data itself** — there was no `--arg threshold=N` to write because no human knew what `N` should be in advance.

In a static-pipeline tool, you'd have to:

1. Run a first command to get the percentile.
2. Eyeball / parse / shell-substitute the value.
3. Run a second command with that value baked in.

Or write a custom script. With fimod, one mold encapsulates the full logic and remains composable in the chain.

---

## Why this exists

Most data wrangling pipelines look like a sequence: parse → transform → filter → serialize. As long as the steps and parameters are known up front, a static composition (`jq '.foo' | jq '.bar'`, `mlr cat then put then filter`) is enough.

But many real workflows have a feedback loop:

- "Filter outliers above the 95th percentile" — the threshold is a property of the data.
- "Sample only if the list is too large" — the decision depends on `len(data)`.
- "Validate against the schema inferred from the first row" — the schema is derived from input.
- "Route through @anonymize_pii only if the source is production" — the routing depends on env/args.

These cases force a choice: either give up the pipeline composability and write a one-off script, or pre-compute the parameters in a separate command and string things together with shell substitution. Both lose the "one CLI invocation, one declarative chain" property.

Dynamic molds keep that property. The pipeline becomes a runtime object that the data can reshape — but every mutation is constrained, queued, and applied at well-defined points (snapshot semantics, see below).

---

## One invocation, one pipeline

The practical payoff of dynamic molds is that *the decision* and *the action it parameterizes* live in the same mold — and therefore in the same CLI call. The data is read once, the computed value never crosses the shell boundary, and the chain stays a single declarative line.

Written by hand with shell substitution, the percentile example becomes a small script:

```bash
jq --argjson t "$(jq '[.[].duration] | sort | .[length*0.95|floor]' events.json)" \
   '[.[] | select(.duration > $t)]' events.json
```

Two reads of the same file, a value bounced through the shell, careful escaping if the input contains anything odd. With a dynamic mold the same workflow collapses to:

```bash
fimod s -i events.json -m @with_threshold --arg col=duration --arg pct=95
```

The call site reads as a single intent — "filter by p95 of `duration`" — and the patterns below extend the same idea to conditional routing and adaptive serialization.

---

## Canonical patterns

### 1. Compute-then-inject

The mold inspects data, computes a parameter, and injects a downstream step parameterized by that value.

```bash
fimod s -i events.json -m @with_threshold --arg col=duration_ms --arg pct=95
```

```python
# Simplified core of @with_threshold: compute a percentile, inject a filter using it.
def transform(data, args, pipeline, **_):
    col = args["col"]
    pct = float(args.get("pct", "95"))
    values = sorted(r[col] for r in data if isinstance(r.get(col), (int, float)))
    threshold = values[int((len(values) - 1) * pct / 100)]  # nearest-rank percentile
    pipeline.insert_next(Step.create(
        expr=f"[r for r in data if r[{col!r}] > args['threshold']]",
        args={"threshold": threshold},
    ))
    return data
```

Reference mold: `with_threshold` — shipped in the default registry, fixture-tested in `tests-molds/with_threshold/`. The published version adds full arg validation (missing/non-int/out-of-range), linear-interpolation percentile (NIST type 7), and operator whitelist for downstream filter safety. See also `sample_if_large` for the same pattern applied to row-count cutoffs.

### 2. Conditional routing

Inspect the data shape; append a downstream registry mold only if the shape warrants it.

```bash
# Same command for any CSV — the mold decides whether to anonymize.
fimod s -i users.csv -m @auto_anonymize
```

```python
# Core of @auto_anonymize: append @anonymize_pii when sensitive columns are present.
def transform(data, args, headers, pipeline, **_):
    sensitive = [s.strip() for s in args.get("detect", "email").split(",") if s.strip()]
    found = [s for s in sensitive if headers and s in headers]
    if found:
        pipeline.append(Step.create(
            mold="@anonymize_pii",
            args={"fields": ",".join(found)},
        ))
    return data
```

The chain is decided from the input shape (the CSV header in this case): the caller runs the same command on every export, and the mold appends `@anonymize_pii` only when a sensitive column is present. Exports without those columns flow through untouched, no flag to wire.

Reference mold: `auto_anonymize` — shipped in the default registry, fixture-tested in `tests-molds/auto_anonymize/`.

### 3. Adaptive output

The mold decides how the chain should serialize based on output size or shape.

```bash
fimod s -i logs.json -m @compact_if_big --arg max=1000
```

```python
# Core of @compact_if_big: flip to compact JSON when the result is large.
def transform(data, args, pipeline, **_):
    max_items = int(args.get("max", "1000"))
    size = len(data) if isinstance(data, (list, dict)) else None
    if size is not None and size > max_items:
        pipeline.current_step().set('output_format', 'json-compact')
    return data
```

Keeps the pretty default for small results, switches to compact for bulk dumps — without forcing the caller to think about it.

Reference mold: `compact_if_big` — shipped in the default registry, fixture-tested in `tests-molds/compact_if_big/`.

---

## When NOT to use a dynamic mold

Dynamic molds add a layer of indirection. Do not reach for them when the simpler alternatives work:

- **Static parameters** known at the call site → use `--arg key=value` or inline `-e expr`.
- **Pure transformation** with no decision tree → write a regular `transform(data, ...)` mold without the `pipeline` parameter.
- **One-off scripts** that won't be reused → an inline `-e` is fine, no need to register a mold.

The cost of a dynamic mold is readability: someone reading the chain has to open the mold to know what gets injected. Use it when the dynamic decision is **the value-add** of the mold (the user wants the live computation), not as a generic structuring mechanism.

---

## Limitations

### Bounded dynamic growth

A chain may inject at most 1,024 steps in total. This global limit covers both
`pipeline.insert_next()` and `pipeline.append()` and stops recursive molds from
growing the pipeline forever. All static and injected steps share the same
invocation-wide `max_duration` budget.

### Snapshot semantics

`pipeline.length()`, `pipeline.step(j)`, and the list of remaining steps are **computed once per step**, at the start of `transform()`. A step injected by step *i* via `insert_next` or `append` is **only visible from step *i+1* onwards**. You cannot read or mutate a step you have just appended in the same `transform()` call.

Practically: do the injection, return data, and let the next step (which can be an inline `-e` in the same CLI invocation) interact with the injected step.

### Series, not fan-out

`append` chains a step at the end of the pipeline; it does not create N parallel runs of the same downstream mold. The pipeline is strictly series — each step receives the output of the previous one. If you need a fan-out (process N chunks in parallel), the right tool is a shell loop or an external job runner; dynamic molds are not a replacement for that.

### `set('args')` is future-only

`step.set('args', {...})` is forbidden on the current step. The current step's `args` has already been passed to its `transform()` — mutating it would have no observable effect. Use a future step receiver instead.

### Deterministic, not reactive

A mold sees its input data once. It cannot "subscribe" to events or react to streaming data. Dynamic in fimod means *the chain is decided at runtime from the input data*, not *the chain reacts to a stream of events*.

---

## API reference

The full `pipeline` parameter API — `current_step()`, `step(i)`, `length()`, `insert_next`, `append`, `Step.create(...)`, `step.get` / `step.set` — is documented in the [Mold Scripting](mold-scripting.md) guide.

This page is the *why*; that one is the *how*.
