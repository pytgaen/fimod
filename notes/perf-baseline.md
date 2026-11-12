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

---

## Mesure A/B locale — optimisations 1–3, 2026-09-09

Comparaison du binaire de `2ce7ccd` sauvegardé avant modification avec le
binaire du worktree après optimisation. Même Rust 1.98.1, Monty 0.0.23,
profil `release` (`opt-level=z`, LTO, mimalloc avec comptage mémoire), sans UPX.
Machine : AMD Ryzen AI 9 HX 370, Linux WSL2 x86_64.

Les temps couvrent la commande CLI entière, avec caches chauds. Médianes de
sept passages par binaire, neuf pour dotpath, en alternant l’ordre avant/après.
Le pic RSS est la médiane de trois exécutions séparées via `/usr/bin/time -f %M`.
Les sorties avant/après sont identiques octet pour octet. Les autres builds et
les suites de tests ne tournent pas pendant les mesures.

### Dotpath : même document et même chemin avant/après

20 000 objets, environ 1,56 Mo ; tableau placé sous 1 ou 8 clés `node`.
La modification porte sur `flag`, à côté du tableau. Sortie JSON compacte
vers `/dev/null`.

| Opération | Profondeur | Avant | Après | Temps gagné | RSS avant → après |
| --- | ---: | ---: | ---: | ---: | ---: |
| `dp_set` | 1 | 130.2 ms | 108.5 ms | 16.7 % | 122.8 → 94.4 Mio |
| `dp_set` | 8 | 230.7 ms | 116.9 ms | 49.3 % | 393.0 → 94.4 Mio |
| `dp_delete` | 1 | 133.8 ms | 109.7 ms | 18.0 % | 123.0 → 94.5 Mio |
| `dp_delete` | 8 | 242.8 ms | 114.6 ms | 52.8 % | 393.1 → 94.3 Mio |

### Helpers et sortie : 100 000 objets, 7,87 Mo

Cinq champs par objet : `id`, `name`, `active`, `score`, `team`. Le tri et les
extrema utilisent `score`, le comptage utilise `team`. Sorties des helpers en
JSON compact vers `/dev/null`. Les cas NDJSON écrivent tous dans le même fichier
local ; seul le choix du format change. Le mold retourne `data`, et appelle
également `set_output_format("ndjson")` dans le cas override.

| Scénario | Avant | Après | Temps gagné | RSS avant → après |
| --- | ---: | ---: | ---: | ---: |
| `it_sort_by` | 924.9 ms | 615.9 ms | 33.4 % | 394.8 → 394.3 Mio |
| `it_count_by` | 353.8 ms | 324.1 ms | 8.4 % | 245.0 → 240.5 Mio |
| `it_min_by` | 362.7 ms | 318.8 ms | 12.1 % | 240.9 → 238.3 Mio |
| `it_max_by` | 364.3 ms | 317.8 ms | 12.8 % | 238.9 → 228.3 Mio |
| NDJSON explicite (témoin) | 356.8 ms | 356.8 ms | stable | 251.0 → 250.4 Mio |
| NDJSON déduit de `.ndjson` | 393.7 ms | 353.6 ms | 10.2 % | 251.0 → 250.5 Mio |
| NDJSON choisi par le mold | 398.6 ms | 372.9 ms | 6.4 % | 251.0 → 250.5 Mio |

Le gain mémoire net concerne ici dotpath (environ −76 % à profondeur 8).
Le pic RSS du tri et de la sérialisation reste pratiquement inchangé.
Ces mesures locales ne prédisent pas le gain sur tout mold : le passage
Monty/Rust demeure, et les valeurs demandant une normalisation JSON gardent
le chemin de compatibilité.

### Reproduire les jeux A/B

Exécuter dans un répertoire de travail temporaire, en conservant les deux
binaires `fimod-before` et `fimod-after` construits avec le même profil :

```python
import json
from pathlib import Path

def row(i, score):
    return dict(id=i, name=f"user-{i:06}", active=i % 2 == 0,
                score=score, team=f"team-{i % 16}")

Path("users.json").write_text(json.dumps(
    [row(i, (i * 7919) % 100000) for i in range(100000)],
    separators=(",", ":")))
for depth in (1, 8):
    data = dict(rows=[row(i, i) for i in range(20000)], flag=False)
    for _ in range(depth):
        data = dict(node=data)
    Path(f"depth-{depth}.json").write_text(json.dumps(data, separators=(",", ":")))
Path("identity.py").write_text("def transform(data, **_):\n    return data\n")
Path("override.py").write_text(
    "def transform(data, **_):\n    set_output_format(\"ndjson\")\n    return data\n")
```

