# git_changelog

Generate a Markdown changelog from `git log` output.

Demonstrates `tpl_render_from_mold` with a `.j2` template file.

## Usage

```bash
git log --pretty='{"hash":"%h","msg":"%s","date":"%cs"}' \
  | fimod s -m @git_changelog -O txt
```

Output:

```markdown
# Changelog

3 commits.

## 2026-03-29

- feat(template): add Jinja2 templating engine (e813bcb)
- fix(core): extract pipeline logic (3afdf56)

## 2026-03-28

- feat(cache): add registry mold cache (48637d1)
```

With a custom title:

```bash
git log --pretty='{"hash":"%h","msg":"%s","date":"%cs"}' -n 10 \
  | fimod s -m @git_changelog --arg title="Release Notes" -O txt
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `title` | No | Changelog title (default: `Changelog`) |

## How it works

Reads JSON-lines from stdin (one `{"hash","msg","date"}` per line), groups commits by date,
then renders the result through `templates/changelog.md.j2` using `tpl_render_from_mold`.
