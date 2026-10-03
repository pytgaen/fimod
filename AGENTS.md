@AGENTS.base.md

# Fimod agent instructions

Read [AGENTS.base.md](AGENTS.base.md) once if it has not already been loaded.
It defines shared execution and Git rules; this file adds Fimod-specific rules.

Fimod is a Rust CLI that transforms structured data with Python mold scripts
executed by [Monty](https://github.com/pydantic/monty). It ships as a standalone
binary and requires no system Python installation.

## Read the relevant source of truth

Use these references when the task touches their subject; do not load the whole
set for every edit. Repository evidence takes precedence over recalled behavior.

| Task or decision | Reference |
| --- | --- |
| Product scope, non-goals, major design tradeoffs | `notes/VISION.md` |
| Pipeline, module responsibilities, sandbox boundaries | `notes/ARCHITECTURE.md` |
| File placement, test and documentation locations | `notes/CODE_LAYOUT.md` |
| Tooling, implementation conventions, Monty watchpoints | `notes/DESIGN_NOTES.md` |
| Roadmap scope or migration of completed items | `notes/ROADMAP.md` |
| Commits, PRs, prereleases, releases | `notes/release-workflow.md` and the relevant workflow skill |

Build tooling is managed through `mise.toml`; use `Taskfile.yml` for maintained
commands. Check `Cargo.toml` and the implementation for current dependency APIs
and versions instead of keeping version-specific instructions here.

## Boundaries to preserve

- Rust owns format parsing/serialization and all host capabilities. Monty handles
  structured values; filesystem, environment, clock, and other host access remain
  gated by Rust's sandbox policy. Do not bypass it to fill Python-runtime gaps.
- `serde_json::Value` is the I/O representation; mold chains carry `MontyObject`
  between steps. Preserve native identity conversion and direct serialization
  paths where applicable; consult the architecture before changing conversions.
- Molds define `transform(data, **_)`, declaring only the keyword parameters they
  use (`args`, `env`, `headers`, `pipeline`). Environment exposure is explicit via
  `--env`; preserve that contract.
- Keep stdout for data. Diagnostics and debug output, including mold prints in
  debug mode, belong on stderr.
- Prefer dedicated function variants when behaviors diverge substantially
  (existing example: `re_sub` / `re_sub_fancy`).

## Tools and verification

Use `rtk` for verbose CLI commands (`cargo`, `git`, `gh`, searches, builds, tests).
Prefix each verbose command in a chain and the producer in a pipe. Exact-output
reads and interactive tools may run directly. If filtering hides a failure, read
RTK's tee file, then use `rtk run <command>` for raw output when needed.

Choose validation by the change:

- Rust code, dependencies, or build/CI changes: run focused checks first, then
  `rtk task lint` (fmt, clippy, cargo-deny) and `rtk task test` before completion.
- Mold changes: run affected fixtures using the `mold-tests` skill; regenerate
  `molds/catalog.toml` when a public mold is added or modified.
- Published documentation: check affected examples and run `rtk task doc:build`
  when rendering or navigation is affected.
- Agent instructions or wording-only edits: review the diff, references, and
  consistency; `rtk git diff --check` is sufficient without rebuilding Rust.

Useful focused commands:

```bash
rtk cargo test --test cli <topic>    # CLI integration tests in tests/cli/
rtk cargo test --lib <name>          # Unit tests alongside Rust code
rtk cargo test --test molds_test     # Mold fixtures in tests-molds/
```

Run relevant local checks and fix failures caused by the requested change without
asking at each iteration. Report unrelated failures and unavailable checks
explicitly; do not expand the patch to fix unrelated work.

Update affected user documentation under `docs/guides/`, `docs/reference/`, or
`README.md` when behavior changes. When a roadmap item ships, move its useful
content into user docs rather than merely marking it complete in the roadmap.

## Git and release invariants

Use the `release-workflow` skill for Git closeout and releases, and
`prerelease-workflow` for prerelease validation. These workflows do not grant
permission to publish.

- Feature/fix commits belong on dedicated branches and go through a PR; never
  commit them directly on `main`. Use Conventional Commits and squash merges.
- Update `CHANGELOG.md` only in a release commit, whose subject is exactly
  `chore(release): X.Y.Z`, made directly on `main`.
- That commit contains only `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, plus removal
  of the transient `notes/changelog-X.Y.Z.md` consumed by the release.
- Create the stable tag `vX.Y.Z` on `main` immediately after the release commit.
  Follow the separate prerelease workflow for `rc.N` tags.
