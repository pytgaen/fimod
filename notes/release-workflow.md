# Release Workflow

Ce document décrit le cycle de release de Fimod : conventions, outils, invariants. Il s'adresse aux mainteneurs et contributeurs qui proposent des PR ou coupent des versions.

## TL;DR

1. Le travail se fait sur une branche, proposé via PR, mergé en **squash** sur `main`.
2. Le titre + body de la PR sont rédigés comme des commits conventionnels ; ils pilotent la génération automatique du CHANGELOG.
3. Une release est un commit `chore(release): X.Y.Z` **direct sur main** (pas de PR), accompagné d'un tag `vX.Y.Z`.
4. Une prerelease publique (`rc.N`) valide le pipeline de distribution avant la release finale.

---

## Conventions de commits

Fimod suit [Conventional Commits](https://www.conventionalcommits.org/) (norme Angular, compatible [semantic-release](https://semantic-release.gitbook.io/)).

**Types reconnus** (impact sur CHANGELOG et bump semver) :

| Type | Section CHANGELOG | Bump |
|---|---|---|
| `feat` | Features | minor |
| `fix` | Bug Fixes | patch |
| `perf` | Performance | patch |
| `docs` | Documentation | — |
| `refactor` | Refactoring | — |
| `chore` | Housekeeping | — |
| `ci`, `test`, `style`, `build` | *(skippé)* | — |

**BREAKING CHANGE** (via `!` dans le type ou footer `BREAKING CHANGE:`) :

- En 0.x.y (pré-1.0) : bump **minor** (0.4.x → 0.5.0). Convention projet, conforme à [semver §4](https://semver.org/#spec-item-4).
- À partir de 1.0.0 : bump **major**.

Exemples :

```
feat(built-ins): add dp_has and dp_delete functions
fix(registry): handle missing catalog.toml gracefully
perf(mold): cache parsed docstrings across invocations
docs(cli-reference): document --env glob patterns
chore(release): 0.4.1
```

---

## Fil conducteur — comment le CHANGELOG se construit

Fimod utilise **squash & merge** sur GitHub (pas de merge commits). Le défi : garder un CHANGELOG riche malgré la perte de granularité du squash.

**Solution** : rédiger le body de PR comme une liste de commits conventionnels atomiques. [git-cliff](https://git-cliff.org/) avec `split_commits = true` parse chaque ligne comme un commit indépendant.

### 1 PR = 1 intention sémantique

- **Titre de PR** : subject conventionnel court et éditorial
  ```
  feat(built-ins): add dotpath test/delete functions
  ```
- **Body de PR** : bullets conventionnels, un par changement atomique
  ```markdown
  - feat(built-ins): dp_has(data, path) tests path existence
  - feat(built-ins): dp_delete(data, path) removes key/index
  - feat(iter): it_count_by(array, key) returns counts per field value
  - fix(serde): wrap Value in NativeNumbers before non-serde_json serializers
  - docs: update built-ins.md
  ```
- **Squash & merge** avec l'option GitHub *"Default to pull request title and description"* → le commit sur `main` contient subject + body structuré.
- **git-cliff** regroupe automatiquement les entries par section lors de la génération du CHANGELOG.

Les bullets peuvent répéter/détailler le titre de PR — c'est voulu, l'un sert de titre éditorial court, l'autre de log détaillé pour le CHANGELOG.

### Highlights éditoriaux

Au fil du développement d'une version, un fichier `notes/release-vX.Y.Z.md` est rédigé pour capturer les **Highlights** (prose éditoriale, emojis autorisés). Exemple :

```markdown
- Monty v0.0.11 → v0.0.14 — natural JSON support, u32 CodeLoc fix.
- New dotpath built-ins — `dp_has`, `dp_delete` complete the toolkit.
```

Lors de la release, ce contenu est injecté en tête de section dans le CHANGELOG, juste après la date. Le fichier `notes/release-vX.Y.Z.md` est supprimé dans le même commit (le contenu vit désormais dans `CHANGELOG.md`).

---

## Cycle de release

### Phase 1 — Travail (itératif, via PR)

1. Créer une branche (`feat/...`, `fix/...`, `release/X.Y.Z`).
2. Commiter normalement (les commits intra-branche sont squashés).
3. Ne **jamais** modifier `CHANGELOG.md` dans cette phase.
4. Ouvrir la PR avec titre + body conventionnels (voir fil conducteur).
5. Attendre CI verte.
6. Merger en **squash** (option *"pull request title and description"*).

### Phase 2 — Release (direct sur main)

1. Switch sur `main`, `git pull --ff-only` (garantit un historique linéaire).
2. Vérifier working tree clean.
3. Analyser les commits depuis le dernier tag : déterminer le bump (patch/minor/major, cf. règles ci-dessus).
4. Bump `Cargo.toml`, rebuild `Cargo.lock` (`cargo build`).
5. Smoke test : `cargo test --lib`.
6. Générer le CHANGELOG :
   ```bash
   git-cliff --unreleased --tag vX.Y.Z --prepend CHANGELOG.md
   ```
7. Si `notes/release-vX.Y.Z.md` existe : injecter son contenu comme sous-section `### Highlights` en tête de la section `[X.Y.Z]`, puis supprimer le fichier.
8. Commit EXACTEMENT ces 3 fichiers (+ éventuelle suppression de `notes/release-vX.Y.Z.md`) :
   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git add -u notes/release-vX.Y.Z.md   # si supprimé
   git commit -m "chore(release): X.Y.Z"
   git tag vX.Y.Z
   ```
9. Push avec confirmation :
   ```bash
   git push && git push --tags
   ```

Le push du tag déclenche `.github/workflows/release.yml` (build multi-plateformes, checksums, GitHub Release).

---

## Prerelease publique (rc.N)

Utilisée **avant** une release majeure (typiquement X.Y.0) pour valider le pipeline de distribution public — `install.sh`, checksums SHA256, migration de registry, etc.

### Différences avec une release

|  | Prerelease (`vX.Y.Z-rc.N`) | Release (`vX.Y.Z`) |
|---|---|---|
| **But** | Valider la distribution publique | Publication officielle |
| **CHANGELOG.md** | Non modifié | Section générée par git-cliff |
| **Commit** | `chore(prerelease): X.Y.Z-rc.N` | `chore(release): X.Y.Z` |
| **Plateformes** | Linux x86_64 (musl) | Toutes (Linux x86/ARM, macOS ARM, Windows x64) |
| **Variants** | `standard` uniquement | `standard` + `slim` |
| **Binaire compressé (UPX)** | Non | Oui |
| **CI** | `.github/workflows/prerelease.yml` | `.github/workflows/release.yml` |
| **GitHub Release** | Marquée `prerelease: true` | Stable |
| **Branche** | Quelconque (y compris non-main) | `main` uniquement |

### Cycle prerelease

1. Bump `Cargo.toml` à `X.Y.Z-rc.N` (séparateur point, conforme semver).
2. `cargo build` pour régénérer `Cargo.lock`.
3. Commit `chore(prerelease): X.Y.Z-rc.N` (les 2 fichiers Cargo uniquement).
4. Tag `vX.Y.Z-rc.N`.
5. Push → déclenche `prerelease.yml` → build + GitHub Prerelease + job e2e-install (installe via `install.sh`, vérifie la version, teste la migration de registry, smoke test).

**Important** : les commits `chore(prerelease):` sont **skippés** par `git-cliff` (configuration `cliff.toml`) — ils n'apparaîtront jamais dans le `CHANGELOG.md` final.

### Convention numérotation rc

- `rc.N` avec séparateur **point**, jamais tiret (`rc.1`, `rc.2`, ...) — conforme [semver §9](https://semver.org/#spec-item-9).
- Numérotation indépendante par version cible : `0.5.0-rc.1`, `0.5.0-rc.2`, puis `0.5.0`.
- Une `rc.N` peut vivre sur une branche de feature (pas besoin de merger sur main avant de tagguer).

---

## Invariants critiques

À respecter absolument sous peine de corrompre le CHANGELOG ou l'historique :

1. `CHANGELOG.md` n'apparaît **JAMAIS** dans un commit non-`chore(release):`. Toute modif accidentelle doit être stashée jusqu'en Phase 2.
2. Le commit `chore(release):` contient **EXACTEMENT** 3 fichiers : `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` (plus une éventuelle suppression de `notes/release-vX.Y.Z.md`). Tout autre fichier → STOP.
3. Aucun tag n'est créé avant que la PR de travail soit mergée sur `main`.
4. Aucun commit direct sur `main` en Phase 1 — tout passe par une PR.
5. Subject du commit release **EXACTEMENT** `chore(release): X.Y.Z` — jamais `fix:`, `feat:`, etc.
6. Squash uniquement — pas de merge commits (historique linéaire requis pour `git-cliff`).

---

## Outillage

- **[git-cliff](https://git-cliff.org/)** — génération CHANGELOG depuis l'historique git. Installation : `cargo install git-cliff --locked`. Config : `cliff.toml`.
- **GitHub squash-merge** — activer l'option *"Default to pull request title and description"* dans les settings du repo (sinon le body disparaît du commit squashé).
- **CI releases** — `.github/workflows/release.yml` (trigger `v[0-9]+.[0-9]+.[0-9]+`), `.github/workflows/prerelease.yml` (trigger `v*-rc.*`).

---

## Fichiers de référence

- `cliff.toml` — configuration git-cliff (types, groupes, templates, skip rules).
- `CHANGELOG.md` — historique public, maintenu automatiquement.
- `notes/release-vX.Y.Z.md` — Highlights éditoriaux (temporaire, supprimé à la release).
- `.github/workflows/release.yml` — pipeline release.
- `.github/workflows/prerelease.yml` — pipeline prerelease.