Commandes par binaire, avec warm-up puis répétitions alternées selon le
protocole ci-dessus :

```bash
rtk run /usr/bin/time -f "%e s / %M KiB" ./fimod-before s --sandbox-file= -i users.json -e 'it_sort_by(data, "score")' --output-format json-compact > /dev/null
rtk run /usr/bin/time -f "%e s / %M KiB" ./fimod-after s --sandbox-file= -i users.json -e 'it_sort_by(data, "score")' --output-format json-compact > /dev/null
```

Pour dotpath, utiliser `depth-8.json` avec
`dp_set(data, "node.node.node.node.node.node.node.node.flag", True)` ou
`dp_delete(data, "node.node.node.node.node.node.node.node.flag")`.
Pour NDJSON : `-m identity.py -o out.ndjson` avec ou sans
`--output-format ndjson`, puis `-m override.py -o out.ndjson --output-format ndjson`.

Les scénarios sont aussi couverts à 20 000 objets par les tests opt-in
`cli_nested_dotpath_edits_under_budget`,
`cli_native_iter_helpers_over_large_records_under_budget` et
`cli_final_ndjson_output_under_budget` :

```bash
rtk run cargo test --release --locked --offline --test performance -- --ignored --nocapture --test-threads=1
```

## 2026-11-12 — Convertisseur à arène unique (`json_to_monty` / `json_into_monty`)

Les deux convertisseurs remplissent une seule arène `MontyGraph` pré-dimensionnée
(`count_nodes`, capacité exacte) au lieu d'une arène par valeur fusionnée niveau
par niveau. Machine : WSL2, 24 cœurs logiques, partagée et chargée (load average
de 4 à 13 pendant les mesures) ; les chiffres sont donc bruités. Commit de base
`3bd778d`, profil standard (`opt-level = "z"`).

Test `value_to_monty_conversion_large_array_under_budget` (100 000 enregistrements,
`json_into_monty` seul). Les premières mesures « avant » et « après » de ce test
ont été faites sous des charges machine différentes et sans alternance : elles
ne sont pas comparables et ne sont pas retenues. Mesures retenues, faites
pendant la relecture avec des répliques non versionnées de ce test :

- Réplique du test avec le convertisseur d'avant (commit `3bd778d`) et le
  nouveau compilés côte à côte, même profil, 20 paires alternées, charge machine
  4,7 à 5,8, **allocateur système** : avant 104,6 ms (99,3 à 117,9), après
  69,8 ms (67,8 à 73,9), gain par paire 33,4 % (27,7 à 40,7).
- Même algorithme sous **mimalloc** (l'allocateur du binaire fimod, via
  `CountingMiMalloc`), dans le
  même processus, avant et après alternés, deux passes, charge machine
  d'environ 12 : avant 74,9 et 67,1 ms, après 34,4 et 30,4 ms, soit environ 55 %.
- Les micro-benchmarks de `tests/performance.rs` lient la bibliothèque sans
  l'allocateur global du binaire : ils tournent sous l'allocateur système et
  leurs ratios diffèrent de ceux du binaire livré.

CLI, filtre sur `users100k.json` (7,87 Mo, 100 000 enregistrements), deux
séries `hyperfine` successives de 9 passes, binaires alternés :

| série | avant | après |
|---|---|---|
| 1 | 270,2 ms ± 15,5 | 225,4 ms ± 25,1 |
| 2 | 270,0 ms ± 20,9 | 243,2 ms ± 20,6 |

Pic mémoire (`/usr/bin/time -f '%e s %M KiB'`, 3 passes) : avant 328 544 à
328 720 KiB, après 248 564 à 248 820 KiB (environ -24 %).

Les commandes ci-dessous reproduisent les mesures CLI et mémoire, ainsi que le
côté « après » du micro-test sous l'allocateur système. Les répliques alternées
ne sont pas versionnées.

