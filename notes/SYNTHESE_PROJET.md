# Synthèse du projet Fimod

Date d'analyse : 2026-05-15  
Version observée : `0.7.3` (`Cargo.toml`, `CHANGELOG.md`)

## Résumé exécutif

Fimod est un outil CLI Rust de transformation de données. Sa promesse centrale est simple : manipuler des formats courants avec une syntaxe Python, sans installer Python. Le projet embarque Monty, un moteur Python écrit en Rust, et conserve la lecture, l'écriture, la sérialisation et les garde-fous de sécurité côté Rust.

Le positionnement est clair : remplacer une partie des usages `jq`, `yq`, `awk`, scripts Python ad hoc et pipelines `curl | jq` par un binaire unique, portable, testable et adapté aux environnements CI/CD. Le projet vise surtout les ingénieurs DevOps, les mainteneurs de pipelines, les équipes qui manipulent des manifestes/configurations, et les auteurs de transformations réutilisables.

Le dépôt est déjà structuré comme un vrai produit : documentation utilisateur MkDocs/Zensical, catalogue de molds, tests d'intégration, fixtures déclaratives, workflows CI/release, scripts d'installation Linux/macOS/Windows et notes d'architecture internes.

## Proposition de valeur

Fimod combine quatre choix forts :

- **Syntaxe Python** pour éviter l'apprentissage d'un DSL dédié.
- **Binaire Rust autonome** pour éviter `pip install`, CPython et la dérive d'environnements.
- **Pipeline data-in/data-out** centré sur `Read -> Parse -> Transform -> Serialize -> Write`.
- **Molds réutilisables** : scripts Python versionnables, testables, partageables via répertoires locaux ou registres Git.

Les formats pris en charge couvrent les besoins de pipeline les plus fréquents : JSON, JSON compact, NDJSON/JSONL, YAML, TOML, CSV/TSV, texte, lignes, HTTP en entrée et raw en sortie.

## Surface fonctionnelle

La commande principale est `fimod shape`, alias `fimod s`. Elle accepte une transformation inline (`-e`) ou un mold (`-m`), et peut chaîner plusieurs étapes dans le même processus.

Fonctions principales observées :

- Transformation de fichiers, stdin et URLs HTTP.
- Conversion entre formats.
- Slurp mono-fichier ou multi-fichiers, avec mode liste ou mode nommé.
- Batch sur plusieurs entrées.
- Écriture stdout, fichier, dossier de sortie ou in-place.
- Watch mode sur fichiers locaux.
- Passage d'arguments (`--arg`) et variables d'environnement filtrées (`--env`).
- Gestion des headers HTTP, timeout, redirects et cache.
- Contrôle CSV : délimiteurs, headers en entrée/sortie, colonnes explicites.
- Mode `--check` pour transformer une vérité de données en code retour.
- Messages de molds contrôlés par `msg_*`, `--quiet` et `--msg-level`.
- Registres de molds : add/list/show/remove/priority/cache/build-catalog.
- Commandes de setup : registres communautaires, sandbox par défaut, complétions shell.
- REPL Monty via `fimod monty repl`.

## Architecture

L'architecture est en couches nettes :

| Couche | Fichiers principaux | Rôle |
|---|---|---|
| CLI | `src/main.rs`, `src/cli.rs`, `src/cmd/*` | Parsing Clap, validation des options, dispatch des sous-commandes |
| Pipeline | `src/pipeline.rs` | Source de vérité du flux read/parse/execute/serialize/write |
| Résolution de molds | `src/mold.rs`, `src/registry/*` | Chargement local, URL, inline, `@name`, catalogues et cache |
| Exécution | `src/engine.rs` | Boucle Monty, appels externes, sandbox, API dynamique de pipeline |
| Formats/I/O | `src/format.rs`, `src/http.rs`, `src/convert.rs`, `src/serde_compat.rs` | Parsing, sérialisation, HTTP, conversion JSON <-> Monty |
| Built-ins | `src/regex.rs`, `src/dotpath.rs`, `src/iter_helpers.rs`, etc. | Fonctions Rust exposées aux molds |
| Tests molds | `src/test_runner.rs`, `tests-molds/*` | Runner de fixtures `input` / `expected` |

Le coeur technique repose sur un invariant important : l'I/O passe par Rust, puis les données sont converties vers `MontyObject` pour l'exécution. Monty ne lit pas directement le système de fichiers ni le réseau.

## Flux d'exécution

Un appel typique suit ce parcours :

