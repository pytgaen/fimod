# fix-0.6.0.md — Corrections identifiées (revue pipeline API)

## Bugs confirmés

### B1 — Messages d'erreur périmés dans `set_step_field` (5 endroits)
**Fichier :** `src/engine.rs` lignes 419, 422, 431, 437, 444

Les messages utilisent l'ancienne API (`PipelineStep`, `step['exit']`) au lieu de la nouvelle (`Step.set()`).

```
L419: "PipelineStep field '{key}' is read-only"
        → "Step.set('{key}'): field is read-only"
L422: "PipelineStep has no field '{key}'"
        → "Step.set('{key}'): unknown field"
L431: "step['exit'] must be an integer"
        → "Step.set('exit', N): value must be an integer"
L437: "step['output_format'] must be a string"
        → "Step.set('output_format', ...): value must be a string"
L444: "step['output_file'] must be a string"
        → "Step.set('output_file', ...): value must be a string"
```

---

### B2 — Mutation `output_file` sur step futur silencieusement perdue
**Fichiers :** `src/engine.rs:415`, `src/pipeline.rs:142-194`

`WRITABLE` inclut `"output_file"`. Quand un step i appelle `pipeline.step(j).set('output_file', ...)`,
la mutation entre dans `pending_mutations[j]["output_file"]`. Dans `execute_chain` :
- `mutations.get(&j)` ne lit que `input_format` et `output_format`
- `mutations.remove(&j)` ne traite que `exit`

La mutation est silencieusement jetée. Deux options :
- (a) Errorer explicitement : `"Step.set('output_file', ...): can only be set on the current step"`
- (b) Implémenter le support réel dans `execute_chain` (passer `output_file` via `MoldOptions`)

Option (a) recommandée — (b) implique des changements structurels importants pour un cas d'usage marginal.

---

### ~~B3~~ — RETIRÉ : `current_step().set('input_format', ...)` fonctionne déjà

**Statut :** non-bug (analyse initiale incorrecte).

**Preuve empirique :** test `test_b3_step_set_input_format_current` (variante step-API) et
`test_b3_baseline_set_input_format_global` (variante fonction globale) produisent **la même
sortie** (`84\n`) sur la branche actuelle. Le re-parse a donc bien lieu via le step-API,
contrairement à ce que cette section affirmait.

**Hypothèse :** le `set('output_format', 'yaml')` (qui marche, écrit dans `format_override`)
déclenche déjà la re-sérialisation entre steps, et le re-parse côté input du step suivant
utilise ce format. Le `input_format` mutation ne change donc rien d'observable dans ce
scénario.

Tests retirés du fichier pour ne pas garder de tests verts qui n'apportent aucune garantie.

---

### B4 — `pipeline.step(-1)` : overflow usize, message d'erreur trompeur
**Fichier :** `src/engine.rs:296`

`extract_int_arg` retourne `i64`, puis cast `as usize`. `-1i64 as usize` = `18446744073709551615`.
L'erreur "index out of range (total=3)" affiche un index absurde.

Fix : valider `idx >= 0` avant le cast et errorer avec `"pipeline.step(): index must be non-negative"`.

---

### B5 — Attributs `in_place`, `slurp`, `no_input` faux sur les steps futurs
**Fichier :** `src/engine.rs:236` (`build_future_step_dc`)

Ces trois attributs sont hardcodés à `false`/`false`/`false` pour tous les steps futurs,
indépendamment de `--in-place`, `--slurp`, etc. Ce sont des options pipeline-wide qui devraient
être propagées depuis le contexte.

Fix : passer ces valeurs via `MoldContext` et les inclure dans `build_future_step_dc`.

---

## Problèmes de conception

### C1 — Pas de garde de type sur les méthodes pipeline
**Fichier :** `src/engine.rs:288` (`dispatch_method`)

`dispatch_method` route sur le nom de la méthode sans vérifier que `args[0]` est bien
du bon type. Cas problématiques :

