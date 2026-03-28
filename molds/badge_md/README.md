# badge_md

Generate a [shields.io](https://shields.io) badge in Markdown format.

Demonstrates `tpl_render_str` with inline template.

## Usage

```bash
echo '{"label":"build","status":"passing","color":"green"}' \
  | fimod s -m @badge_md -O txt
```

Output:

```
![build](https://img.shields.io/badge/build-passing-green)
```

With args override:

```bash
fimod s --no-input -m @badge_md -O txt \
  --arg label=coverage --arg status=98% --arg color=brightgreen
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `label` | No | Badge label (overrides input) |
| `status` | No | Badge status text (overrides input) |
| `color` | No | Badge color (overrides input) |

## How it works

Uses `tpl_render_str` to render an inline Jinja2 template with the input data or args.