1. `main.rs` parse la CLI et délègue au module `cmd`.
2. `cmd/shape.rs` traduit les options utilisateur en configuration de pipeline.
3. `pipeline.rs` lit l'entrée depuis fichier, stdin ou HTTP.
4. `format.rs` détecte ou applique le format d'entrée.
5. Les données sont converties vers Monty.
6. `engine.rs` exécute chaque mold du chaînage.
7. Les appels `re_*`, `dp_*`, `it_*`, `hs_*`, `msg_*`, `gk_*`, `tpl_*`, etc. sont résolus par dispatch Rust.
8. Le résultat final revient en `serde_json::Value`.
9. Le format de sortie est résolu puis sérialisé.
10. La sortie est écrite vers stdout, fichier, dossier ou entrée in-place.

Depuis les dernières versions, le pipeline évite autant que possible les conversions intermédiaires coûteuses : les résultats de chaîne restent en `MontyObject` entre deux étapes, sauf quand une étape force une re-parse via changement de format.

## Molds et extensibilité

Un mold est un script Python qui expose `transform(data, **_)`. Les contextes optionnels (`args`, `env`, `headers`, `pipeline`) sont passés comme kwargs et peuvent être déclarés avant `**_` quand le mold les utilise. Le dépôt contient un catalogue intégré de molds couvrant des cas concrets :

- extraction et filtrage de champs (`pick_fields`, `filter_fields`, `deep_pluck`) ;
- transformation de structure (`flatten_nested`, `rename_keys`, `sort_json_keys`) ;
- CSV et stats (`csv_stats`, `csv_to_json_records`) ;
- anonymisation (`anonymize_pii`, `auto_anonymize`) ;
- validation et contrôle de pipeline (`validate_fields`, `checkpoint`, `with_threshold`, `sample_if_large`) ;
- génération texte/template (`badge_md`, `git_changelog`, `env_to_dotenv`) ;
- YAML/configuration (`yaml_merge`) ;
- compatibilité partielle avec des usages `jq` (`jq_compat`).

Le système de registres permet d'utiliser des sources locales, GitHub/GitLab ou HTTP. La résolution `@name` cherche selon les priorités configurées, et `@source/name` permet de désambiguïser.

L'extension du runtime se fait selon un modèle stable : ajouter un module de built-ins Rust, déclarer `EXTERNAL_FUNCTIONS`, implémenter `dispatch`, puis brancher le module dans `engine.rs`.

## Sécurité et sandbox

La sécurité repose sur une séparation volontaire :

- les molds manipulent des objets de données ;
- Rust garde la maîtrise de l'I/O, du réseau, des variables d'environnement, du temps et des limites de ressources ;
- les capacités externes sont gérées par une politique `sandbox.toml`.

Le modèle n'est pas présenté comme un sandbox multi-tenant pour code hostile. Il vise plutôt un usage CLI local, avec garde-fous forts contre les effets de bord accidentels, les scripts distants trop permissifs et les traitements qui s'emballent.

Le code définit notamment un code de sortie `137` pour les dépassements de limites sandbox, ce qui reprend une convention proche des arrêts par le système.

## Qualité et tests

Le projet a une stratégie de test dense et adaptée à sa surface :

- tests d'intégration CLI dans `tests/cli/*` ;
- tests de molds via fixtures dans `tests-molds/*` ;
- runner spécifique pour comparer entrées/sorties attendues ;
- tests dédiés aux formats, chaînes, registre, HTTP, sandbox, watch, templates, env, erreurs, etc.

Métriques observées dans le dépôt :

- `37` fichiers Rust dans `src/`, environ `12 136` lignes ;
- `37` fichiers de tests Rust, environ `8 801` lignes ;
- `153` fichiers de fixtures dans `tests-molds/` ;
- `28` scripts Python de molds ;
- documentation Markdown utilisateur : environ `5 278` lignes.

La CI exécute formatage, clippy, tests multi-OS, MSRV Rust `1.75`, audit sécurité, cargo-deny et build musl. Les lints Rust interdisent notamment `unsafe_code` et refusent certains patterns Clippy.

## Build, release et distribution

Le projet est distribué comme binaire autonome, avec variantes :

- build par défaut avec support HTTP (`reqwest`) ;
- build `slim` sans dépendance HTTP ;
- feature `watch` activée par défaut.

Les releases GitHub construisent plusieurs cibles :

- Linux musl `x86_64` et `aarch64` ;
- macOS Apple Silicon ;
- Windows MSVC ;
- variantes default et slim.

Les binaires Linux/Windows sont compressés avec UPX. Le workflow release publie aussi checksums, archive des molds, fichier `VERSION` et images Docker GHCR multi-arch.

Le dépôt fournit aussi :

- `install.sh` pour Linux/macOS ;
- `install.ps1` et chemin alternatif Windows via `ubi` ;
- `Taskfile.yml` pour build, test, lint, docs, dist et génération VHS ;
- documentation déployée via GitHub Pages.

## Documentation

La documentation utilisateur est solide et organisée :

