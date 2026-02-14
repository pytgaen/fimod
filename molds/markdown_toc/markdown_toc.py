"""
Extract the table of contents from a Markdown file.

Usage:
  fimod s -i README.md -m @markdown_toc
"""
# fimod: input-format=lines
# fimod: output-format=json

def transform(data, args, env, headers):
    toc = []
    for line in data:
        match = re_match(r"^(#+)\s+(.+)", line)
        if match:
            groups = match["groups"]
            level = len(groups[0])
            title = groups[1].strip()
            slug = re_sub(r"[^a-z0-9\-]", "", title.lower().replace(" ", "-"))
            toc.append({
                "level": level,
                "title": title,
                "anchor": slug
            })
    return toc
