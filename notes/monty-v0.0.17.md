# Monty v0.0.17 — Impact on fimod

Date: 2026-04-22
Previous: v0.0.14 → v0.0.17 (skips v0.0.15, v0.0.16 in fimod's history)

## Summary

Three successive Monty releases rolled up: bugfixes (GC coverage, overflow panics,
empty tuple memory accounting), a pair of additive builtins (`hasattr`, `setattr`),
and a small Python syntax addition (chain assignment). **No breaking change** on
the public API surface that fimod consumes.

## API surface consumed by fimod

Regenerated from `src/` at this run (not memoized):

| Symbol | Fichiers | Rôle |
|---|---|---|
| `MontyObject` | convert, dotpath, engine, env_helpers, exit_control, format, format_control, gatekeeper, hash, iter_helpers, main, msg, pipeline, regex, template | Type de base échangé avec le VM Python |
| `DictPairs` | convert, env_helpers, format, iter_helpers, regex | Paires ordonnées pour les dicts Python |
| `MontyRun` | engine.rs:175,196 | Runner itératif compile-puis-run |
| `RunProgress` | engine.rs:213-277 | Enum pilotant la boucle d'exécution (5 variants, tous matchés) |
| `PrintWriter`, `PrintWriterCallback` | engine.rs:45-60,204-272 | Redirection stdout (StderrPrint implémente PrintWriterCallback) |
| `ResourceLimits`, `LimitedTracker`, `NoLimitTracker` | engine.rs:198,285-291 ; main.rs:587 | Sandboxing CPU/mémoire |
| `OsFunction` | engine.rs:304-325 | Dispatch appels OS (Date/DateTime/Getenv/GetEnviron + wildcard) |
| `NameLookupResult` | engine.rs:255-263 | Résolution dynamique de noms (re_sub, json_stringify, …) |
| `ExtFunctionResult` | engine.rs:365-372 | Conversion OsCallOutcome → résultat VM (variants Return, Error) |
| `MontyException`, `ExcType` | engine.rs:365,384,409-414 | Exceptions Python typées (PermissionError, TimeoutError, MemoryError) |
| `MontyDate`, `MontyDateTime` | engine.rs:335-361 | Construction manuelle pour today/now |
| `MontyRepl`, `NoLimitTracker`, `detect_repl_continuation_mode`, `ReplContinuationMode` | main.rs:573-647 | REPL interactif |

## Changes impacting fimod

| Symbole | Changé ? | Nature | Action |
|---|---|---|---|
| `MontyObject` | non (feature flag interne renommé) | — | RAS |
| `DictPairs` | non | — | RAS |
| `MontyRun::new` | oui (comportement) | Valide input_names comme identifiants Python (erreur `SyntaxError` sinon) | RAS — fimod passe toujours `data`/`args`/`env`/`headers` (valides) |
| `RunProgress` variants | non (5 → 5) | — | RAS — match fimod exhaustif, tous variants couverts |
| `PrintWriter` / `PrintWriterCallback` | non | — | RAS |
| `ResourceLimits::{new, max_duration, max_memory}` | non | — | RAS |
| `ResourceTracker` trait | oui (nouvelle méthode `gc_interval()` requise) | — | RAS — fimod n'implémente pas le trait, utilise les built-ins |
| `OsFunction` | non | — | RAS — arm `_ =>` couvre tout ajout éventuel |
| `NameLookupResult` / `ExtFunctionResult` | non | — | RAS |
| `MontyException::new`, `ExcType::{PermissionError,TimeoutError,MemoryError,SyntaxError}` | non | — | RAS |
| `MontyDate` / `MontyDateTime` champs publics | non | — | RAS |
| `MontyRepl` | additif (`call_function`, `function_names`, `has_function`) | — | RAS — fimod n'utilise que `new`/`feed_run` |

## Breaking changes

**Aucun.**

## New capabilities unlocked

Pour les auteurs de molds :

- **`hasattr(obj, name)`** — builtin Python désormais disponible.
- **`setattr(obj, name, value)`** — builtin Python désormais disponible.
- **Chain assignment** — `a = b = 1` est maintenant supporté (avant : `SyntaxError`).
- **Validation d'entrée plus robuste** — sources avec lone surrogates ou unicode invalide remontent maintenant un `MontyRuntimeError`/`MontySyntaxError` explicite au lieu de panicker.
- **Fiabilité GC** — plusieurs fixes (empty tuple singleton, i64::MIN negation overflow, GC interval respecté) : les molds intensifs en allocations voient moins de cas limites.

Pour fimod lui-même (non exposé aux molds, mais disponible si besoin) :

- `MontyRepl::call_function(name, args, print)` — invoquer une fonction du REPL depuis Rust. Ouvre la voie à `fimod mold test` qui réutiliserait la session plutôt que recompiler.
- `MontyRepl::function_names()` / `has_function(name)` — introspection des fonctions définies. Utile pour `fimod mold list --local` si un jour on charge les molds via REPL.

## Upgrade steps

1. `Cargo.toml` : `tag = "v0.0.14"` → `tag = "v0.0.17"`
2. `cargo update -p monty`
3. `cargo build && cargo test && cargo clippy -- -D warnings`
4. Mettre à jour `docs/reference/monty-engine.md` ligne 5 : `v0.0.14` → `v0.0.17`
5. Documenter les nouveaux builtins (`hasattr`, `setattr`) et chain assignment dans la section "Python features supported" de `docs/reference/monty-engine.md`

## Risk

**low.** Zéro changement de signature publique sur la surface consommée. La seule nouvelle validation (`MontyRun::new` → identifiants Python) est satisfaite par les noms hardcodés de fimod.

## Recommendation

**upgrade now.** Bénéfices clairs (bugfixes GC, deux builtins, chain assignment),
risque minimal. À valider par CI (`cargo test --test cli` + `cargo test --test molds_test`).
