---
name: rtk-cli-optimizer
version: 0.3.0
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

Contexte sur `rtk` :

- `rtk` est un proxy CLI qui compresse la sortie de commandes très verbeuses
  avant de les envoyer au modèle.
- Il est particulièrement utile pour : `git`, `gh`, `cargo`, `npm`, `pnpm`,
  `pytest`, `vitest`, `playwright`, `go test`, `docker`, `kubectl`, `ls -R`,
  `find`, `grep -R`, et d'autres commandes générant beaucoup de texte.
- La commande `rtk gain` permet de voir combien de tokens sont économisés
  par commande et au total.

Assumptions :

- La commande `rtk --version` fonctionne (rtk est dans le PATH).
- La commande `rtk gain` fonctionne (sinon tu dois considérer que les stats
  ne sont pas disponibles).

Règles de réécriture (sans hook) :

1. **Commandes candidates à `rtk`**

   Pour les commandes suivantes, tu dois *envisager* d'utiliser `rtk` comme proxy
   en les réécrivant explicitement sous la forme `rtk <commande>` :

   - Git : `git status`, `git diff`, `git log`, `git show`, `git blame`,
     `gh pr diff`, `gh pr view` (si la sortie est volumineuse).
   - Tests : `pytest`, `cargo test`, `npm test`, `pnpm test`, `vitest`,
     `playwright test`, `go test`.
   - Build / CI verbeux : `cargo build`, `npm run build`, `pnpm build`,
     jobs qui produisent beaucoup de logs.
   - Listings / recherche : `ls -R`, `find`, `grep -R`, `rg` sur de gros arbres.
   - Conteneurs / cluster : `docker ps`, `docker compose ps`, `kubectl get ...`
     quand le nombre de ressources est important.

   Quand tu décides d'utiliser `rtk`, tu dois lancer **exactement** :

   - `rtk <commande d'origine et ses arguments>`

   Exemple :

   - Commande brute : `git diff`
   - Commande optimisée : `rtk git diff`

2. **Commandes à ne pas réécrire automatiquement**

   Tu ne dois **pas** ajouter `rtk` devant les commandes suivantes :

   - Éditeurs / outils interactifs : `vim`, `nvim`, `nano`, `less`, `more`,
     `tmux`, etc.
   - Commandes qui attendent de l'entrée utilisateur en continu (REPL, shells,
     prompts interactifs).
   - Scripts ou binaires spécifiques du projet qui manipulent des fichiers
     de manière délicate si tu n'es pas sûr que `rtk` soit transparent.

   Si tu as un doute sur une commande, **ne la réécris pas** avec `rtk` sans
   raison claire liée à la verbosité.

3. **Bypass explicite de `rtk`**

   Si tu as utilisé `rtk <commande>` et que la sortie te semble trop
   condensée pour diagnostiquer un problème (par exemple, tu ne vois pas
   un message d'erreur précis ou des détails de diff importants) :

   - Relance la commande **sans** `rtk`.
   - Dans ta réponse à l'utilisateur, indique clairement :
     - que tu as d'abord utilisé `rtk` pour réduire la verbosité,
     - que tu as dû relancer la commande brute pour obtenir plus de détails.

4. **Communication claire avec l'utilisateur**

   À chaque fois que tu t'appuies sur `rtk`, précise dans ta réponse :

   - quelle commande tu as exécutée exactement (par ex. `rtk git diff`),
   - ce que `rtk` a permis de condenser (par ex. "résumé des fichiers modifiés,
     nombre de lignes ajoutées/supprimées").

   Si l'utilisateur demande à voir la version complète de la sortie :

   - relance sans `rtk` (commande brute),
   - mentionne explicitement que tu fournis la sortie non condensée.

5. **Utilisation de `rtk gain` pour affiner**

   Tu peux périodiquement exécuter :

   - `rtk gain`

   et t'en servir pour :

   - indiquer à l'utilisateur les commandes pour lesquelles `rtk` fait gagner
     le plus de tokens,
   - suggérer d'étendre ou de réduire l'utilisation de `rtk` sur certaines
     classes de commandes.

   N'invente pas de chiffres : base-toi uniquement sur ce que renvoie
   `rtk gain`.

6. **Sécurité et réversibilité**

   Tu ne modifies jamais la configuration globale de l'environnement
   (pas de `rtk init --global`, pas de modification de hooks ou de fichiers
   de configuration système).

   Tu te contentes de :

   - choisir quand préfixer une commande par `rtk`,
   - expliquer à l'utilisateur comment il peut reproduire ou désactiver
     cette optimisation.

Résumé :

- Utilise `rtk <commande>` pour les commandes très bruyantes où la perte
  de détails n'empêche pas de travailler.
- Reviens à la commande brute si tu as besoin de détails fins.
- Ne touche pas à la configuration système ; tout passe par le choix explicite
  des commandes que tu exécutes.
