# Contributing to fimod

## Table of contents

- [Local development](#local-development)
- [CI pipeline](#ci-pipeline)
- [Release process](#release-process)
- [Documentation deployment](#documentation-deployment)
- [Dependency management (Renovate)](#dependency-management-renovate)
- [Cargo deny — licences & advisories](#cargo-deny--licences--advisories)
- [Changelog (git-cliff)](#changelog-git-cliff)

---

## Local development

```bash
cargo build                  # debug build
cargo test                   # all tests (unit + integration)
cargo test --test cli        # integration tests only
cargo fmt                    # format code
cargo clippy -- -D warnings  # lint (same flags as CI)
cargo audit                  # security audit
```

Required tools for a full local check:

```bash
cargo install cargo-audit
cargo install cargo-deny
```

---

## CI pipeline

**File:** `.github/workflows/ci.yml`
**Triggers:** every push to `main` and every pull request.

```
push / pull_request
│
├── lint          (ubuntu-latest)
│   ├── cargo fmt --check
│   └── cargo clippy --all-targets --all-features -- -D warnings
│
├── test          (matrix: ubuntu-latest, macos-14)
│   └── cargo test --all-features
│
├── msrv          (ubuntu-latest)
│   └── cargo check with Rust 1.75  ← minimum supported version
│
└── security      (ubuntu-latest)
    ├── cargo audit   (RustSec advisory database)
    └── cargo deny    (licences + advisories + git sources)
```

**Caching:** `Swatinem/rust-cache@v2` is active on every job — the first run
compiles everything; subsequent runs on the same branch reuse the cache and
typically complete in under a minute.

The CI must be green before any PR can be merged.

---

## Release process

**File:** `.github/workflows/release.yml`
**Trigger:** pushing a tag matching `v[0-9]+.*` (e.g. `v0.2.0`).

### How to cut a release

```bash
git tag v0.2.0
git push origin v0.2.0
```

That's it. The workflow handles everything else.

### What the workflow does

```
tag v*
│
├── changelog
│   └── git-cliff --latest → CHANGELOG_RELEASE.md (artifact)
│
├── build (matrix, parallel)
│   ├── x86_64-unknown-linux-musl   (ubuntu, cross/Docker)
│   ├── aarch64-unknown-linux-musl  (ubuntu, cross/Docker)
│   ├── aarch64-apple-darwin        (macos-14, native cargo)
│   └── x86_64-pc-windows-msvc     (windows-latest, native cargo)
│
└── release
    ├── download all build artifacts
    ├── compute SHA256 checksums  → fimod-{version}-sha256sums.txt
    └── softprops/action-gh-release → GitHub Release
        ├── body = generated changelog
        ├── prerelease = true  (if tag contains "-", e.g. v0.2.0-beta.1)
        └── assets:
            ├── fimod-{version}-x86_64-unknown-linux-musl.tar.gz
            ├── fimod-{version}-aarch64-unknown-linux-musl.tar.gz
            ├── fimod-{version}-aarch64-apple-darwin.tar.gz
            ├── fimod-{version}-x86_64-pc-windows-msvc.zip
            └── fimod-{version}-sha256sums.txt
```

### Cross-compilation

Linux musl targets use [`cross`](https://github.com/cross-rs/cross), which
runs the compiler inside a Docker image — no native musl toolchain required on
the runner. macOS ARM and Windows are compiled natively on their respective
runners.

The `.cargo/config.toml` file at the repo root documents the musl linker
settings for local builds (when not using `cross`).

### Pre-release vs stable

| Tag | Release type |
|-----|-------------|
| `v1.0.0` | Stable |
| `v1.0.0-beta.1` | Pre-release |
| `v1.0.0-rc.1` | Pre-release |

---

## Documentation deployment

**File:** `.github/workflows/docs.yml`
**Triggers:**
- `push` to `main` (paths: `docs/**`, `mkdocs.yml`) → build **and** deploy
- `pull_request` (same paths) → build only (no deploy)

```
docs/**  or  mkdocs.yml changed
│
├── build-docs   (ubuntu-latest)
│   ├── astral-sh/setup-uv
│   ├── uvx zensical build         ← génère site/
│   └── upload artifact: site/
│
└── deploy-docs  (push to main only)
    ├── download artifact: site/
    └── peaceiris/actions-gh-pages → branch gh-pages
        └── published at https://pytgaen.github.io/fimod
```

To preview the docs locally:

```bash
uvx zensical serve    # http://localhost:8000
```

---

## Dependency management (Renovate)

**File:** `renovate.json`

[Renovate](https://docs.renovatebot.com/) opens automated PRs when
dependencies have updates. Configuration summary:

| Rule | Behaviour |
|------|-----------|
| Schedule | Monday before 09:00 (to batch weekly updates) |
| Cargo patch updates | Auto-merged via PR |
| GitHub Actions | Grouped into a single PR, auto-merged |
| `monty` (git dep) | Tracked via latest commit |
| Security alerts | PR labelled `security`, no delay |

To enable Renovate: install the
[Renovate GitHub App](https://github.com/apps/renovate) on the repository.

---

## Cargo deny — licences & advisories

**File:** `deny.toml`

`cargo deny check` runs in CI (security job). It enforces three things:

1. **Advisories** — denies crates with known CVEs (RustSec database).
2. **Licences** — only permissive licences allowed (MIT, Apache-2.0, BSD-2/3,
   ISC, …). Any dependency introducing a copyleft licence will fail CI.
3. **Sources** — crate registries and git sources must be explicitly
   allowlisted. The `monty` git dependency is pre-approved.

To run locally:

```bash
cargo deny check
```

To add a new allowed licence or suppress a false-positive advisory, edit
`deny.toml` and document the reason in a comment.

---

## Changelog (git-cliff)

**File:** `cliff.toml`

[git-cliff](https://git-cliff.org/) generates the release body automatically
from [Conventional Commits](https://www.conventionalcommits.org/).

Commit prefixes and their changelog section:

| Prefix | Section |
|--------|---------|
| `feat:` | Features |
| `fix:` | Bug Fixes |
| `docs:` | Documentation |
| `perf:` | Performance |
| `refactor:` | Refactoring |
| `test:` | Testing |
| `chore:` | Miscellaneous |

Breaking changes must include `BREAKING CHANGE:` in the commit body.

To preview the changelog for the next release locally:

```bash
cargo install git-cliff
git cliff --latest
```