```bash
hyperfine -N --warmup 2 --runs 9 \
  "tmp/perf-phase1/fimod-before-wp1 s --sandbox-file= -i tmp/perf-phase1/users100k.json -e '[r for r in data if r[\"active\"]]' --output-format json-compact" \
  "target/release/fimod s --sandbox-file= -i tmp/perf-phase1/users100k.json -e '[r for r in data if r[\"active\"]]' --output-format json-compact"
/usr/bin/time -f '%e s %M KiB' <commande> > /dev/null
cargo test --release --test performance value_to_monty -- --ignored --nocapture
```

## 2026-11-12 — Sortie JSON indentée directe

La sortie JSON indentée (format par défaut) est sérialisée directement depuis la
valeur Monty, comme le JSON compact, uniquement quand le résultat est natif JSON
(`convert::is_json_native`) ; le sérialiseur direct emprunte les clés de
dictionnaire au lieu de les copier. Octets de sortie identiques. Machine : WSL2,
24 cœurs logiques, partagée et chargée (load average de 2,6 à 5,3 pendant les
mesures) ; chiffres bruités. Base : arbre de travail au-dessus de `3bd778d`,
après la correction du stdout fermé, profil standard (`opt-level = "z"`).

CLI, mold identité (`-m tmp/perf-phase1/identity.py`), format par défaut, sur
`users100k.json` (7,87 Mo, 100 000 enregistrements), `hyperfine -N --warmup 3
--runs 21`, médianes (min), binaires alternés :

| série | ordre | avant | après |
|---|---|---|---|
| 1 | avant, après | 349,6 ms (320,3) | 273,8 ms (266,2) |
| 2 | après, avant | 344,5 ms (308,4) | 401,1 ms (317,2), pic isolé à 1 466 ms |
| 3 | avant, après | 329,7 ms (305,8) | 272,3 ms (256,5) |
| 4 (reprise de 2) | après, avant | 324,4 ms (301,5) | 275,9 ms (255,0) |
| 5 | après, avant | 309,0 ms (299,3) | 264,6 ms (254,9) |

Gain sur la médiane : environ 22 % (série 1), 17 % (3), 15 % (4), 14 % (5). La
série 2 est perturbée par un pic de charge pendant la mesure du binaire
« après » (maximum 1 466 ms) et n'est pas exploitable ; elle a été refaite
(série 4).

Pic mémoire (`/usr/bin/time -f '%e s %M KiB'`, 3 passes, identité) : avant
357 164 à 357 392 KiB, après 351 172 à 351 584 KiB (environ -1,6 %).

Filtre `[r for r in data if r["active"]]`, format par défaut, mêmes réglages
`hyperfine` (21 passes, binaires alternés), médianes (min), load average 1,1 à
1,2 : série 1 (avant, après) 255,9 ms (244,9) puis 229,2 ms (209,3) ; série 2
(après, avant) avant 243,8 ms (223,7), après 207,6 ms (200,8) ; série 3 (avant,
après) 251,3 ms (244,5) puis 222,7 ms (217,0). Gain d'environ 10 à 15 %. Pic
mémoire (une passe) : 248 964 KiB avant, 248 640 KiB après.

Test de performance `cli_default_indented_json_output_under_budget` (20 000
enregistrements, mold identité) : stdout 57,6 ms, fichier `-o` 53,9 ms
(budget 300 ms en release).

```bash
hyperfine -N --warmup 3 --runs 21 \
  "tmp/perf-phase1/fimod-before-wp3 s --sandbox-file= -i tmp/perf-phase1/users100k.json -m tmp/perf-phase1/identity.py" \
  "target/release/fimod s --sandbox-file= -i tmp/perf-phase1/users100k.json -m tmp/perf-phase1/identity.py"
/usr/bin/time -f '%e s %M KiB' <commande> > /dev/null
cargo test --release --test performance cli_default_indented -- --ignored --nocapture
```

## 2026-11-12 — Profil standard à `opt-level = "s"`

Passage de `[profile.release]` de `opt-level = "z"` à `"s"` (le profil
`release-fast`, `opt-level = 3`, ne change pas). Arbre de travail basé sur
`3bd778d` avec les changements des sections précédentes du jour.

Tailles (octets) :

