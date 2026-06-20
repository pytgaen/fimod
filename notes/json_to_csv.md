# Identity JSON → CSV streaming

## Context

The `-e data` identity fast-path (`is_native_identity_chain`, `src/cmd/shape.rs`)
bypasses Monty entirely: it reads → parses → serializes without any
`Value ↔ MontyObject` conversion. On top of that, one streaming case is already
implemented in `src/pipeline.rs`:

- `try_stream_identity_json_array` streams a **top-level JSON array → NDJSON/CSV**
  without materializing the array when the target format supports it
  (`format::stream_json_array_to_ndjson` / `format::stream_json_array_to_csv`,
  `DeserializeSeed`/`Visitor` implementations that write each element as it is
  parsed).

Guard conditions (`stream_identity_json_array_input_format`): not `--check`,
`--debug`, `--slurp`, `--no-input`, not an HTTP URL, not in-place, input is
`json`/`json-compact`, output is `ndjson`/`csv`, and the content actually starts
with `[`.

This note covers extending that mechanism to **array JSON → CSV**.

## Scope: what streams, what doesn't

A target format is streamable from a JSON array iff it emits **one output unit per
element, without needing the whole array**.

| Target | Streamable in identity | Reason |
|---|---|---|
| **NDJSON** | ✅ done | 1 element = 1 JSON line |
| **CSV** | ✅ done | 1 object = 1 row; real structural reprojection NDJSON can't do |
| lines | ❌ no value | for objects `lines ≡ ndjson` char-for-char; only differs for arrays of **strings** (niche). Its real value comes from a *transform*, not identity |
| YAML | ❌ | `serde_saphyr` serializes the whole Value; manual indentation, never a bulk target |
| JSON (array) | ❌ | json→json is a copy/reformat (same non-sense as ndjson→ndjson) |
| TOML | ❌ | top-level array-of-tables doesn't stream |

So the clean perimeter is: **array JSON → { NDJSON ✓, CSV }**.

## Why CSV is clean

Current `serialize_csv` (`src/format.rs`) already derives columns from the
**first object only**:

```rust
let first_obj = rows[0].as_object()?;
let columns: Vec<String> = first_obj.keys().cloned().collect();
```

So "header frozen on the first element" is the *existing contract*, not a
concession to streaming. Therefore the default streamed CSV path must keep that
contract: `--csv-scan` defaults to `1`. With the default, streamed or not, the
output stays identical → zero behavior change. Field stringification
(`value_to_field`: string as-is, null → empty, anything else → compact JSON) is
reused verbatim.

## Explicit output schema: `--csv-header`

Today `--csv-header a,b,c` primarily means: "when reading headerless CSV, name
the input columns `a`, `b`, `c`". For JSON/NDJSON input, that input-side meaning
does not apply. In an identity JSON → CSV conversion, reusing the same option as
an explicit **output schema** is readable and consistent:

```bash
echo '[{"a":1,"b":2},{"a":3,"c":9}]' \
  | fimod s --input-format json --output-format csv --csv-header a,b,c -e data
```

Expected projection:

```csv
a,b,c
1,2,
3,,9
```

Rules:

- If the input is CSV, `--csv-header` keeps its current input-column meaning.
- If the output is CSV from object rows, `--csv-header` is also the explicit
  output column schema: order is fixed, missing values become empty cells,
  off-schema keys are ignored.
- This must be implemented in the normal `serialize_csv` path too, not only in
  the streaming seed, so the result does not depend on whether the fast-path
  activates.
- There is no separate `--csv-header-names` option.

## The header problem

The header is the **first line written**. In a single streaming pass you must
commit to the column set *before* you have seen all elements. This splits "sparse
matrix" into two kinds of holes:

- **Vertical hole** — known column, value missing on some rows → empty cell.
  Trivial in streaming, already handled (`unwrap_or("")`). ✅
- **Horizontal hole** — a *new* column discovered mid-stream. Impossible to fix
  in one pass: once the header is on stdout it can't be widened retroactively. ❌

Full automatic union of all keys (the "true" auto sparse matrix) is therefore
**not streamable**: it needs a full pre-scan = two passes / buffer everything =
no streaming. That case is only available through an explicit full-scan mode
(`--csv-scan 0`); it must not become the implicit default.

