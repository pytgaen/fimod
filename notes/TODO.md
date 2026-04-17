# TOHL — TODO

Reprise demain. Spec actuelle : `notes/tohl-spec.md` (~335 lignes, v0.1 brouillon).

## Prochaines étapes suggérées

### 1. Valider le design sur du concret (avant de compléter la spec)

- [ ] Écrire **3 exemples de configs réelles** en TOHL :
  - Une config Fimod (registry, catalog, etc.)
  - Un équivalent `Cargo.toml`
  - Une config d'app web (server, db, auth, logging)
- [ ] Repérer ce qui coince : syntaxe manquante, ergonomie, ambiguïtés

### 2. Prototyper un parser minimal (Rust)

- [ ] Choisir entre `pest` (déclaratif, grammaire séparée) ou `nom` (combinators)
- [ ] Cible : 200-300 lignes, parse les 7 extensions de la spec
- [ ] Output : `serde_json::Value` (cohérent avec Fimod)
- [ ] Tests sur les 3 exemples ci-dessus

### 3. Compléter la spec v1.0 (après prototype)

Trous identifiés à boucher :

- [ ] **Section "Exemples complets"** — un fichier TOHL réaliste de bout en bout
- [ ] **Règles d'erreur formalisées** :
  - `[.x]` en premier dans un fichier
  - Conflit `vars` / annotation inline
  - Alias non résolu (`port: z` mais `z` absent de `types`)
  - `null` sur champ non-nullable (`port: int = null`)
- [ ] **Interopérabilité** :
  - TOHL → JSON (devenir du frontmatter, perte de types)
  - TOHL → TOML strict (dégradation null/types)
  - JSON → TOHL (types inférés ou `any` ?)
- [ ] **Grammaire EBNF complète** (actuelle = informelle, il manque frontmatter-aliases, compact-section, sibling-shortcut)
- [ ] **Versioning de la spec** — `version = 1` dans frontmatter : sémantique d'évolution
- [ ] **Commentaires** — garder `#` TOML ? Positions autorisées ?
- [ ] **Encoding** — UTF-8 obligatoire ? BOM ? Newlines `\n` vs `\r\n` ?

## État de la discussion (pour reprise à froid)

Décisions prises :

- **Nom** : TOHL — *Typed Obvious Hackable Language*
- **Extension fichier** : `.tohl`
- **Superset strict de TOML 1.0** — jamais de breaking change
- **7 extensions** : frontmatter, null, typage inline optionnel, inline tables typées, raccourci frère `[.x]`, alias (types/chemins/vars), mode compact
- **Type `any`** comme opt-out de `vars`
- **Mode compact** avec `[<d]` (enfant direct du dernier chemin absolu) — outil compress/decompress comme pivot
- **Conflit `vars` vs annotation inline** = erreur

Rejetés (explicitement) :

- Navigation multi-niveaux (`[..x]`, `[...x]`, `[/./.x]`) — coût cognitif > gain
- Anchors/références (`&ref`, `*ref`)
- Expressions/calcul (`port = base_port + 1`)
- Imports entre fichiers
- `:=` comme opt-out (remplacé par `: any`)

## Questions ouvertes

- Fimod comme véhicule d'adoption ? (utiliser TOHL pour `catalog.toml`, config molds, etc.)
- Parser Rust à extraire en crate séparée dès le départ ou laisser dans fimod ?
- Besoin d'un site minimal (tohl-lang.org ?) pour la spec, ou GitHub README suffit au début ?
