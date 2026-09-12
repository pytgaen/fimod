# Monty v0.0.23 — Impact on fimod

Date: 2026-09-09
Previous: v0.0.21 → v0.0.23
Status: migration locale implémentée et validée ; aucun commit ni push.

La cartographie et les ruptures ci-dessous décrivent le point de départ 0.0.21 ;
la section Implementation record décrit le résultat actuel.
Fimod HEAD analysé : `1f3c592` ; `Cargo.toml` et `Cargo.lock` utilisent `monty` et `monty-types` 0.0.21 depuis crates.io.

## Summary

Release fonctionnelle avec deux ruptures Rust affectant les objets Pipeline/Step ; migration ciblée nécessaire avant le bump.

## Sources and method

- [Release v0.0.23](https://github.com/pydantic/monty/releases/tag/v0.0.23)
- [Comparaison complète v0.0.21...v0.0.23](https://github.com/pydantic/monty/compare/v0.0.21...v0.0.23) : 76 commits, cible `302e0f27`.
- [Diff complet](https://github.com/pydantic/monty/compare/v0.0.21...v0.0.23.diff) consulté : la réponse JSON GitHub est limitée à 300 fichiers et ne suffit pas.
- Imports `monty` et `monty_types` régénérés depuis tous les fichiers `src/**/*.rs`, puis sites d’usage et diff des définitions examinés. Aucun résultat de compilation de la cible revendiqué.

## API surface consumed by fimod

24 symboles importés directement. Les types des suspensions obtenus indirectement (`FunctionCall`, `NameLookup`, `OsCall` et équivalents REPL) sont aussi examinés ci-dessous.

| Type/fonction | Consommateurs | Rôle |
|---|---|---|
| `BASELINE_MEMORY` | `src/mem_limit.rs` | Base mémoire du processus |
| `CompileOptions` | `src/cmd/monty.rs`, `src/engine.rs` | Options de compilation |
| `DictPairs` | `src/convert.rs`, `src/engine.rs`, `src/env_helpers.rs`, `src/format.rs`, `src/iter_helpers.rs`, `src/regex.rs`, `src/template.rs` | Dictionnaires et attributs |
| `ExcType` | `src/cmd/monty.rs`, `src/engine.rs` | Traduction des erreurs et refus sandbox |
| `ExtFunctionResult` | `src/engine.rs` | Réponses aux appels externes et OS |
| `LIVE_MEMORY` | `src/mem_limit.rs` | Comptage via CountingMiMalloc |
| `MontyDate` | `src/convert.rs`, `src/engine.rs` | Horloge et conversion des dates |
| `MontyDateTime` | `src/convert.rs`, `src/engine.rs` | Horloge et conversion des dates/heures |
| `MontyException` | `src/cmd/monty.rs`, `src/engine.rs` | Construction et traduction des erreurs |
| `MontyObject` | `src/cmd/monty.rs`, `src/convert.rs`, `src/dotpath.rs`, `src/engine.rs`, `src/env_helpers.rs`, `src/exit_control.rs`, `src/format.rs`, `src/format_control.rs`, `src/gatekeeper.rs`, `src/hash.rs`, `src/iter_helpers.rs`, `src/monty_args.rs`, `src/msg.rs`, `src/pipeline.rs`, `src/regex.rs`, `src/template.rs` | Valeurs du pipeline et objets Pipeline/Step |
| `MontyRepl` | `src/cmd/monty.rs` | Session Python interactive |
| `MontyRun` | `src/engine.rs` | Compilation et démarrage des molds |
| `MontyTimeDelta` | `src/convert.rs` | Sérialisation des durées |
| `MontyTimeZone` | `src/convert.rs` | Sérialisation des fuseaux |
| `NameLookupResult` | `src/cmd/monty.rs`, `src/engine.rs` | Résolution de noms |
| `OsFunctionCall` | `src/engine.rs` | Frontière OS/sandbox |
| `PrintWriter` | `src/cmd/monty.rs`, `src/engine.rs` | Routage stdout/debug |
| `PrintWriterCallback` | `src/engine.rs` | Print debug vers stderr |
| `ReplContinuationMode` | `src/cmd/monty.rs` | Gestion multiligne |
| `ReplProgress` | `src/cmd/monty.rs` | Boucle REPL |
| `ResourceLimits` | `src/engine.rs` | Limites sandbox |
| `ResourceTracker` | `src/cmd/monty.rs`, `src/engine.rs` | Suivi mémoire/temps |
| `RunProgress` | `src/engine.rs` | Boucle d’exécution |
| `detect_repl_continuation_mode` | `src/cmd/monty.rs` | Détection de continuation |

## Changes impacting fimod

| Symbole | Impact / action |
|---|---|
| `BASELINE_MEMORY` | Inchangé |
| `CompileOptions` | Inchangé |
| `DictPairs` | Ajouts compatibles : Default, Eq, iter public |
| `ExcType` | Ajout BinasciiError ; matchs fimod avec branche de repli |
| `ExtFunctionResult` | Inchangé |
| `LIVE_MEMORY` | Inchangé |
| `MontyDate` | Inchangé |
| `MontyDateTime` | Structure inchangée ; documentation des offsets précisée |
| `MontyException` | Pas de rupture identifiée sur les méthodes consommées |
| `MontyObject` | Rupture Dataclass → ClassInstance ; ajout Time ; type_name retourne &str |
| `MontyRepl` | API consommée conservée ; tables de compilation déplacées sans clonage |
| `MontyRun` | new/start conservent leurs signatures |
| `MontyTimeDelta` | Inchangé |
| `MontyTimeZone` | Pas de rupture sur la structure consommée |
| `NameLookupResult` | Ajout Error et conversions From, compatible avec les constructions fimod |
| `OsFunctionCall` | Pas de variant ajouté ou retiré dans le diff |
| `PrintWriter` | Ajout wants_poll/poll_flush ; variants utilisés conservés |
| `PrintWriterCallback` | poll_flush ajouté avec implémentation par défaut |
| `ReplContinuationMode` | Variants consommés conservés |
| `ReplProgress` | Variants conservés ; payloads exposent object_id/abort, receiver de méthode changé |
| `ResourceLimits` | Ajout max_suspensions=1000, enforcement à la charge de l’hôte |
| `ResourceTracker` | Nouveaux helpers et cadence de contrôle modifiée ; compteurs conservés |
| `RunProgress` | Variants conservés ; FunctionCall.method_call supprimé, NameLookup étendu |
| `detect_repl_continuation_mode` | Corrections décorateurs et chaînes triple-quoted |

## Breaking changes

1. **`MontyObject::Dataclass { ... }` → `ClassInstance(Box<MontyClassInstance>)`.**
   `name/type_id/field_names/attrs/frozen` ne se transposent pas mécaniquement.
   La nouvelle instance contient `class_type`, `instance_id: MontyUuid` et `attrs` ;
   `MontyClassType` contient `name`, `id`, `host_defined`, `is_dataclass`, `attrs`.
   Adapter les constructeurs et patterns dans `src/engine.rs:435`, `:453`, `:488`,
   `:542`, `:579`, `:800`, `:834`, `:998`, ainsi que les identités à `:226`.
   Préserver l'identité des instances pendant une exécution et l’isolation entre
   exécutions. Vérifier les garanties actuelles de mutabilité : le champ `frozen`
   n’existe plus dans ce contrat de transport.
2. **`FunctionCall.method_call: bool` → `object_id: Option<MontyUuid>`, sans `self` dans `args`.**
   `src/engine.rs:946` ne compile plus ; `dispatch_method` (`:479`) et ses helpers
   supposent actuellement `args[0] = self`. Résoudre le receiver dans une table
   locale à l’exécution, puis adapter les offsets des arguments. Remplacer seulement
   le booléen par `object_id.is_some()` laisserait une régression fonctionnelle.
   Les recherches d’attributs passent maintenant aussi par `NameLookup` : distinguer
   `lookup.object_id()` des noms globaux à `src/engine.rs:995`, afin de ne pas
   résoudre un attribut comme une fonction globale. Revoir aussi le refus explicite
   dans `src/cmd/monty.rs:129` pour les objets hôte éventuels.

Sources : [types des objets](https://github.com/pydantic/monty/blob/v0.0.23/crates/monty-types/src/object.rs),
[suspensions](https://github.com/pydantic/monty/blob/v0.0.23/crates/monty/src/run_progress.rs),
[adaptation du harness upstream](https://github.com/pydantic/monty/blob/v0.0.23/crates/monty-datatest/src/main.rs).

## Behavioral watchpoints

- **Limite de suspensions** : `ResourceLimits::default()` ajoute `max_suspensions=1000`,
  mais l’interpréteur stocke seulement ce budget ; l’hôte doit le compter et l’appliquer.
  Les boucles fimod (`src/engine.rs:943`, `src/cmd/monty.rs:126`) ne le font pas.
  Décider explicitement du budget et de sa portée avant de l’exposer comme protection.
  Une limite fixe peut affecter les molds appelant un built-in sur plus de 1000 lignes.
  Les nouveaux `abort()` permettent un arrêt non interceptable ; ne pas annoncer
  que le bump seul active cette garantie.
- **Mémoire/temps** : `ResourceTracker` change ses checkpoints. Les globals
  `LIVE_MEMORY`/`BASELINE_MEMORY` restent présents : conserver `CountingMiMalloc`
  et vérifier les tests de limites, y compris les boucles d’appels externes.
- **`datetime.time`** : le nouveau variant `MontyObject::Time` arrive dans les branches
  d’erreur de `monty_to_json` (`src/convert.rs:142`) et `MontySerialize` (`:194`).
  Pas de rupture de compilation grâce aux wildcards, mais un résultat `time`
  direct ne sera pas sérialisable sans adaptation des deux chemins.
- **REPL** : vérifier décorateurs, chaînes multiligne et conservation des définitions
  entre snippets après la refonte interne des tables de compilation.
- Aucun benchmark fimod réalisé : ne pas reprendre comme gains mesurés les
  optimisations internes upstream (arguments positionnels, REPL, checkpoints).

## New capabilities unlocked

Disponibles upstream après migration et validation, pas encore dans fimod :

- `functools.reduce` et `functools.partial` ; modules `base64` et `binascii`
  (dont les encodeurs/décodeurs ASCII85 ajoutés dans l’intervalle).
- `itertools.takewhile`, `dropwhile`, `filterfalse`, `starmap`, `accumulate`,
  `batched`, `zip_longest`.
- `str.format`, `datetime.time`, options `@dataclass(eq=..., frozen=...)`,
  `object.__setattr__` et protocole `__index__` plus complet.
- Corrections de mutations de conteneurs pendant `repr`, comparaison et itération,
  corrections asyncio et continuation REPL.

Les valeurs doivent toujours être convertibles par fimod : produire des chaînes
pour les encodages bytes et appeler `.isoformat()` sur `time` en attendant son
support natif à la frontière JSON. Ces modules n’ajoutent pas d’autorisation OS.

## Documentation

`docs/reference/monty-engine.md` annonçait encore 0.0.18 : seul ce numéro est corrigé
vers la version réellement verrouillée, 0.0.21, dans cette analyse.
Lors de la migration, revoir les tables classes/dataclasses, stdlib, syntaxe,
les exemples de suspensions et la politique de ressources ; ne pas annoncer
0.0.23 comme moteur actif avant le bump. `notes/DESIGN_NOTES.md` contient aussi
une référence périmée à 0.0.19 dans ses watchpoints, à synchroniser lors du bump.

## Upgrade steps

1. Sur une branche dédiée, adapter le contrat Pipeline/Step à `ClassInstance`,
   aux UUID et au routage des méthodes/attributs ; définir les tests de compatibilité.
2. Après validation du plan, passer **les deux** dépendances crates.io dans
   `Cargo.toml` à `0.0.23`, puis régénérer le lock sans laisser dériver vers une cible
   plus récente : `rtk cargo update -p monty --precise 0.0.23` et
   `rtk cargo update -p monty-types --precise 0.0.23` ; contrôler le lock résultant.
3. Adapter les deux conversions `Time` si son retour direct doit être supporté ;
   traiter explicitement le budget de suspensions (molds et REPL).
4. `rtk cargo build` ; arrêter et diagnostiquer tout échec.
5. Tester Pipeline/Step (get/set, create, insert/append, identité, refus d’attributs),
   noms globaux vs attributs, conversions temporelles, built-ins répétés, limites
   mémoire/temps et continuation REPL ; ajouter des fixtures pour les nouveaux modules.
6. `rtk task lint` puis `rtk task test` ; traiter tout échec avant poursuite.
7. Mettre à jour la référence moteur et le brouillon du changelog fimod, sans toucher
   au `CHANGELOG.md` public avant la release.

## Risk before adaptation

**High** pour un bump isolé : les objets de contrôle du pipeline utilisent directement
une API supprimée. Risque concentré dans `engine.rs`, mais les effets portent sur
le routage, l’identité, la mutabilité et les limites d’exécution. Analyse statique
uniquement ; aucune compilation ni exécution de Monty 0.0.23 effectuée ici.

## Initial recommendation

**Wait** pour le bump seul ; entreprendre la migration ciblée décrite ci-dessus.
Les fonctionnalités sont utiles, mais la compatibilité Pipeline/Step et le contrat
des suspensions doivent être établis avant intégration.

## Implementation record

- `monty` et `monty-types` verrouillés à 0.0.23. La résolution a aussi révélé
  l’incompatibilité de l’ancien pin get-size2=0.10.1 avec Ruff 0.0.9 : retrait du
  pin et des exclusions Dependabot / task outdated ; compact_str passe à 0.10.
- Registre d’objets hôte par mold, UUID aléatoires via getrandom, receivers retrouvés
  par identité. Le dispatch interne conserve explicitement le receiver en premier
  argument. Les instances de steps sont réutilisées par index dans le même mold.
- Les attributs modifiés dans la copie sandbox ne modifient ni les métadonnées hôte
  ni les specs Step.create. L’ancien champ frozen ne peut plus empêcher l’affectation
  locale ; les décisions du pipeline reposent désormais sur la copie hôte.
- `datetime.time` sérialisé dans les deux chemins, avec microsecondes et offsets
  jusqu’à la seconde. REPL décorateurs/triple-quoted et modules nouveaux couverts.
- Choix utilisateur : quota configurable **désactivé par défaut**. Clé TOML entière
  `max_suspensions`, absente ou 0 = désactivée. Setup get/set/show et presets intégrés.
  Quota indépendant par mold / snippet REPL ; N autorisé, N+1 refusé avant action.
  Arrêt par abandon de la suspension, non interceptable en Python ; session REPL
  récupérée. Tests de seuil, dépassement, configuration invalide, conservation lors
  d’un autre setup set, désactivation et plus de 1000 appels sans quota.
- Référence moteur et CLI mises à jour ; aucun gain de performance revendiqué.

### Validation locale

- `rtk cargo build` : succès après résolution du pin Ruff périmé.
- `rtk task lint` : succès (fmt, clippy --all-targets -D warnings, cargo-deny).
  Cargo-deny conserve des avertissements non bloquants de dépendances dupliquées.
- `rtk task test` : 675 tests réussis ; 10 ignorés par configuration (performance
  et doctest). Tests exécutés avec accès aux sockets des serveurs HTTP locaux.
- `rtk git diff --check` : succès.
- `rtk task doc:build` : succès, aucun problème signalé par Zensical.
- Un premier passage restreint échouait sur les sockets HTTP et le verrou du cache
  cargo-deny ; relance avec permissions adaptées réussie. Un lancement ciblé du
  pipeline a aussi terminé avec SIGABRT sans diagnostic ; sa relance brute séquentielle
  et la suite complète parallèle ont ensuite réussi. Cause non établie.

La migration est prête pour revue locale. Aucune validation CI distante, release,
mesure de performance ou publication n’a été effectuée.
