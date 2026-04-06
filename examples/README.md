# Examples

This directory contains standalone mold scripts that demonstrate common fimod patterns.

## Usage

```bash
fimod s -i input.json -m examples/jq_filter.py
```

## Contents

- `jq_filter.py`: Filter a list of objects based on criteria (like `jq 'map(select(...))'`).
- `jq_map.py`: Map/transform objects to a new structure (like `jq 'map({...})'`).

## Looking for more molds?

Production-ready molds are available in the [fimod-powered](https://github.com/pytgaen/fimod-powered) registry:

```bash
fimod registry add fimod-powered https://github.com/pytgaen/fimod-powered
fimod mold list
```
