# Monty v1.0.0-beta.2 — Impact on fimod

Date: 2026-09-22
Previous: v0.0.23 → v1.0.0-beta.2

## Implementation status

Migration locale réalisée après validation utilisateur : les deux crates sont
verrouillées sur `=1.0.0-beta.2`, le MSRV est 1.96, le lock est régénéré.
La disponibilité crates.io a finalement été confirmée par Cargo.

- Valeurs natives MontyObject/ObjectRef, sans enum de compatibilité ; accès
  `unstable::MontyNode` pour les valeurs exactes, identités et parcours sans allocation.
- Pipeline/Step gardent les UUID et métadonnées hôtes ; callbacks adaptés à CallArgs.
- OsPolicy explicite sur molds et REPL : clock via allow_clock, sommeil/entropie
  refusés, random explicitement seedé disponible. Fuseaux fixes de datetime.now
  respectés. Budgets feed et quota hôte conservés.
- Gatekeeper utilise désormais la vérité Python native de Monty, y compris pour
  les conteneurs vides. Les noms de classes utilisateur ne servent jamais à
  identifier les types natifs dans les helpers.
- Documentation moteur actualisée. `rustls` corrigé de 0.23.43 à 0.23.45 pour
  [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285), révélé par lint.
- Tests ajoutés : contrôle des nouvelles opérations sensibles, fuseaux, capacités
  Python/stderr, graphes partagés/cycliques, classes homonymes de types natifs.

Vérifications finales :

- `rtk task lint` : succès (fmt, clippy, advisories/bans/licenses/sources).
- `rtk task test` : 689 tests réussis (240 lib, 3 bin, 444 CLI, 2 fixtures),
  12 tests de performance et 1 doctest ignorés par défaut.
- `rtk task lint:msrv` : succès avec Rust 1.96.1 (canal 1.96).
- `rtk cargo test --offline --test performance under_budget -- --ignored --nocapture` :
  9 tests réussis en debug ; ce n'est pas une comparaison avant/après ni un
  benchmark release. Cette passe précède le dernier ajustement du parcours
  des champs/indices et des gardes de types.
- `rtk git diff --check` : succès.
- Une passe sandboxée des tests HTTP a échoué sur SocketBindError ; la suite
  finale a été relancée avec accès aux serveurs locaux et a réussi.

L'analyse ci-dessous décrit l'état **avant migration** et les motifs des adaptations.

## Summary

Migration majeure et incompatible à la frontière Rust, avec de nouvelles capacités Python et une politique OS par défaut à adapter impérativement. Recommandation initiale : **wait**, préparer une migration dédiée avant le bump ; cette migration est maintenant implémentée localement.

