# Poetry Migrate

This mold migrates a Poetry `pyproject.toml` to either **Poetry 2** or **uv** format.

## Supported Migration Paths

- **Poetry 1 → Poetry 2**: Modernizes to PEP 621 `[project]` with `poetry-core` build backend
- **Poetry 1 → uv**: Full migration to PEP 621 with `hatchling` build backend
- **Poetry 2 → uv**: Switches build backend from `poetry-core` to `hatchling`

## Usage

```bash
# Poetry 1 → Poetry 2 (default)
fimod s -i pyproject.toml -m @poetry_migrate -o new_pyproject.toml

# Poetry → uv
fimod s -i pyproject.toml -m @poetry_migrate -o new_pyproject.toml --arg target=uv
```

## Features

- **Metadata Migration**: Maps `[tool.poetry]` metadata to `[project]` (PEP 621).
- **Dependency Conversion**: Converts Poetry-specific constraints (`^`, `~`) to PEP 440 standard (`>=`, `<`).
- **Dev Dependencies**: Moves `dev-dependencies` to `[dependency-groups.dev]` (PEP 735 compatible).
- **Build System**: `poetry-core` for Poetry 2 target, `hatchling` for uv target.

## Detailed Conversion Rules

| Source (Poetry 1/2) | Target (PEP 621) | Logic |
| :--- | :--- | :--- |
| `[tool.poetry]` | `[project]` | 1:1 mapping for `name`, `version`, `description`. Authors/maintainers are parsed from "Name <email>" strings. |
| `dependencies` | `project.dependencies` | Converted to PEP 508 strings. `python` constraint moves to `requires-python`. |
| `dev-dependencies` | `[dependency-groups.dev]` | Legacy dev-dependencies are merged into the `dev` group. |
| `source` | `[[tool.uv.index]]` | Package sources are converted to `uv` index configuration (uv target only). |
| `scripts` | `[project.scripts]` | Direct copy. |
| `packages` | `[tool.hatch.build...]` | Converted to Hatchling configuration (includes copy). |

### Version Mappings

The script translates Poetry's specific operators to standard PEP 440:

- `^1.2.3` (Caret) -> `>=1.2.3, <2.0.0`
- `~1.2.3` (Tilde) -> `>=1.2.3, <1.3.0`
- `*` (Wildcard) -> `>=0.0.0` (Approximation)

## Known Limitations

Some Poetry-specific fields are currently ignored during conversion:

- `allow-prereleases`: This option in dependencies is lost.
- `source` (in dependency): Association of a dependency with a specific source is lost.
- `include` / `exclude` (root level): Only `packages` is processed for the build system.

## Comparison Notes

### Build System
The converter uses `poetry-core>=2.0.0` when targeting Poetry 2, and `hatchling` when targeting uv. While `poetry-core` is compatible with PEP 621, switching to `hatchling` is a common pattern when migrating fully to the `uv` ecosystem.

### Index Strategy
`uv` uses a different index strategy than Poetry. By default, `uv` checks indexes in order. The converter attempts to preserve source definitions but manual review of index priority is recommended for complex setups with private registries.