| build | `"z"` brut | `"z"` UPX | `"s"` brut | `"s"` UPX |
|---|---|---|---|---|
| natif (glibc), `cargo build --release` | 11 830 368 | non mesuré | 13 307 168 | non mesuré |
| musl, `cargo zigbuild`, distribué | 11 355 792 | 3 802 804 (3,8 Mo) | 12 861 584 | 4 232 824 (4,2 Mo) |

Binaire `fast` natif (`release-fast`, `opt-level = 3`) : 18 773 856 octets,
non compressé. L'affirmation « ~3,3 Mo » de la documentation ne correspondait
déjà plus au profil `"z"` (3,8 Mo mesuré) ; elle devient « ~4,2 Mo ».

Vitesse, filtre `[r for r in data if r["active"]]`, `--output-format
json-compact`, sur `users100k.json` (7,87 Mo, 100 000 enregistrements),
`hyperfine -N --warmup 2 --runs 21 --export-json`, binaires natifs
(`tmp/perf-phase1/fimod-z`, `target/release/fimod`,
`target/release-fast/fimod-fast`), médianes (min). Machine partagée (24 cœurs
logiques, WSL2). `hyperfine` n'entrelace pas les binaires : l'ordre est changé
à chaque série. Load average (1 min) relevé avant et après chaque série.

| série | ordre | load avant / après | standard `"z"` | standard `"s"` | fast |
|---|---|---|---|---|---|
| 1 | z, s, fast | 1,15 / 1,49 | 214,6 ms (209,5) | 166,0 ms (160,8) | 133,5 ms (130,8) |
| 2 | fast, s, z | 1,49 / 1,41 | 214,2 ms (207,9) | 165,4 ms (159,3) | 134,0 ms (131,8) |
| 3 | s, fast, z | 1,41 / 1,32 | 213,4 ms (207,8) | 168,6 ms (164,2) | 136,1 ms (132,2) |
| 4 | z, fast, s | 1,32 / 1,27 | 212,1 ms (206,4) | 165,1 ms (160,3) | 131,7 ms (130,5) |
| 5 | s, z, fast | 1,27 / 1,42 | 213,1 ms (201,4) | 162,2 ms (160,3) | 134,4 ms (130,7) |

Aucune série écartée. Sur les médianes, `"s"` est 21,0 à 23,9 % plus rapide que
`"z"` (séries 3 et 5) ; le build fast est 17,1 à 20,2 % plus rapide que le
standard `"s"` (séries 5 et 4). L'écart fast / standard reste dans la
fourchette 10-30 % : la mention « 15-25 % plus rapide » de la documentation est
conservée. Une première campagne (moyennes, 4 séries, load 2,2 à 1,8) donnait
des valeurs cohérentes mais plus bruitées ; elle est remplacée par celle-ci.

Remarque : entre la mesure des tailles et celle des vitesses, `task
test:performance` et `task test:performance:fast` ont reconstruit
`target/release/fimod` et `target/release-fast/fimod-fast` (13 324 384 et
18 792 928 octets au moment des mesures de vitesse, contre 13 307 168 et
18 773 856 en sortie de `cargo build`). Les tailles du tableau ci-dessus sont
celles des builds `cargo` directs.

```bash
# Avant le changement de profil ("z")
cargo build --release && cp target/release/fimod tmp/perf-phase1/fimod-z
CARGO_ZIGBUILD_ZIG_PATH="$(mise which zig)" mise exec -- cargo zigbuild --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/fimod tmp/perf-phase1/fimod-musl-z
# Après le changement de profil ("s")
cargo build --release
cargo build --profile release-fast --features fast
CARGO_ZIGBUILD_ZIG_PATH="$(mise which zig)" mise exec -- cargo zigbuild --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/fimod tmp/perf-phase1/fimod-musl-s
# UPX sur les copies uniquement
"$(mise which upx)" --best --lzma tmp/perf-phase1/fimod-musl-z
"$(mise which upx)" --best --lzma tmp/perf-phase1/fimod-musl-s
stat -c %s tmp/perf-phase1/fimod-musl-{z,s}
hyperfine -N --warmup 2 --runs 21 --export-json tmp/perf-phase1/wp4-rN.json \
  "tmp/perf-phase1/fimod-z s --sandbox-file= -i tmp/perf-phase1/users100k.json -e '[r for r in data if r[\"active\"]]' --output-format json-compact" \
  "target/release/fimod s ..." "target/release-fast/fimod-fast s ..."
```
