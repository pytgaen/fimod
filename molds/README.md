# Molds

Reusable transform scripts for `fimod`. Each subdirectory contains a mold with its own README showing usage examples.

Browse available molds: `fimod mold list`

## Usage

Use `@name` to run a mold from a registered catalog:

```bash
fimod s -i data.json -m @flatten_nested
fimod s -i users.json -m @pick_fields --arg fields=name,email
```

See [`catalog.toml`](catalog.toml) for the full list with descriptions.

## Contribute a mold

Add a subdirectory here with your script and a `README.md`, then open a PR.
The mold will be bundled automatically in the next release.
