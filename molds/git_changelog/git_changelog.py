"""
Generate a Markdown changelog from a JSON array of data.

Each commit object needs: hash, msg, date.

Usage:
  fimod s data.json -m @git_changelog --output-format txt
"""
# fimod: output-format=txt
# fimod: arg=title  Changelog title (default: Changelog)

def transform(data, args, **_):
    title = args.get("title", "Changelog")

    # Group by date
    by_date = {}
    for c in data:
        date = c.get("date", "unknown")
        by_date.setdefault(date, []).append(c)

    # Sort dates descending
    dates = sorted(by_date.keys(), reverse=True)

    ctx = {
        "title": title,
        "dates": dates,
        "by_date": by_date,
        "total": len(data),
    }

    return tpl_render_from_mold("templates/changelog.md.j2", ctx)
