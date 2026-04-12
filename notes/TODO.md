Créer SECURITY.md
Créer CODE_OF_CONDUCT.md

----

Problème : résolution de chemin avec `-m ../path/.../a.py`

Quand on passe un chemin fichier direct à `-m`, fimod ne sait pas distinguer deux intentions :

1. Chemin vers un répertoire de mold — l'utilisateur donne le dossier, fimod cherche le script principal (`transform.py` / unique `.py`) selon les conventions habituelles.
2. Chemin vers un fichier `.py` isolé — l'utilisateur pointe directement un script ; fimod doit vérifier s'il est le seul `.py` dans son répertoire parent (= structure de mold valide) pour pouvoir résoudre les assets associés (templates `.j2`, etc.).

Enjeu principal : tant que ce cas n'est pas traité, un mold qui référence un template Jinja2 (`.j2`) situé à côté de lui via `tpl_render_from_mold` ne peut pas être chargé par chemin relatif externe.


----

Title: fimod mold show — support --path to inspect a mold by file path

Description:

Currently fimod mold show <name> only resolves molds installed in a registry. This makes it impossible for tools (e.g. the VS Code extension) to display detailed information about a workspace mold that is just a .py file in the user's project.

Proposed change:

Add a --path <file> option to fimod mold show that inspects a mold file directly on disk, without requiring it to be installed in any registry.

```
fimod mold show --path ./molds/upper.py --output-format json
```

The output should be identical in structure to the existing fimod mold show <name> --output-format json — same JSON fields (name, description, source_path, readme_path, input_format, output_format, args), derived by parsing the file at the given path.

When --path is used, registry can be null.

Motivation:

The VS Code extension sidebar lists workspace molds (local .py files detected via glob). To open the detail webview with playground for these molds, the extension needs the same structured metadata that fimod mold show returns. Without --path, the extension would have to duplicate the mold parsing logic in TypeScript, which goes against the architecture principle of the extension being a pure UI layer over the CLI.


-----