Version de départ vérifiée dans `Cargo.toml:32` et `Cargo.lock:1876` : `monty` et `monty-types` 0.0.23, depuis crates.io (le workflow historique utilisant un tag Git ne s'applique plus).
GitHub retourne v1.0.0-beta.2 comme dernière release, publiée le 2026-09-21 à 21:35:16 UTC. Son indicateur `prerelease` est false, mais sa version reste une préversion SemVer beta.

## Evidence and limits

- [Release cible](https://github.com/pydantic/monty/releases/tag/v1.0.0-beta.2).
- [Comparaison complète des tags](https://github.com/pydantic/monty/compare/v0.0.23...v1.0.0-beta.2) : 67 commits. Le résultat JSON est limité à 300 fichiers et omet des patches ; analyse effectuée sur les archives complètes des deux tags.
- [API des valeurs cible](https://github.com/pydantic/monty/blob/v1.0.0-beta.2/crates/monty-types/src/object.rs), [accès instables](https://github.com/pydantic/monty/blob/v1.0.0-beta.2/crates/monty-types/src/object/unstable.rs).
- [Politique OS](https://github.com/pydantic/monty/blob/v1.0.0-beta.2/crates/monty-types/src/os_policy.rs), [limites](https://github.com/pydantic/monty/blob/v1.0.0-beta.2/crates/monty-types/src/resource.rs), [manifest upstream](https://github.com/pydantic/monty/blob/v1.0.0-beta.2/Cargo.toml).
- Analyse statique des sources, sans bump, compilation cible ni benchmark. Les appels de vérification crates.io ont répondu HTTP 403 : publication des deux crates cibles non confirmée dans cette session.

## API surface consumed by fimod

Inventaire régénéré depuis les imports multiligne et usages qualifiés de `src/`, tests embarqués inclus : **28 symboles nommés**. Les types des suspensions inférés par Rust sont détaillés dans la table d'impact.

| Type/fonction Monty | Fichiers fimod consommateurs | Rôle dans fimod |
|---|---|---|
| `BASELINE_MEMORY` | `src/mem_limit.rs` | Baseline du compteur mémoire de mimalloc. |
| `CompileOptions` | `src/cmd/monty.rs`, `src/engine.rs` | Options de compilation des molds et snippets. |
| `DictPairs` | `src/convert.rs`, `src/engine.rs`, `src/env_helpers.rs`, `src/format.rs`, `src/iter_helpers.rs`, `src/regex.rs`, `src/template.rs` | Construction, parcours et modification des dictionnaires. |
| `ExcType` | `src/cmd/monty.rs`, `src/engine.rs` | Reconnaissance des limites et construction des refus. |
| `ExtFunctionResult` | `src/engine.rs` | Réponse aux appels système. |
| `LIVE_MEMORY` | `src/mem_limit.rs` | Comptabilisation des allocations mimalloc. |
| `MontyClassInstance` | `src/engine.rs` | Objets hôtes Pipeline/Step et attributs. |
| `MontyClassType` | `src/engine.rs` | Classes hôtes Pipeline/Step. |
| `MontyDate` | `src/convert.rs`, `src/engine.rs`, `src/iter_helpers.rs (test, qualified use)` | Horloge autorisée et sérialisation date. |
| `MontyDateTime` | `src/convert.rs`, `src/engine.rs` | Horloge autorisée et sérialisation datetime. |
| `MontyException` | `src/cmd/monty.rs`, `src/engine.rs` | Construction et traduction des erreurs. |
| `MontyObject` | `src/cmd/monty.rs`, `src/convert.rs`, `src/dotpath.rs`, `src/engine.rs`, `src/env_helpers.rs`, `src/exit_control.rs`, `src/format.rs`, `src/format_control.rs`, `src/gatekeeper.rs`, `src/hash.rs`, `src/iter_helpers.rs`, `src/monty_args.rs`, `src/msg.rs`, `src/pipeline.rs`, `src/regex.rs`, `src/template.rs` | Valeurs de la chaîne, conversions, helpers et sérialisation. |
| `MontyRepl` | `src/cmd/monty.rs` | Session interactive, feed_start et tracker_mut. |
| `MontyRun` | `src/engine.rs` | Compilation et exécution suspendable via new/start. |
| `MontyTime` | `src/convert.rs` | Sérialisation des heures. |
| `MontyTimeDelta` | `src/convert.rs` | Sérialisation des durées. |
| `MontyTimeZone` | `src/convert.rs` | Sérialisation des fuseaux. |
| `MontyUuid` | `src/engine.rs` | Identités aléatoires et registre des objets hôtes. |
| `NameLookupResult` | `src/cmd/monty.rs`, `src/engine.rs` | Résolution des built-ins et attributs. |
| `OsFunctionCall` | `src/engine.rs` | Dispatch exhaustif contrôlé par SandboxPolicy. |
| `PrintWriter` | `src/cmd/monty.rs`, `src/engine.rs` | Sortie normale ou callback debug. |
| `PrintWriterCallback` | `src/engine.rs` | Redirection de print vers stderr en debug. |
| `ReplContinuationMode` | `src/cmd/monty.rs` | Invite multiligne. |
| `ReplProgress` | `src/cmd/monty.rs` | Boucle REPL, reprise et conservation de session. |
| `ResourceLimits` | `src/engine.rs` | Configuration mémoire, durée et suspensions. |
| `ResourceTracker` | `src/cmd/monty.rs`, `src/engine.rs` | Application des limites, budget de snippet. |
| `RunProgress` | `src/engine.rs` | Boucle des molds et dispatch des suspensions. |
| `detect_repl_continuation_mode` | `src/cmd/monty.rs` | Détection des snippets incomplets. |

## Changes impacting fimod

**13/28 symboles ont une évolution directe d'API ou de comportement ; 2 autres transportent les types modifiés.** Ce décompte ne signifie pas 13 ruptures de compilation. Les charges utiles inchangées contenant MontyObject restent à réévaluer lors de la migration.

| Symbole consommé | Changé ? | Nature | Action requise |
|---|---|---|---|
| `BASELINE_MEMORY` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `CompileOptions` | oui | Ajout source_scan_threshold, défaut 4096 octets. | default() reste compatible ; vérifier les scripts très imbriqués. |
| `DictPairs` | oui | Type retiré. | Migrer vers constructeurs/itérateurs de paires et accès au graphe si nécessaire. |
| `ExcType` | oui | Ajout BinasciiIncomplete. | Les matchs fimod ont un fallback : aucune adaptation directe. |
| `ExtFunctionResult` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `LIVE_MEMORY` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyClassInstance` | oui | Type retiré. | Utiliser MontyObject::class_instance ; adapter identité/attributs. |
| `MontyClassType` | oui | Type retiré. | Utiliser MontyObject::class_type ; adapter le registre. |
| `MontyDate` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyDateTime` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyException` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyObject` | oui | Enum récursive → structure opaque possédant un graphe. | Migrer tous les constructeurs, matchs et modifications de conteneurs. |
| `MontyRepl` | oui | feed_start accepte Into<NamedValues> ; ajout OsPolicy, pré-scan. | Configurer OsPolicy ; Vec de valeurs nommées reste accepté. |
| `MontyRun` | oui | new/start gardent leurs signatures ; nouvelle politique OS par défaut. | Configurer with_os_policy avant start. |
| `MontyTime` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyTimeDelta` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyTimeZone` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `MontyUuid` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `NameLookupResult` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `OsFunctionCall` | oui | Six variants ajoutés ; DateTimeNow transporte Option<MontyTimeZone>. | Adapter le match exhaustif et la politique horloge/entropie/sommeil. |
| `PrintWriter` | oui | Routage stdout/stderr ; CollectStreams change de buffer. | Stdout et Callback restent compatibles ; vérifier print(file=sys.stderr). |
| `PrintWriterCallback` | oui | Méthodes stderr avec implémentations par défaut. | StderrPrint reste compatible : les deux flux aboutissent sur stderr. |
| `ReplContinuationMode` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |
| `ReplProgress` | indirect | Variants inchangés ; valeurs et arguments transportés changent. | Adapter comparaison à None ; tester reprise après quota/erreur. |
| `ResourceLimits` | oui | max_duration remplacé par budgets feed/turn ; max_total_sleep ajouté. | Mapper la durée fimod sur max_feed_duration, préserver budget chaîne. |
| `ResourceTracker` | oui | set_max_duration retiré ; nouveaux budgets feed/turn. | Utiliser set_max_feed_duration pour les snippets. |
| `RunProgress` | indirect | Variants inchangés ; FunctionCall.args devient CallArgs, kwargs retiré. | Adapter engine.rs:1041 et conserver les quotas hôtes. |
| `detect_repl_continuation_mode` | non | Contrat consommé inchangé. | RAS hors migration des valeurs contenues. |

## Breaking changes

1. **MontyObject et DictPairs** — variants `Int`, `Dict`, `List`, `Date`, etc. → constructeurs `int`, `dict`, `list`, `date` et lecture via `ObjectRef`. `DictPairs` disparaît. 367 occurrences de `MontyObject::` sur 353 lignes dans 16 fichiers, tests embarqués inclus : ce n'est pas un renommage mécanique.
   Sites principaux : `src/convert.rs:75`, `src/convert.rs:132`, `src/convert.rs:200`, `src/dotpath.rs`, `src/iter_helpers.rs`, `src/format.rs`, tous les helpers de la cartographie.
   Les accesseurs stables ne couvrent pas toute la fidélité actuellement requise (BigInt complet, dates, identité des instances, modification de conteneurs). Évaluer un adaptateur étroit utilisant `monty_types::unstable` pour ces cas ; cette API ne garantit explicitement aucune compatibilité entre releases. Conserver la précision i64/u64, le rejet des valeurs non représentables et les deux chemins de sérialisation. Le graphe impose aussi de vérifier le comportement des valeurs partagées/cycliques et le coût des copies.
2. **Classes hôtes** — `MontyClassType`/`MontyClassInstance` retirés → `MontyObject::class_type`/`class_instance`. Adapter `src/engine.rs:240`, `:252`, `:284`, `:519`, `:642`. Préserver les UUID et métadonnées détenues par l'hôte, sans faire confiance aux attributs modifiables côté Python.
3. **FunctionCall** — `args: Vec<MontyObject>` et `kwargs` séparés → `args: CallArgs`, avec `args()`/`kwargs()` retournant des vues empruntées. Adapter `src/engine.rs:1041`, notamment `append`, `mem::take` et le receiver ajouté en tête. `object_id` demeure disponible. Les méthodes de reprise consommées gardent leur forme générale.
4. **Durée** — `ResourceLimits::max_duration` et `ResourceTracker::set_max_duration` → variantes `max_feed_duration`/`set_max_feed_duration`, avec budget par tour distinct. Adapter `src/engine.rs:1019`, `:1134`, `src/cmd/monty.rs:100`. Conserver le calcul du temps restant pour la chaîne ; un budget par tour seul permettrait à chaque reprise de recommencer la limite. Les compteurs LIVE_MEMORY/BASELINE_MEMORY restent compatibles avec CountingMiMalloc ; les contrôles avant croissance des conteneurs se renforcent.
5. **OsFunctionCall** — ajout de `Urandom`, `Time`, `Sleep`, `SystemSleep`, `AsyncSleep`, `AsyncSystemSleep` ; le match de `src/engine.rs:1158` devient incomplet. `DateTimeNow` reçoit désormais un fuseau typé. Définir explicitement les refus des nouvelles opérations et vérifier la sémantique des dates avec fuseau.
6. **Rupture de politique sans erreur de compilation** — `MontyRun` et `MontyRepl` utilisent par défaut l'horloge système et l'entropie pour initialiser random, y compris sur les chemins suspendables. Configurer `with_os_policy` aux constructions `src/engine.rs:923` et `src/cmd/monty.rs:30`, au minimum `datetime: DateTimeSource::CallHost`, et décider explicitement `random_start` et `sleep`. Ne pas laisser les défauts upstream contourner `allow_clock`. Les appels système de sommeil demandent une décision hôte ; le temps suspendu n'est pas imputé au budget d'exécution. Conserver le quota fimod non rattrapable par Python, également dans le REPL.
7. **MSRV** — Rust 1.95 → **1.96** upstream. Adapter `Cargo.toml:12`, `Taskfile.yml:76`, `.github/workflows/ci.yml` et les mentions du plancher dans `mise.toml`/notes. Le canal local est déjà `stable`/`latest` : ne pas réintroduire de pin local 1.95.

## New capabilities unlocked

Capacités du runtime cible, à annoncer pour fimod seulement après intégration et validation :

- `copy.copy` et `copy.deepcopy` ; nouveau module `random` (initialisation à contrôler), nouveau module `time` (horloges et sommeil à contrôler).
- Formatage `%` des chaînes/bytes et builtin `format()` ; fusion de dictionnaires `a | b` ; cibles attribut/index dans les déstructurations.
- `eval`, `exec`, `locals` exécutés dans Monty. Revoir les mentions documentaires qui les déclarent absents ; leur ajout n'implique pas un accès à CPython ou aux fichiers hôtes.
- `print(..., file=sys.stderr)`, extensions itertools, math et binascii ; meilleure compatibilité des grands entiers et du case folding.
- Répertoire courant virtuel et support de fuseaux configurables. Aucun accès filesystem fimod supplémentaire n'est impliqué.

## Upgrade steps

1. Après accord sur la migration, travailler sur une branche dédiée et choisir la frontière d'adaptation des valeurs (accès stables, accès instables isolés si requis). Vérifier la disponibilité crates.io des deux versions cibles.
2. Adapter les ruptures ci-dessus dans les sources avant le bump, en préservant les contrats de sérialisation, les objets hôtes et la politique sandbox.
3. Passer le MSRV à 1.96 et les deux dépendances explicites à `1.0.0-beta.2`. `cargo update -p monty` seul ne peut pas franchir la contrainte actuelle 0.0.23 ni sélectionner cette beta.
4. Régénérer le lock : `rtk cargo update -p monty -p monty-types`, puis `rtk cargo build`. Arrêter et diagnostiquer au premier échec.
5. Vérifier les régressions ciblées : grands entiers/dates dans les deux sérialiseurs, mutations dotpath sans altération de l'entrée, dispatch Pipeline/Step et UUID, allow_clock désactivé/activé dans mold et REPL, mémoire/durée/quotas et conservation du REPL, stdout/stderr et nouvelles capacités Python.
6. Exécuter `rtk task lint`, `rtk task test`, `rtk task lint:msrv`. Comparer les benchmarks existants de conversion/chaîne/helpers : les graphes et copies peuvent changer le profil de performance.
7. Mettre à jour `docs/reference/monty-engine.md` et les notes de conception avec le comportement réellement validé.

## Documentation decision

La documentation publique est maintenant mise à jour pour la version intégrée : version et exemple REPL, modules copy/random/time, dict union, eval/exec/locals, mécanisme des objets externes, contrôle OsPolicy et limites de durée. Ne pas publier de nouveaux chiffres de performance sans mesure.

## Risk

**high** — remplacement transversal de la représentation des valeurs, APIs instables nécessaires à évaluer, nouveaux défauts OS pouvant contourner la politique actuelle, changements de durée et MSRV. Ce risque initial motive les tests et le verrouillage exact des versions ; voir le statut d’implémentation ci-dessus.

## Recommendation

Migration implémentée localement. Garder les versions exactes et les tests de
régression pendant la phase beta. Aucune validation de distribution, commit,
push ou release n'est implicite dans les vérifications locales.
