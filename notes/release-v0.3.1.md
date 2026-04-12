# Release v0.3.1

Patch release — fixes and polish after v0.3.0.

## Fixes

- **`it_group_by` preserves insertion order** — switched from `BTreeMap` to `IndexMap` so grouped output follows the order data appears in, instead of being sorted alphabetically.
- **Quoted descriptions in `# fimod:` directives** — arg descriptions containing commas can now be quoted (`"..."` or `'...'`) so they aren't split as separate directives. All bundled molds updated to use quoted descriptions where needed.
- **`--mold=` / `--expression=` long-form flags** — the `=`-style long flags were not recognized by the manual pre-parse pass; fixed.
- **OsCall debug logging** — intercepted `OsCall` now emits a `[debug]` message instead of failing silently.

## Improvements

- **`mold show --output-format json`** — machine-readable JSON output for `fimod mold show`.
- **Documentation** — expanded `formats.md` (lines vs txt vs NDJSON guidance, shell-friendly recipes), updated `cli-reference.md` and `built-ins.md`.

## Housekeeping

- Added `indexmap` dependency.
- Removed dead `parse_data` / `parse_file` functions from `pipeline.rs`.
- Updated `CHANGELOG.md` for v0.3.0 final wording.