```python
step = pipeline.current_step()
step.length()          # retourne ctx.total_steps — faux
step.insert_next(...)  # injecte un step depuis un Step — absurde

spec = Step.create(expr="data * 2")
spec.current_step()    # retourne le step courant — absurde
```

Seuls `create` (vérifie `STEP_CLASS_TYPE_ID`) et `set` (vérifie `_step_idx`) font la vérification.

Fix : ajouter des gardes de type :
- Méthodes pipeline (`current_step`, `step`, `length`, `insert_next`, `append`) : vérifier `PIPELINE_TYPE_ID`
- Méthode `set` : déjà partiellement gardée via `get_step_idx`

---

### C2 — `set('output_format', ...)` : sémantique différente current vs futur
**Fichiers :** `src/engine.rs:425`, `src/pipeline.rs:148`

Sur le **step courant** : `step.set('output_format', 'json')` → `ctx.format_override` → sérialisation effective.

Sur un **step futur** : la mutation aboutit dans `opts.output_format` → `ctx.output_format` → uniquement
l'attribut lisible `step.output_format`. Elle ne passe jamais dans `ctx.format_override`, donc ne change
pas la sérialisation réelle. Le comportement est radicalement différent, en silence.

Fix envisageable : dans `execute_chain`, quand `mutations[j]["output_format"]` existe, le stocker
séparément et l'injecter automatiquement comme `format_override` au moment où le step j termine
(au lieu de le passer uniquement comme attribut lisible).

---

### C3 — Double chemin de création de step, un seul documenté
**Fichier :** `src/engine.rs:355` (`extract_step_spec`)

`insert_next` et `append` acceptent deux formes :

```python
pipeline.insert_next(Step.create(expr="data * 2"))  # documenté
pipeline.insert_next(expr="data * 2")               # shortcut silencieux via kwargs
```

Le path kwargs existe indépendamment de `Step.create`. À trancher : documenter ou supprimer.

---

### C4 — `pipeline.length()` non mis à jour dans le step qui injecte
**Fichier :** `src/engine.rs:324`, `src/pipeline.rs:165`

Après `pipeline.insert_next(...)`, `pipeline.length()` retourne l'ancienne valeur pour le reste
de l'exécution du mold courant. La mise à jour est visible uniquement depuis le step suivant.

Ce n'est pas un bug (le runner recalcule au début de chaque itération), mais c'est surprenant.
Fix : documenter ce comportement dans `docs/guides/mold-scripting.md`.

---

## Cosmétique / Nommage

### N1 — Commentaire périmé ligne 286
**Fichier :** `src/engine.rs:286`

```rust
/// Dispatch a method call on a Pipeline or PipelineStep Dataclass.
```
→ `"Pipeline, Step instance, or Step class Dataclass"`

---

### N2 — Nom de test périmé
**Fichier :** `tests/cli/pipeline.rs:241`

```rust
fn test_pipe_missing_mold_or_expr_error()
```
→ `test_step_create_missing_spec_error`

---

## Molds

### M1 — `sample_if_large.py` : validation absente sur l'arg `max`
**Fichier :** `molds/sample_if_large/sample_if_large.py:17`

```python
max_items = int(args["max"])  # KeyError ou ValueError opaques si arg absent ou invalide
```

Fix : valider explicitement avec un message lisible :

```python
if "max" not in args:
    msg_error("sample_if_large: --arg max=N is required")
    pipeline.current_step().set('exit', 1)
    return data
try:
    max_items = int(args["max"])
except ValueError:
    msg_error(f"sample_if_large: max must be an integer, got: {args['max']!r}")
    pipeline.current_step().set('exit', 1)
    return data
```

---

## Fichiers à mettre à jour

| Fichier | Raison |
|---------|--------|
| `notes/changelog-0.6.0.md` | Complètement obsolète — montre `Pipe()`, `step['key']`, ancienne API |
