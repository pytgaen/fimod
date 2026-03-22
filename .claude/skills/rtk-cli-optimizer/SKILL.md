---
name: rtk-cli-optimizer
version: 0.4.0
description: Utiliser rtk comme proxy CLI dans Claude Code, sans hook automatique, de manière contrôlée et réversible.
type: hook
category: development
subcategory: cli
---

Tu travailles dans un environnement où l'outil `rtk` (Rust Token Killer)
est installé, mais **aucun hook automatique de réécriture (rtk-rewrite.sh) n'est activé**.

Ton rôle :

- Décider **quand** utiliser explicitement `rtk <commande>` au lieu de
  la commande brute.
- Ne **jamais** réécrire implicitement des commandes sensibles.
- Expliquer clairement à l'utilisateur quand tu passes par `rtk`
  et comment le bypasser.

## Contexte sur `rtk`

`rtk` est un proxy CLI qui compresse la sortie de commandes très verbeuses
avant de les envoyer au modèle. Il est transparent : si une commande n'a pas
de filtre dédié, `rtk` la passe telle quelle — il est donc toujours sûr de
l'utiliser.

La commande `rtk gain` permet de voir combien de tokens sont économisés par
commande et au total.

**Assumptions :**
- `rtk --version` fonctionne (rtk est dans le PATH).
- `rtk gain` fonctionne (sinon considère que les stats ne sont pas disponibles).

## Règle d'or

**Toujours préfixer avec `rtk`**, y compris dans les chaînes `&&` :

```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## Commandes candidates à `rtk` (avec gains typiques)

### Build & Compile — 80–90% d'économie
```bash
rtk cargo build         # sortie Cargo build
rtk cargo check         # sortie Cargo check
rtk cargo clippy        # avertissements Clippy groupés par fichier (80%)
```

### Tests — 90–99% d'économie
```bash
rtk cargo test          # échecs seulement (90%)
rtk vitest run          # échecs seulement (99.5%)
rtk playwright test     # échecs seulement (94%)
```

### Git — 59–80% d'économie
```bash
rtk git status          # statut compact
rtk git log             # log compact (tous flags compatibles)
rtk git diff            # diff compact (80%)
rtk git show            # show compact (80%)
rtk git add             # confirmations ultra-compactes (59%)
rtk git commit          # confirmations ultra-compactes (59%)
rtk git push / pull / fetch / branch / stash / worktree
```

Note : le passthrough fonctionne pour **tous** les sous-commandes git, même
ceux non listés explicitement.

### GitHub CLI — 26–87% d'économie
```bash
rtk gh pr view <num>    # PR compact (87%)
rtk gh pr checks        # checks compact (79%)
rtk gh run list         # workflow runs compact (82%)
rtk gh issue list       # issues compact (80%)
rtk gh api              # réponses API compact (26%)
```

### JavaScript / TypeScript — 70–90% d'économie
```bash
rtk pnpm list / outdated / install
rtk npm run <script>
rtk npx <cmd>
```

### Fichiers & Recherche — 60–75% d'économie
```bash
rtk ls <path>           # arborescence compacte (65%)
rtk grep <pattern>      # résultats groupés par fichier (75%)
rtk find <pattern>      # résultats groupés par répertoire (70%)
```

### Infrastructure — 85% d'économie
```bash
rtk docker ps / images / logs <c>
rtk kubectl get / logs
```

### Réseau — 65–70% d'économie
```bash
rtk curl <url>          # réponses HTTP compactes (70%)
rtk wget <url>          # sortie de téléchargement compacte (65%)
```

## Commandes à ne pas réécrire

Tu ne dois **pas** ajouter `rtk` devant :

- Éditeurs / outils interactifs : `vim`, `nvim`, `nano`, `less`, `tmux`, REPLs.
- Commandes où la sortie exacte est nécessaire pour diagnostiquer un problème.
- Scripts spécifiques du projet si tu n'es pas certain que `rtk` soit transparent.

En cas de doute, **ne réécris pas** avec `rtk` sans raison claire liée à la verbosité.

## Bypass explicite

Si la sortie `rtk` est trop condensée pour diagnostiquer un problème :

- Relance **sans** `rtk`.
- Indique clairement dans ta réponse que tu as dû passer à la commande brute.

## Communication avec l'utilisateur

À chaque fois que tu t'appuies sur `rtk`, précise :

- la commande exacte exécutée (ex. `rtk git diff`),
- ce que `rtk` a condensé.

Si l'utilisateur demande la sortie complète, relance sans `rtk` et signale-le.

## `rtk gain` pour affiner

Exécute `rtk gain` pour :

- indiquer les commandes les plus rentables,
- suggérer d'étendre ou de réduire l'utilisation selon le profil de la session.

Ne jamais inventer de chiffres — base-toi uniquement sur ce que renvoie `rtk gain`.

## Sécurité et réversibilité

Tu ne modifies jamais la configuration globale :
- pas de `rtk init --global`,
- pas de modification de hooks ou de fichiers de configuration système.

Tout passe par le choix explicite des commandes exécutées.
