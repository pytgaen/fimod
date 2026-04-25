# Output spread — `-o` en miroir de slurp

- **Date**: 2026-04-22
- **Status**: draft (no implementation)
- **Related**: `src/main.rs:679-1000` (logique slurp existante), `src/pipeline.rs:527-576` (`output_result`)

## Motivation

Fimod lit déjà plusieurs sources en une structure unique (`--slurp` + aliases). Le chemin inverse manque : **un mold qui produit plusieurs sorties** depuis une entrée unique (split d'un document, extraction head/body, partition par catégorie).

Aujourd'hui on contourne avec deux invocations + intermédiaire JSON ; on veut la même ergonomie qu'à l'entrée.

## Principe

**Tout passe par `-o`.** Zéro nouveau flag. fimod dispatche statiquement sur la forme de la CLI, avant d'exécuter le mold. La forme attendue du résultat se lit directement sur l'invocation.

Symétrie exacte avec `-i` :

| Entrée (slurp) | Sortie (spread) |
|---|---|
| `-i a -i b --slurp` → `data = [va, vb]` | `-o a -o b` ← mold retourne `[...]` |
| `-i x:a -i y:b --slurp` → `data = {x, y}` | `-o x:a -o y:b` ← mold retourne `{...}` |

Plus une variante qui n'existe pas côté entrée (cardinalité dynamique) : le template.

## Les 4 modes de `-o`

```bash
# (1) Classique — comportement actuel, inchangé
fimod data.json -e @t -o out.json

# (2) Positionnel — mold retourne une liste
fimod page.html -e @split -o head.txt -o body.html -o footer.md

# (3) Nommé — mold retourne un dict, clés connues à la CLI
fimod page.html -e @split -o head:a.txt -o body:core.html

# (4) Template — mold retourne un dict, N inconnu à la CLI
fimod events.ndjson -e @partition -o '{key}.toml'
```

### Dispatch (lu avant exécution du mold)

| Forme CLI | Mode | Attendu du mold |
|---|---|---|
| 1× `-o path` sans placeholder | classique | valeur unique |
| N× `-o path` sans `:` | positionnel | liste de longueur N |
| N× `-o k:path` (tous aliasés) | nommé | dict avec clés = alias |
| 1× `-o` contenant `{key}` | template | dict non-vide |

### Le placeholder `{key}`

- Reconnu **littéralement** : le mode template s'active uniquement si le path contient le token exact `{key}`.
- Pas de moteur d'expressions. Pas d'interpolation depuis les champs des valeurs.
- Extension future possible : `{idx}` pour le pendant dynamique du positionnel (`-o 'part-{idx}.ndjson'` + mold retourne liste). Hors scope v1.

## Règles de validation

### Parsing CLI (bail immédiat)

- **All-or-nothing aliases** (réutilise `parse_input_entry` et la règle de `main.rs:924-930`) : `-o a -o b:x` → erreur "all `-o` must use `:` or none must".
- **Template exclusif** : `-o '{key}.json' -o foo.txt` → erreur "template `-o` cannot be combined with other `-o` flags".
- **Clés dupliquées** (mode nommé) → erreur "duplicate key".
- **Path vide** côté gauche ou droite de `:` → erreur.

### Après exécution du mold (bail avec message ciblé)

- **Positionnel** : si le retour n'est pas une liste, ou si `len != N`, erreur précise (`expected list of 3, got dict` / `got list of 5`).
- **Nommé** : si le retour n'est pas un dict, ou si les clés ne matchent pas **exactement** les alias (ni extra, ni missing), erreur listant les différences.
- **Template** : si le retour n'est pas un dict ou est vide, erreur.
- **Retour `None`/`Null`** en mode spread → erreur (pas d'écriture silencieuse de fichiers vides).

Strict par défaut. Relâcher plus tard si un vrai cas d'usage le demande.

## Format par fichier

- Chaque `-o` décide son format via **son extension** (comportement `resolve_format` actuel).
- `--output-format X` reste un fallback global si une extension est absente/inconnue.
- En mode template, une seule extension → format uniforme sur toutes les sorties. C'est cohérent (un template = une intention).

Conséquence : mode nommé = **formats hétérogènes gratuits** (`.txt` + `.html` + `.yaml` dans le même run).

## Interactions

### `--in-place`
Interdit en modes 2/3/4. L'input est unique, les sorties multiples — `--in-place` n'a pas de sens. Message : "--in-place is incompatible with multi-output".

### `-o dir/`
Interdit en modes 2/3. Chaque `-o` doit pointer un fichier. (Dir ciblée = mode batch côté entrée, pas côté sortie.)

### Multi `-i` (batch 1-to-1)
Aujourd'hui : `-i a -i b -o dir/` = mold exécuté par fichier, basename préservé (`main.rs:1026-1058`). Cette sémantique reste.

- Multi `-i` **sans** slurp + spread (modes 2/3/4) → **bail**. Combinatoire ambiguë.
- Multi `-i` **avec** `--slurp` + spread → **OK**. C'est le cas le plus élégant : slurp côté in, spread côté out, parfaitement symétrique. Le mold voit `data = [...]` ou `{...}` et produit en retour une autre structure multiple.

### `--no-input` + spread
OK. Mold génère from scratch, spread s'applique comme d'habitude.

### `--check`
En spread, `--check` valide la forme du retour (liste/dict + cardinalité/clés) sans rien écrire.

### `set_output_file()` (API mold, `main.rs:992`)
Actuellement un mold peut override `-o` dynamiquement. En mode spread, soit :
- **(A)** l'accepter avec un dict/liste du même shape que les `-o` CLI, soit
- **(B)** l'interdire et faire bail.

**Choix v1 : (B), interdit.** Un seul chemin de décision pour les sorties multiples. On rouvrira si besoin.

### CSV en spread
Chaque output CSV utilise les `csv_opts` globales (`--csv-delimiter`, etc.). Pas de CSV-opts par fichier en v1. Si un seul `-o` est `.csv` et les autres JSON, seules les options CSV s'appliquent aux CSV, les JSON ignorent.

### Molds chaînés (`-e @a -e @b`)
Aucun impact. Le spread s'applique **au résultat final** de la chaîne. Les étapes intermédiaires restent des `Value` Python transmis entre molds, inchangé.

## Exemples complets

### Split d'un HTML en parties logiques (mode nommé)

```bash
fimod page.html --input-format html -e @extract \
  -o head:page-head.txt \
  -o body:page-body.html \
  -o footer:page-footer.md
```

Mold :
```python
def transform(data, **_):
    return {
        "head": extract_head_text(data),
        "body": extract_body_html(data),
        "footer": extract_footer_md(data),
    }
```

### Fan-out positionnel (splitting une liste en N premiers éléments)

```bash
fimod ranking.json -e 'data[:3]' -o first.json -o second.json -o third.json
```

### Partition par catégorie, N inconnu (mode template)

```bash
fimod events.ndjson -e 'groupby(data, "level")' -o 'logs/{key}.ndjson'
# → logs/info.ndjson, logs/warn.ndjson, logs/error.ndjson (selon données)
```

### Slurp + spread (N→M via mold unique)

```bash
fimod -i fr:fr.yaml -i en:en.yaml --slurp -e @merge-i18n \
  -o common:common.json \
  -o fr-specific:fr-only.json \
  -o en-specific:en-only.json
```

Le mold voit `data = {"fr": {...}, "en": {...}}` et produit `{"common": {...}, "fr-specific": {...}, "en-specific": {...}}`.

## Messages d'erreur (templates)

Formulation uniforme, qui pointe le mode détecté et l'écart constaté :

- `spread mode (positional): expected list of N values from mold, got <type>` (avec len si liste)
- `spread mode (named): missing keys: [a, b]; extra keys: [c]`
- `spread mode (template): expected non-empty dict from mold, got <type>`
- `-o flags: mix of ':' and non-':' forms is not allowed — use all aliased or none`
- `-o flags: duplicate key 'users'`
- `-o flags: template '{key}.toml' cannot be combined with other -o`

## Hors scope v1

- **`{idx}` template** pour listes dynamiques.
- **`--partition <key>`** déclaratif sans mold (garder en tête pour plus tard, même chemin d'écriture sous-jacent).
- **Format archive** (`-o bundle.tar.gz` qui empaquète un dict) — élégant mais orthogonal.
- **Écriture partielle sur erreur** : si le mold lance une exception après avoir produit un dict partiel, on n'écrit rien (comportement tout-ou-rien). La question du "flush progressif" en mode template reste ouverte si on veut plus tard streamer.
- **Sortie vers `stdout` partiel** (`-o body:-`) : sémantiquement OK mais à confirmer (risque de mélange avec les logs debug stdout). Probablement OK puisque `--debug` va déjà sur stderr.

## Questions ouvertes

1. **`set_output_file()` en mode spread** — figé à "interdit" ici, mais si quelqu'un a un cas d'usage concret (notamment molds qui décident dynamiquement du layout), rouvrir.
2. **`--check` sur spread** — valider shape+cardinalité seulement, ou aussi tenter la sérialisation de chaque part ? Sérialiser donne plus de garanties, coûte plus.
3. **Ordre d'écriture** — importe peu sauf si l'un des `-o` est `-` (stdout). Proposer : même ordre que les `-o` sur la CLI (mode nommé/positionnel) ou ordre lexico des clés (template).
4. **Placeholder `{key}` littéral dans un nom de fichier** — pathologique, ignoré pour v1. Si besoin, introduire `{{` d'échappement comme en format strings Python.

## Plan d'implémentation (esquisse, pour discussion séparée)

1. Parsing CLI : factoriser `parse_input_entry` → `parse_output_entry`, ou partager la même fonction.
2. Enum `OutputMode { Single, Positional(Vec<Path>), Named(Vec<(Alias, Path)>), Template(PathTemplate) }`.
3. Validation statique dans le parseur CLI (mix `:`, duplicatas, template exclusif).
4. Dans `output_result` (ou nouveau `output_multi_result`) : switch sur `OutputMode`, valide le shape du résultat, écrit chaque part via la logique existante.
5. Tests d'intégration : un fichier par mode + matrice des interdictions (`--in-place`, multi `-i` sans slurp, `set_output_file`).
6. Doc : `docs/guides/cli-reference.md` section "Multi-output" symétrique à la section "Slurp mode".
