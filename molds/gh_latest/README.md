# gh_latest

Get the latest release tag or full download URL from a GitHub repository.

## Usage

### Tag only

```bash
fimod s -i https://github.com/sigstore/cosign/releases/latest -m @gh_latest
# → v3.0.5
```

### Full download URL (fixed asset name)

```bash
fimod s -i https://github.com/sigstore/cosign/releases/latest -m @gh_latest \
  --arg repo="sigstore/cosign" --arg asset="cosign-linux-amd64"
# → https://github.com/sigstore/cosign/releases/download/v3.0.5/cosign-linux-amd64
```

### Asset name with version placeholder

Asset names support `{tag}` (e.g. `v3.0.5`) and `{version}` (e.g. `3.0.5`) placeholders:

```bash
fimod s -i https://github.com/sinelaw/fresh/releases/latest -m @gh_latest \
  --arg repo="sinelaw/fresh" --arg asset='fresh-editor_{version}-1_amd64.deb'
# → https://github.com/sinelaw/fresh/releases/download/v0.1.0/fresh-editor_0.1.0-1_amd64.deb
```

### Download the asset in one shot

Combine with a second `fimod` call to fetch the binary:

```bash
fimod s -i https://github.com/sinelaw/fresh/releases/latest \
  -m @gh_latest \
  --arg repo="sinelaw/fresh" \
  --arg asset='fresh-editor_{version}-1_amd64.deb' \
  | fimod s -I - --output-format raw -O
```

Or more classically:

```bash
fimod s -i "$(fimod \
  -i https://github.com/sinelaw/fresh/releases/latest \
  -m @gh_latest \
  --arg repo="sinelaw/fresh" \
  --arg asset='fresh-editor_{version}-1_amd64.deb')" \
  --output-format raw -o fresh-editor.deb
```


## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `repo` | No | GitHub `owner/repo` (needed for URL mode) |
| `asset` | No | Asset filename pattern — supports `{tag}` and `{version}` placeholders |

When both `repo` and `asset` are provided, the mold outputs the full download URL. Otherwise, it outputs the tag.

## How it works

GitHub's `/releases/latest` endpoint returns a 302 redirect to the latest release page.
The mold uses `no-follow` to capture the redirect `Location` header, then extracts the tag from the URL path.

## Mold directives

- `input-format=http` — expose the raw HTTP response (status, headers, body)
- `no-follow` — don't follow the 302 redirect
- `raw-mode=no-quote` — output the tag/URL as plain text (no JSON quoting)
