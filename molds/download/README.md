# download

Download a file from a URL and save it to disk (wget-like).

## Usage

```bash
fimod s -i https://example.com/file.tar.gz -m @download
```

With a custom output filename:

```bash
fimod s -i https://example.com/file.tar.gz -m @download --arg out=myfile.tar.gz
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `out` | No | Output filename — defaults to the last path segment of the URL |

## Example

```bash
fimod s -i https://github.com/cli/cli/releases/download/v2.45.0/gh_2.45.0_linux_amd64.tar.gz \
  -m @download --arg out=gh.tar.gz
# → writes gh.tar.gz in the current directory
```

## How it works

The mold uses `input-format=http` to get the raw HTTP response body as bytes, then calls
`set_output_format("raw")` and `set_output_file(filename)` to write the binary content
directly to disk — bypassing JSON serialization entirely.

## Mold directives

- `input-format=http` — expose the raw HTTP response (status, headers, body bytes)
