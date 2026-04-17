# Monty v0.0.14

- **Date**: 2026-04-17 (analysis + upgrade)
- **Previous fimod version**: v0.0.11
- **Monty release**: v0.0.14 (landed in fimod)

## Changes

### Fix panic on files > 65535 lines (#342)

`CodeLoc` fields changed from `u16` to `u32`. Prevents panic when parsing source files with lines larger than `u16::MAX`.
**Impact**: None — fimod doesn't use `CodeLoc` directly.

### `ExternalExceptionData` (#349)

New type for enriched external exception data.

### Natural JSON support in `MontyObject` (#348)

New `object_json` module with serialize-only wrappers:

- `JsonMontyObject` — wraps `&MontyObject`, serializes as natural JSON (`42` not `{"Int":42}`)
- `JsonMontyArray` — wraps `&[MontyObject]`
- `JsonMontyPairs` — wraps `&[(MontyObject, MontyObject)]`

Dict with all-string keys → plain JSON object. Non-string keys → `{"$dict": [[k, v], ...]}`.
Non-JSON-native types use `$`-tagged convention: `$tuple`, `$bytes`, `$set`, `$frozenset`, `$dataclass`, `$exception`, `$repr`, `$ellipsis`, `$float` (for nan/inf).

Also: `DictPairs::len()` and `DictPairs::is_empty()` are now public.

**Potential for fimod**: Could simplify parts of `convert.rs` (MontyObject→serde_json::Value), but only for serialization path. Deserialization (Value→MontyObject) still needs manual conversion.

## GOTCHA: `serde_json/arbitrary_precision` is now enabled transitively

v0.0.14 added `serde_json = { version = "1.0", features = ["arbitrary_precision"] }` to Monty's production deps (needed by `JsonMontyObject` for `BigInt` support). Cargo features are **additive and global**, so this flips `arbitrary_precision` on for **every** `serde_json` user in the fimod build — including fimod itself.

### Symptom

Any `serde_json::Value` handed to a **non-serde_json** serializer emits numbers as:

```toml
port = { "$serde_json::private::Number" = "8080" }
```

instead of `port = 8080`. That's serde_json's private contract between `Number` and its own serializer leaking out.

### Affected call sites in fimod

- `format.rs` — `toml::to_string_pretty(value)` and `serde_saphyr::to_string(value)`
- `template.rs` — `minijinja::Value::from_serialize(ctx_json)`

### Fix

Added `src/serde_compat.rs` with a `NativeNumbers<'a>(&'a Value)` newtype whose `Serialize` impl walks the tree and emits `Number` as bare `i64`/`u64`/`f64` via the serde visitor protocol. Zero-cost, applied at all three call sites.

If a future change introduces another path where a `serde_json::Value` reaches a non-serde_json serializer, wrap it in `NativeNumbers(&value)`.

## Breaking changes for fimod

None API-wise, but the `arbitrary_precision` leak required the `serde_compat` wrapper.

## Upgrade done

1. `Cargo.toml`: `tag = "v0.0.11"` → `tag = "v0.0.14"`
2. Added `src/serde_compat.rs` with `NativeNumbers` wrapper + tests
3. Wired `NativeNumbers` into `format.rs` (TOML, YAML) and `template.rs` (minijinja)
4. Updated `docs/reference/monty-engine.md` version references
5. All tests green (165 unit + 242 CLI + 2 mold fixtures)