- guides : quick start, concepts, scripting, dynamic molds, authoring, AI integration, CLI reference ;
- références : formats, built-ins, defaults de molds, codes de sortie, moteur Monty ;
- exemples : JSON, YAML, CSV, HTTP ;
- cookbook et galerie de molds.

Les notes internes jouent aussi un rôle important :

- `notes/VISION.md` fixe les non-négociables produit ;
- `notes/ARCHITECTURE.md` décrit les couches et invariants ;
- `notes/CODE_LAYOUT.md` sert de carte de contribution ;
- `notes/DESIGN_NOTES.md` documente les décisions.

Cette discipline documentaire réduit fortement le coût d'entrée pour un contributeur ou un agent.

## Forces du projet

- Positionnement produit lisible et différencié.
- Architecture modulaire, avec responsabilités bien séparées.
- Forte cohérence entre README, docs, notes internes et code.
- Bon niveau de test pour un outil CLI jeune.
- Surface d'extension claire pour les built-ins, formats et molds.
- Distribution travaillée : installateurs, releases multi-plateformes, Docker, docs.
- Choix de sécurité cohérent avec le produit : Rust comme frontière de confiance, Monty comme moteur de transformation.

## Points d'attention

- **Dépendance à Monty** : Monty est jeune et son API peut changer. Le projet assume ce risque, mais chaque upgrade doit rester traité comme un sujet d'intégration sérieux.
- **Surface CLI large** : la richesse fonctionnelle augmente le risque de combinaisons d'options difficiles à maintenir.
- **Format IR en JSON** : `serde_json::Value` simplifie les conversions mais perd les commentaires, ancres YAML, détails TOML et certaines informations de forme.
- **Sandbox** : le modèle est adapté à un CLI local, pas à un service multi-tenant. Cette limite doit rester explicite.
- **Cache non borné** : certains caches de performance sont volontairement non bornés pour l'instant ; c'est acceptable en CLI, mais à surveiller sur gros traitements.
- **Docs très vivantes** : la documentation est riche, mais doit rester synchronisée avec une cadence de release rapide.

## Problèmes et risques identifiés

### P1 - Parsing manuel de l'ordre `-m` / `-e` — traité

Le risque initial était que `src/cmd/shape.rs::build_script_refs` reparcoure
`std::env::args()` pour reconstruire l'ordre d'apparition des options `-m` et
`-e`. Cette logique était fragile parce qu'elle dupliquait une partie du parsing
déjà fait par Clap.

Risques qui ont motivé la correction :

- divergences sur les formes d'options acceptées par Clap ;
- mauvais comportement autour de `--`, valeurs ressemblant à des flags, ou syntaxes rares ;
- difficulté à tester exhaustivement tous les cas d'arguments.

État actuel : le plumbing CLI récupère désormais l'ordre via les indices fournis
par Clap (`ArgMatches`) et passe la chaîne ordonnée explicitement au pipeline
`shape`, y compris en watch mode. Les sujets qui restent à planifier ont été
déplacés vers `notes/todo-to-plannif.md`.

### P1 - Positionnement "Python" à préciser

La promesse "Python-powered without Python installed" est forte et différenciante, mais elle attire naturellement des attentes CPython : imports, stdlib complète, pandas, requests, classes, compatibilité de scripts existants. Or le produit exécute un sous-ensemble Python via Monty, avec des built-ins Rust spécifiques.

Le README mentionne les limitations plus bas, mais le message initial peut être lu comme "du Python normal dans un binaire". Pour éviter une mauvaise qualification des utilisateurs, la proposition de valeur devrait dire très tôt :

- syntaxe Python et fonctions Python usuelles ;
- pas CPython ;
- pas PyPI ;
- sous-ensemble Monty ;
- I/O volontairement contrôlée par Rust.

Ce n'est pas une faiblesse du produit, mais un risque de promesse. Le bon angle est probablement "Python-shaped data transforms", pas "Python runtime généraliste".

### P1 - Promesse sandbox à formuler plus prudemment

Le README affirme que les molds distants/non fiables sont sûrs à exécuter. Les notes de vision sont plus nuancées : Fimod est un CLI local, pas une sandbox hostile-grade ni un service multi-tenant.

La frontière Rust/Monty est une force réelle, mais la communication doit éviter de promettre plus que le modèle ne garantit. Une formulation plus défendable :

- "deny-by-default for host capabilities" ;
- "safe defaults for local CLI usage" ;
- "not designed as a hostile-code multi-tenant sandbox" ;
- "review remote registries you trust, even if host I/O is gated".

Ce point est important pour éviter un malentendu sécurité, surtout avec les molds distants et les registres privés/publics.

### P2 - Contrat de combinaisons `shape` à préserver

