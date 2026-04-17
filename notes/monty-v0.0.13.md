# Monty v0.0.13

- **Date**: 2026-04-17 (analysis)
- **Current fimod version**: v0.0.11
- **Monty release**: v0.0.13

## Changes

### Correct types for datetime os calls + `not_handled` (#332)
Bugfix for datetime-related os function return types.

### `os` and `mount` on start (#337)
`os` and `mount` modules are now loaded at interpreter startup instead of lazily.
Means `os.path` etc. are available immediately in mold scripts.

## Breaking changes for fimod
None.
