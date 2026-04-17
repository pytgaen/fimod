# TOHL — Typed Obvious Hackable Language

*Spec brouillon.*

Extension minimale de TOML visant les 5% de manques réels, sans trahir la philosophie "lisibilité locale + chemins absolus". Le nom évoque TOML (filiation auditive) et remplace *Minimal* par **Hackable** pour signaler l'extensibilité (frontmatter, types inline, raccourcis).

> *TOHL, le TOML typé et hackable.*

**Statut** : exploration — aucune implémentation, aucun engagement.
**Base** : [TOML 1.0.0](https://toml.io/en/v1.0.0) — tout TOML valide reste valide.

## Principes directeurs

1. **Tout TOML 1.0 reste valide** — superset strict, jamais de breaking change.
2. **Une extension = un cas d'usage à 80%+** — pas de feature "cool mais rare".
3. **Spec courte** — doit tenir en < 10 pages pour garder des parsers réalistes.
4. **Lisibilité locale préservée** — une section se comprend avec au plus *la section précédente* comme contexte.
5. **Round-trip JSON bidirectionnel sans perte** (d'où `null`).

## Extensions

### 1. Frontmatter (`---`)

Bloc de métadonnées optionnel en tête de fichier, délimité par `---`. Contient du TOML standard.

```tohl
---
schema = "./user.schema.tohl"
version = 2
fimod = { mold = "@pick_fields", args = { fields = ["name", "email"] } }
---

name = "bob"
email = "b@x"
```

- **Optionnel** : un fichier sans frontmatter reste valide.
- **Format** : TOML (cohérence avec le corps, pas de second parser).
- **Sémantique** : purement métadonnée — n'affecte pas la structure de données.
- **Convention** : librement inspirée de Markdown/Jekyll/Hugo.

### 2. `null` explicite

Valeur `null` autorisée pour tous les types.

```tohl
tls = null
deleted_at = null
```

- Distingue **champ absent** (clé non déclarée) de **champ explicitement null**.
- Indispensable pour round-trip JSON et patches (RFC 7396 merge patch).
- Sérialisation : littéral `null` (comme JSON).

### 3. Typage inline optionnel

Annotation de type après la clé, via `:`. Optionnelle — sans annotation, inférence comme TOML standard.

```tohl
port: int = 8080
name: string = "fimod"
tags: list<string> = ["a", "b"]
ratio: float = 0.5
created: date = 2026-04-14
tls: bool? = null              # ? = nullable
```

#### Types de base

- `int`, `float`, `bool`, `string`
- `date`, `time`, `datetime` (formats ISO natifs TOML)
- `list<T>`, `map<K, V>`
- `any` — bypass validation ; utile pour opt-out explicite d'un type déclaré dans `vars`
- Suffixe `?` = nullable (accepte `null`)

#### Sémantique

- **Validation stricte** : `port: int = "8080"` → erreur de parsing.
- **Pas de coercition implicite**.
- **Rétrocompatible** : omettre le type donne le comportement TOML actuel.

#### Format de sérialisation inline (optionnel, avancé)

```tohl
created: date("yyyyMMdd") = "20260704"
```

Permet de parser/sérialiser des formats non-ISO sans perdre le type logique. À considérer si le cas d'usage se présente — pas dans la v1 minimale.

### 4. Inline tables typées imbriquées

Les inline tables TOML sont imbriquables et supportent le typage inline :

```tohl
a = { b = { c: int = 4, d: string? = null } }
server = { tls = { cert: string = "./cert.pem", expires: date = 2027-01-01 } }
```

Équivalent strict aux sections `[a.b]` + clés typées — choix purement stylistique (compact vs documenté).

#### Équivalences

Les quatre formes suivantes produisent **exactement le même arbre** (`server.api` + `server.tls`, chacun avec 3 clés) :

```tohl
# Forme 1 : inline imbriqué (compact)
server = {
  api = { port: int = 8080, timeout: int = 30, cors: bool = true },
  tls = { cert: string = "./cert.pem", key: string = "./key.pem", expires: date = 2027-01-01 },
}
```

```tohl
# Forme 2 : sections absolues (verbeux mais explicite)
[server.api]
  port: int = 8080
  timeout: int = 30
  cors: bool = true
[server.tls]
  cert: string = "./cert.pem"
  key: string = "./key.pem"
  expires: date = 2027-01-01
```

```tohl
# Forme 3 : section parente + inline enfants (hybride)
[server]
  api = { port: int = 8080, timeout: int = 30, cors: bool = true }
  tls = { cert: string = "./cert.pem", key: string = "./key.pem", expires: date = 2027-01-01 }
```

```tohl
# Forme 4 : sections + raccourci frère (concis, TOHL uniquement)
[server.api]
  port: int = 8080
  timeout: int = 30
  cors: bool = true
[.tls]                          # = server.tls (frère de api)
  cert: string = "./cert.pem"
  key: string = "./key.pem"
  expires: date = 2027-01-01
```

- **Forme 1 (inline imbriqué)** : compact, tout d'un bloc — pour petits arbres.
- **Forme 2 (sections absolues)** : explicite, chaque section indépendante et copiable n'importe où.
- **Forme 3 (hybride)** : section parente + inline enfants — bon compromis quand les enfants tiennent sur une ligne.
- **Forme 4 (`[.x]`)** : concise, idéale pour enchaîner plusieurs frères sans répéter le préfixe.

### 5. Raccourci frère `[.x]`

Un point en tête de section = "remplace le dernier segment du chemin précédent par `x`".

```tohl
[server.api.v1]
  port = 8080
[.v2]                   # server.api.v2
  port = 8081
[.v3]                   # server.api.v3
  port = 8082

[server.database]       # absolu, nouveau sous-arbre
  url = "..."
[.cache]                # server.cache
  ttl = 60
```

**Règle unique** : `[.x]` = `[<préfixe-du-chemin-absolu-précédent>.x]`.

- **Un seul niveau** de relatif (le dernier segment). Pas de `..`, pas de `$`, pas de navigation multi-niveaux.
- **Nécessite un chemin absolu précédent** : `[.x]` en premier dans un fichier = erreur.
- **Les chemins absolus restent toujours valides** — `[.x]` est du sucre, pas une obligation.

#### Non autorisé

```tohl
[.x.y]                  # interdit : pas de multi-segment relatif en v1
[..x]                   # interdit : pas de remontée
```

**Justification** : couvre le cas à 90% (frères contigus) avec un seul caractère de surcoût, sans introduire d'état de navigation complexe.

### 6. Alias (frontmatter)

Tables de substitution déclarées dans le frontmatter pour réduire la répétition. Résolues au parsing, transparentes pour le modèle de données final.

#### Alias de types

```tohl
---
types = { s = "string", i = "int", d = "date", b = "bool", f = "float" }
---
[server.api]
  port: i = 8080
  host: s = "localhost"
  debug: b = false
  created: d = 2026-04-14
```

Résolution : `port: i` → `port: int`. Si la clé n'existe pas dans `types`, fallback sur les types natifs.

#### Alias de chemins

```tohl
---
paths = { s = "server", sa = "server.api", st = "server.tls" }
---
[sa]                       # server.api
  port: i = 8080
[sa.regions.eu]            # server.api.regions.eu
  primary: b = true
[st]                       # server.tls
  cert: s = "./cert.pem"
```

Résolution : préfixe le plus long d'abord (`sa.regions.eu` → `server.api.regions.eu`, pas `s.a.regions.eu`).

#### Typage par variable (`vars`)

Déclare le type de variables par leur nom, appliqué automatiquement à toutes les occurrences dans le corps. Schéma léger inline, sans JSON Schema externe.

```tohl
---
types = { i = "int", s = "string", b = "bool" }
vars = { port = "i", host = "s", enabled = "b" }
---
[server.api]
  port = 8080            # typé int via vars
  host = "localhost"
  enabled = true

[server.admin]
  port = 9090            # même nom → même type garanti
  host = "127.0.0.1"
  enabled = false
```

- **Résolution en cascade** : `vars` → `types` → types natifs.
- **Conflit avec annotation inline** : erreur (deux sources de vérité en conflit).
- **Non-déclarées** : inférence TOML standard (rétrocompatible).
- **Portée** : globale au fichier — si tu veux des `port` de types différents dans des sous-arbres, annote inline (et ne mets pas `port` dans `vars`).

### 7. Mode compact (sérialisation dense)

Forme alternative **sémantiquement identique** au mode humain, optimisée pour la taille et le nombre de tokens (transmission, stockage, contexte LLM).

**Principe** : un outil `tohl-compress` / `tohl-decompress` convertit dans les deux sens. On édite en TOHL-human, on transmet en TOHL-compact.

#### Règles de compression

1. **Pas d'indentation, pas de lignes vides** (sauf frontmatter délimiters).
2. **`=` sans espaces** : `port:i=8080`.
3. **`;` comme séparateur de clés sur une ligne** : `[sa]port:i=8080;host:s="localhost"`.
4. **Alias types + chemins** appliqués systématiquement (étendre le frontmatter avec les paires optimales détectées).
5. **Raccourci enfant `[<d]`** : "enfant direct du dernier **chemin absolu** déclaré" (pas du dernier `[.x]` ou `[<x]`).

#### Raccourci `[<d]` (compact uniquement)

```tohl
[server]host:s="localhost"
[<api]port:i=8080;cors:b=true      # server.api
[<tls]cert:s="./c.pem"             # server.tls (enfant de server, pas de api)
```

**Règle stricte** : `[<d]` résout toujours depuis le dernier chemin **absolu** (`[server]` ici), jamais depuis une section relative. Garantit une décompression mécanique sans ambiguïté.

#### Exemple avant/après

```tohl
---
types = { s = "string", i = "int", b = "bool" }
---
# human
[server.api]
  port: int = 8080
  timeout: int = 30
  cors: bool = true

[server.tls]
  cert: string = "./cert.pem"
  enabled: bool = true
```

```tohl
---
types={s="string",i="int",b="bool"}
paths={s="server"}
---
[s.api]port:i=8080;timeout:i=30;cors:b=true
[<tls]cert:s="./cert.pem";enabled:b=true
```

Réduction ~50% en bytes, ~40% en tokens LLM (estimation). Mode **optionnel** — le mode humain reste la forme canonique pour l'édition.

## Grammaire (informelle)

```ebnf
file         := frontmatter? body
frontmatter  := "---" NEWLINE toml-content "---" NEWLINE
body         := toml-content-with-extensions

section      := "[" (relative-path | absolute-path) "]"
relative-path := "." IDENT
absolute-path := IDENT ("." IDENT)*

key-value    := IDENT (":" type)? "=" value
type         := base-type "?"?
base-type    := "int" | "float" | "bool" | "string"
              | "date" | "time" | "datetime"
              | "list" "<" type ">"
              | "map" "<" type "," type ">"

value        := toml-value | "null"
```

## Ce qui N'est PAS dans TOHL

Explicitement refusé pour garder la spec minimale :

- **Navigation multi-niveaux** (`[..x]`, `[...x]`, `[/./.x]`) — coût cognitif > gain
- **Anchors/références** (`&ref`, `*ref`) — YAML a montré que ça dérive
- **Expressions/calcul** (`port = base_port + 1`) — c'est un job pour Jsonnet/Cue/Nickel
- **Imports entre fichiers** — c'est un job pour l'outillage (Fimod ?), pas le format
- **Schémas inline obligatoires** — le typage inline optionnel suffit, les schémas externes restent une option

## Extension de fichier

Proposition : `.tohl` (clair, court, googlable).

Un parser TOML standard rencontrant `.tohl` devrait échouer proprement (présence de `---`, `null`, `:` après une clé, `[.x]`) — pas de faux positif silencieux.

## Ouvertures

- **Parser de référence** : Rust (via nom/pest), pour servir d'étalon.
- **Véhicule d'adoption** : Fimod pourrait utiliser TOHL pour sa config et son registry `catalog.toml` si le format mûrit.
- **Conversion** : `tohl → toml` trivial (strip frontmatter, rejet si `null` ou types stricts), `toml → tohl` trivial (identité).
