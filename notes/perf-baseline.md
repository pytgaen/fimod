# Performance baseline

## Référence v0.7.3

Date : 2026-05-15 — profil `release` : `opt-level = "z"`, `lto`, `codegen-units = 1`, `strip`

```
mise exec -- cargo test --release --test performance -- --ignored --nocapture
```

### Microbenchmarks

| Test | Médiane | Budget |
| --- | --- | --- |
| JSON parse + Monty round-trip + compact serialize (20 000 records) | 83.3 ms | 350 ms |
| CSV direct Monty round-trip + serialize (20 000 records) | 46.0 ms | 450 ms |
| 3-step mold chain (1 500 records) | 16.0 ms | 250 ms |

### Comparaisons CLI

| Comparaison | fimod | outil | ratio |
| --- | --- | --- | --- |
| JSON filter 20 000 records vs jq | 108.6 ms | 48.9 ms | 2.22x |
| YAML → JSON 5 000 records vs yq | 71.3 ms | 71.2 ms | 1.00x |
| Line filter 50 000 records vs awk | 29.9 ms | 10.0 ms | 3.00x |

---

## Synthèse des gains — session v0.7.4-dev (2026-05-15)

Trois optimisations output fast-path + un binaire vitesse (`ffimod`).

### Output fast-path : `MontySerialize`

Principe : sérialiser `MontyObject` directement sans `monty_to_json → Value` intermédiaire.
Actif sur `json-compact`, `ndjson`, `lines`, `txt` quand pas de `--check`, `--debug`
ni `set_output_format()` dans le mold.

| Format | Mécanisme | Gain CLI mesuré |
| --- | --- | --- |
| `json-compact` | `serde_json::to_vec(&MontySerialize)` | −15 % (108 → 92 ms vs jq) |
| `ndjson` | itère `List`, `MontySerialize` par item | ~−15 % (estimé, non benchmarké) |
| `lines` | bytes bruts pour `String`, `MontySerialize` sinon | −17 % (30 → 25 ms vs awk) |
| `txt` | bytes bruts pour `String`, `MontySerialize` sinon | négligeable (output unique) |

Note : fast-path input (`json_str_to_monty` via `serde::de::Visitor`) implémenté puis
retiré — `monty` active `serde_json::arbitrary_precision` ce qui force les nombres
dans `visit_map` via un sentinel interne, annulant le gain.

### Comparaisons CLI — outil / fimod / ffimod

| Comparaison | outil | fimod v0.7.3 | fimod v0.7.4-dev | ffimod v0.7.4-dev |
| --- | --- | --- | --- | --- |
| JSON filter 20 000 records vs jq | 47.5 ms | 108.6 ms (2.22x) | 92.5 ms (1.95x) | 72.2 ms (1.41x) |
| YAML → JSON 5 000 records vs yq | 70.1 ms | 71.3 ms (1.00x) | 68.9 ms (0.98x) | 42.4 ms (0.56x) |
| Line filter 50 000 records vs awk | 8.5 ms | 29.9 ms (3.00x) | ~25 ms (~3.0x) | 19.4 ms (2.26x) |

---

## ffimod — binaire speed-optimized

Profil `release-fast` : hérite de `release`, `opt-level = 3`.
Activé via feature gate : `cargo build --profile release-fast --features ffimod`.
Tâches : `task build:fast` (local) / `task dist:fast:linux:x86_64` (dist musl, sans UPX).

### Taille binaire

| Binaire | opt-level | Brut | UPX `--best --lzma` |
| --- | --- | --- | --- |
| `fimod` | z | 8.5 MB | 2.9 MB |
| `ffimod` | 3 | 13 MB (+53 %) | 3.8 MB (+31 %) |

UPX ajoute ~160–240 ms de décompression par invocation — rédhibitoire pour CLI
courtes. Distribution : `fimod` avec UPX, `ffimod` sans UPX.

### Microbenchmarks ffimod vs fimod

| Test | fimod (opt-z) | ffimod (opt-3) | Δ |
| --- | --- | --- | --- |
| JSON parse + Monty round-trip + compact serialize (20 000 records) | 80.8 ms | 68.4 ms | −15 % |
| CSV direct Monty round-trip + serialize (20 000 records) | 43.3 ms | 35.3 ms | −18 % |
| 3-step mold chain (1 500 records) | 18.6 ms | 14.1 ms | −24 % |

---

## Optimisations restantes envisagées

1. Fast-path output `json` (pretty) — gain faible (~5-8 %), peu prioritaire
