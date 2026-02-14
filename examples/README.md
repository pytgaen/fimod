# Examples

This directory contains examples of mold scripts for various use cases.

## Usage

Run any of these scripts with `fimod`:

```bash
fimod s -i input.json -m examples/script_name.py
```

## Contents

- `jq_filter.py`: Filter a list of objects based on criteria (like `jq 'select(...)'`).
- `jq_map.py`: Transform (map) objects to a new structure.
- `merge_files.py`: Merge data from the input file with another file (Read-side pattern).
- `skylos_to_gitlab.py`: Convert **Skylos** JSON report to **GitLab Code Quality** format.
