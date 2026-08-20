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

Trois optimisations output fast-path + un binaire vitesse (`fimod-fast`).

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

### Comparaisons CLI — outil / fimod / fimod-fast

| Comparaison | outil | fimod v0.7.3 | fimod v0.7.4-dev | fimod-fast v0.7.4-dev |
| --- | --- | --- | --- | --- |
| JSON filter 20 000 records vs jq | 47.5 ms | 108.6 ms (2.22x) | 92.5 ms (1.95x) | 72.2 ms (1.41x) |
| YAML → JSON 5 000 records vs yq | 70.1 ms | 71.3 ms (1.00x) | 68.9 ms (0.98x) | 42.4 ms (0.56x) |
| Line filter 50 000 records vs awk | 8.5 ms | 29.9 ms (3.00x) | ~25 ms (~3.0x) | 19.4 ms (2.26x) |

---

## fimod-fast — binaire speed-optimized

Profil `release-fast` : hérite de `release`, `opt-level = 3`.
Activé via feature gate : `cargo build --profile release-fast --features fast`.
Tâches : `task build:fast` (local) / `task dist:fast:linux:x86_64` (dist musl, sans UPX).

### Taille binaire

Mesure native Linux actualisée le 2026-05-22 après le passage à `reqwest`
0.13 / `rustls` / AWS-LC.

| Binaire | opt-level | Brut | UPX `--best --lzma` |
| --- | --- | --- | --- |
| `fimod` | z | 10.8 MB | 3.3 MB |
| `fimod-fast` | 3 | 14.7 MB (+36 %) | 4.3 MB (+31 %, not distributed compressed) |

UPX ajoute ~160–240 ms de décompression par invocation — rédhibitoire pour CLI
courtes. Distribution : `fimod` avec UPX, `fimod-fast` sans UPX.

### Microbenchmarks fimod-fast vs fimod

| Test | fimod (opt-z) | fimod-fast (opt-3) | Δ |
| --- | --- | --- | --- |
| JSON parse + Monty round-trip + compact serialize (20 000 records) | 80.8 ms | 68.4 ms | −15 % |
| CSV direct Monty round-trip + serialize (20 000 records) | 43.3 ms | 35.3 ms | −18 % |
| 3-step mold chain (1 500 records) | 18.6 ms | 14.1 ms | −24 % |

---

## Mesure locale 0.9.1-dev — 2026-07-10

Cette section est un instantané de la machine de développement, pas une
promesse de performance portable : AMD Ryzen AI 9 HX 370, Linux WSL2 x86_64.
Les valeurs sont les médianes de cinq exécutions après warm-up.

```text
mise exec -- cargo test --release --test performance -- --ignored --nocapture
mise exec -- cargo test --profile release-fast --features fast --test performance -- --ignored --nocapture
```

### Conversions identité CLI

| Conversion | Jeu actuel | `release` | `release-fast` | Budget |
| --- | ---: | ---: | ---: | ---: |
| NDJSON → JSON compact | 20 000 lignes | 45.765 ms | 29.044 ms | 250 ms |
| JSON → NDJSON | 20 000 objets | 56.953 ms | 30.767 ms | 250 ms |
| JSON → CSV | 20 000 objets | 57.871 ms | 31.119 ms | 300 ms |

Les conversions JSON → NDJSON/CSV et NDJSON → JSON sont exécutées en streaming
par le chemin identité `-e data`. Les trois sorties ont été vérifiées pendant
la mesure (nombre de lignes ou tableau JSON analysable).

### Autres mesures du même passage

| Test | `release` | `release-fast` |
| --- | ---: | ---: |
| JSON parse + Monty round-trip + compact serialize (20 000 objets) | 90.848 ms | 70.669 ms |
| CSV direct Monty round-trip + serialize (20 000 lignes) | 65.163 ms | 39.945 ms |
| Chaîne de 3 molds (1 500 objets) | 25.600 ms | 14.220 ms |
| Filtre JSON vs jq | 76.473 ms / 1.39x | 57.666 ms / 0.97x |
| YAML → JSON vs yq | 75.937 ms / 0.87x | 46.472 ms / 0.50x |
| Filtre lignes vs awk | 43.049 ms / 3.47x | 22.749 ms / 1.77x |

Ce passage ne couvre pas encore une matrice 10/100/1 000 MB, la mémoire RSS,
ni plusieurs machines. Ces mesures de volume et de mémoire restent un protocole
futur ; elles ne doivent pas être déduites des chiffres ci-dessus.

---

## Mesure locale après Monty 0.0.19 — 2026-07-28

Les neuf tests de `tests/performance.rs` passent en profils `release` et
`release-fast`. Ce passage utilise `--test-threads=1` pour éviter que les tests
se concurrencent pendant les comparaisons :

```text
mise exec -- cargo test --release --test performance -- --ignored --nocapture --test-threads=1
mise exec -- cargo test --profile release-fast --features fast --test performance -- --ignored --nocapture --test-threads=1
```

| Scénario | `release` | `release-fast` |
| --- | ---: | ---: |
| JSON → Monty → JSON compact, 20 000 objets | 53,763 ms | 37,022 ms |
| CSV → Monty → CSV, 20 000 lignes | 31,813 ms | 24,403 ms |
| Chaîne de trois molds, 1 500 objets | 9,938 ms | 7,162 ms |
| JSON → NDJSON, identité native, 20 000 objets | 29,117 ms | 17,324 ms |
| Filtre JSON | 61,075 ms, soit 1,32× `jq` | 41,051 ms, soit 0,87× `jq` |

### Probes exploratoires de temps et de mémoire

| Scénario | Temps | Pic RSS |
| --- | ---: | ---: |
| `it_sort_by(data, "name")`, 100 000 objets | 0,49 s | 390 Mio |
| `sorted(data, key=lambda row: row["name"])`, 100 000 objets | 0,24 s | 249 Mio |
| JSON → NDJSON, identité native, 100 000 objets | 0,03 s | 13 Mio |
| CSV → NDJSON, identité matérialisée, 100 000 lignes | 0,06 s | 85 Mio |

Ces probes localisent des coûts, mais ne comparent pas des contrats strictement
identiques : les deux tris diffèrent sur certains cas hétérogènes, et les deux
conversions n'utilisent pas le même parseur. Ces valeurs sont des mesures
locales, pas une promesse portable.

---

## Optimisations restantes envisagées

1. Matrice 10/100/1 000 MB avec temps, débit et RSS sur plusieurs formats.