## Strategies

```bash
echo '[{"a":1,"b":2},{"a":3,"c":9}]' \
  | fimod s --input-format json --output-format csv -e data
```

| Mode | Header source | Vertical holes | New columns | Memory | Result |
|---|---|---|---|---|---|
| default auto (`--csv-scan 1`) | keys of `rows[0]` | filled (empty) | dropped silently | O(1) | `a,b` / `1,2` / `3,` → `c:9` **lost** |
| explicit `--csv-header a,b,c` | declared schema | filled (empty) | dropped if off-schema | O(1) | `a,b,c` / `1,2,` / `3,,9` → deterministic |
| explicit window (`--csv-scan N`) | union of first **N** | filled (empty) | dropped only if first seen **after N** | O(N) | parfait si array < N |
| full union (`--csv-scan 0`) | union of all | filled | none lost | O(file) | opt-in, not really streaming |

## Bounded look-ahead (`--csv-scan`)

Automatic widening would be a behavior change, so the default stays equivalent
to the current serializer: `--csv-scan 1`.

When the user passes `--csv-scan N`, a sampling window over the first N elements
becomes the explicit trade-off between "1st line" (current behavior) and "scan
all" (perfect but not streamable). Implemented directly in `visit_seq`, two
phases:

**Warm-up** (first N elements):
- buffer into a `Vec<Value>` capped at N
- accumulate the **union of keys** in an insertion-ordered set (first object's
  keys, then new ones as they appear — `serde_json` preserve_order keeps this
  natural)

**Switch** (at N reached *or* end of array, whichever first):
- freeze `columns` = collected union
- write the header
- drain the buffer (each element projected onto `columns`)

**Stream** (remainder, > N):
- write each element as it is parsed, projected onto the frozen `columns`

Cost: `O(N)` memory, not `O(file)`. 10 000 JSON objects ≈ a few MB. On a
multi-million-line file the streaming benefit is preserved.

**Free bonus**: if the array has fewer than N elements (common for medium
files), the union is **complete** → perfect sparse matrix at bounded cost. The
"never perfect" caveat only bites files > N whose schema drifts late.

`--csv-scan 0` means "scan all rows before writing the header". It is useful and
deterministic, but it deliberately gives up the streaming memory guarantee. That
cost must be explicit at the CLI.

**Detectability**: the stream phase already projects, so it can **count** dropped
keys past the window and emit a **single aggregated warning** at the end
(`N rows had M unknown columns dropped — increase --csv-scan or use --csv-header`).
One warning, not one per line. For `--csv-scan 1`, keeping silence would match
today's `serialize_csv`; warning is useful but should be decided as a CLI
compatibility choice.

Decision for the first implementation: keep the current silent behavior. Dropped
off-schema keys remain silent for compatibility; diagnostics can be added later
as an explicit CLI behavior.

## Resolution order

1. `--csv-header` given → deterministic schema, no scan.
2. else `--csv-scan N` → union of first N, frozen header, silent drop of keys
   first seen after the window.
3. default is `--csv-scan 1` → current first-object behavior.

## Implementation sketch

1. `JsonArrayCsvSeed` / `Visitor` mirroring `JsonArrayNdjsonVisitor`, wrapping a
   `csv::Writer`, carrying `CsvOptions` + the window size N.
   - warm-up buffer + ordered key union; switch; stream-project.
   - reuse `value_to_field` for cells.
   - array-of-arrays mode (`rows[0].is_array()`, optional header from
     `--csv-header`) folds into the same seed for consistency.
2. Add `--csv-scan N` to `ShapeArgs` and `CsvOptions`.
   - default `1`
   - `0` = full scan / full union, explicit memory trade-off
3. Teach `serialize_csv` to use `opts.header_names` as the output schema for
   object rows too, so `--csv-header` is consistent outside the streaming path.
4. Widen the guard in `stream_identity_json_array_input_format` (currently
   `out_fmt == Ndjson`) to also accept `out_fmt == Csv`, passing `csv_opts`.
5. Empty array (0 elements) → write nothing, matching today's `rows.is_empty()`.
6. Wire the renamed identity JSON-array dispatcher to
   route json-array → csv into the new seed.