La surface `shape` est riche : batch, multi-slurp, watch, raw, HTTP, in-place, input-list, check, CSV options, output dynamique, sandbox, cache. Ce n'est pas un problème logique immédiat : le code isole déjà les cas principaux.

- `run_shape` valide les incompatibilités de `--watch` avant la résolution des entrées ;
- `--input-list` est matérialisé avant les validations post-résolution ;
- `validate_post_input_list` couvre `--no-input`, `--in-place`, batch et multi-slurp ;
- `run_raw_passthrough` sépare clairement le mode `--output-format raw` du pipeline normal ;
- les tests couvrent déjà les cas sensibles dans `tests/cli/args.rs`, `batch.rs`, `multi_slurp.rs`, `output_file.rs` et `watch.rs`.

Le risque n'est donc pas que le comportement actuel soit incohérent. Le risque est plutôt qu'une future option ajoute une exception implicite à batch/slurp/watch/raw/input-list sans test d'interaction.

Action recommandée : garder les validations centralisées et ajouter un test ciblé dès qu'une nouvelle fonctionnalité touche ces modes. Une matrice exhaustive dans la documentation utilisateur n'est pas nécessaire ; les tests d'intégration peuvent faire office de contrat.

### P2 - Commande cache partiellement trompeuse

La CLI expose `fimod registry cache clear @name`, mais `src/registry/catalog.rs::cache_clear(Some(_))` affiche un warning puis vide tout le cache. C'est honnête au runtime, mais la surface de commande laisse penser à une granularité qui n'existe pas encore.

Action recommandée :

- soit implémenter le clear ciblé ;
- soit retirer l'argument `name` de la CLI jusqu'à disponibilité ;
- soit documenter plus explicitement que `@name` est planifié et non opérationnel.

### P2 - Caches process-wide non bornés

Les caches regex et templates sont utiles et cohérents avec un CLI court. En revanche, `template.rs` documente explicitement un cache non borné et le conseille avec prudence pour les usages bibliothèque ou les templates dérivés des données.

Ce n'est pas bloquant aujourd'hui, mais c'est une limite à garder visible si le projet pousse son API librairie ou des modes longs. Un LRU optionnel ou une fonction de purge deviendrait pertinent si ces usages augmentent.

### P2 - Non-streaming et datasets volumineux

Le pipeline charge les données et les convertit en structures intermédiaires. C'est cohérent avec la vision "one-shot pure pipeline", mais cela doit rester assumé dans le positionnement : Fimod est un excellent outil de transformation de fichiers/configs/exports raisonnables, pas un moteur streaming ou big data.

Le risque serait de le vendre comme "data processing" généraliste. Le meilleur cadrage produit est "data shaping for CI, configs, API payloads, release tooling and moderate datasets".

### P2 - API dynamique de pipeline puissante mais complexe

`pipeline.insert_next`, `pipeline.append`, `Step.create`, `Step.get` et `Step.set` donnent beaucoup de pouvoir aux molds. C'est utile pour les molds adaptatifs, mais cela complexifie le modèle mental initial "une transformation pure".

Risque :

- documentation plus difficile ;
- tests plus subtils ;
- possibilité de pipelines auto-modifiants difficiles à diagnostiquer ;
- glissement vers un orchestrateur plutôt qu'un data shaper.

Recommandation : conserver cette API comme fonctionnalité avancée, l'encadrer par des exemples sobres, et éviter d'en faire la voie principale dans le README.

## Priorités recommandées

1. Remplacer le parsing manuel de l'ordre `-m` / `-e` par une source Clap ou un modèle d'étapes ordonnées.
2. Continuer à protéger le contrat CLI par tests d'intégration avant chaque ajout majeur, surtout sur batch/slurp/watch/raw.
3. Garder `notes/VISION.md` comme filtre de roadmap pour éviter l'élargissement vers un runtime applicatif.
4. Formaliser les upgrades Monty avec notes d'impact et tests ciblés.
5. Surveiller les performances et la mémoire sur gros CSV/NDJSON, notamment autour des conversions et caches.
6. Maintenir la documentation des molds au même niveau que le code, car le système de registre en dépend fortement.
7. Conserver la distinction entre sécurité CLI locale et sandbox hostile-grade dans toute communication publique.

## Conclusion

Fimod est un projet jeune mais déjà mature dans sa structure. Il ne se limite pas à un wrapper autour d'un moteur Python : c'est un outil de transformation complet, avec une architecture pensée autour d'une frontière Rust/Monty, une forte attention à l'ergonomie CLI et une vraie stratégie de distribution.

Le principal risque technique est la dépendance à Monty, compensée par une architecture explicite, des tests nombreux et une documentation interne inhabituelle pour un projet de cette taille. La direction produit est cohérente : rester un binaire CLI de transformation, pas devenir une plateforme d'exécution Python générale.
