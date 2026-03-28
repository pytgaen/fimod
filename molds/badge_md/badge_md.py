"""
Generate a shields.io badge in Markdown.

Usage:
  echo '{"label":"build","status":"passing","color":"green"}' | fimod s -m @badge_md -O txt
  fimod s input.json -m @badge_md -O txt
"""
# fimod: output-format=txt
# fimod: arg=label   Badge label (overrides input)
# fimod: arg=status  Badge status text (overrides input)
# fimod: arg=color   Badge color (overrides input)

def transform(data, args, env, headers):
    ctx = {
        "label": args.get("label") or data.get("label", "badge"),
        "status": args.get("status") or data.get("status", "unknown"),
        "color": args.get("color") or data.get("color", "lightgrey"),
    }
    return tpl_render_str(
        "![{{ label }}](https://img.shields.io/badge/{{ label }}-{{ status }}-{{ color }})",
        ctx
    )
