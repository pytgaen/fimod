# markdown_toc

Extract the table of contents from a Markdown file. Returns heading levels, titles, and URL-friendly anchors.

## Usage

```bash
fimod s -i README.md -m @markdown_toc
```

## Example

**Input** (`doc.md`):
```markdown
# Getting Started
## Installation
## Quick Tour
### First Steps
# API Reference
```

**Output**:
```json
[
  {"level": 1, "title": "Getting Started", "anchor": "getting-started"},
  {"level": 2, "title": "Installation", "anchor": "installation"},
  {"level": 2, "title": "Quick Tour", "anchor": "quick-tour"},
  {"level": 3, "title": "First Steps", "anchor": "first-steps"},
  {"level": 1, "title": "API Reference", "anchor": "api-reference"}
]
```

### Output as YAML

```bash
fimod s -i README.md -m @markdown_toc --output-format yaml
```
