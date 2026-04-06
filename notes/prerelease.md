# Prerelease & Public E2E Plan

## 1. Skill `/prerelease`

**File:** `.claude/skills/prerelease/SKILL.md`

**Invocation:**
```
/prerelease              # auto-detect version from commits, bump to X.Y.Z-rc.1
/prerelease 0.3.0        # force 0.3.0-rc.1 (or rc.2 if rc.1 tag exists)
/prerelease --finalize   # strip -rc.N, hand off to /release
```

**Steps:**

1. **Pre-flight** — like `/release` but allows non-main branches (that's the point)
2. **Determine target version**
   - No argument: analyze commits since last tag, compute semver bump -> `X.Y.Z`
   - With argument: use the given version
3. **Determine rc.N suffix**
   - `git tag -l "vX.Y.Z-rc.*"` -> find latest rc.N, increment
   - If none -> rc.1
4. **Bump `Cargo.toml`** to `X.Y.Z-rc.N`
5. **`cargo build`** to sync Cargo.lock
6. **Present summary**, wait for user confirmation
7. **Commit + tag**: `chore(prerelease): X.Y.Z-rc.N` + tag `vX.Y.Z-rc.N`
8. **Optional push** — triggers CI + pre-release workflow

## 2. Workflow: `prerelease.yml`

**File:** `.github/workflows/prerelease.yml`

**Trigger:** tags `v*-rc.*`

**Jobs:**

```
build  ──>  github-release (prerelease: true)  ──>  e2e-install
```

- **build** — reuse release.yml logic (cross-compile, UPX, checksums). Lighter matrix: fewer targets, no Docker.
- **github-release** — create GitHub Release with `prerelease: true`. Assets are downloadable via public `curl`, but won't appear as "latest".
- **e2e-install** — public e2e tests (see below).

## 3. Job `e2e-install`

**Runs on:** `ubuntu-latest` (optional `macos-14`)

**Steps:**

```bash
# 1. Install from the pre-release (curl | sh, public, no token)
FIMOD_VERSION=vX.Y.Z-rc.N sh install.sh
fimod --version  # verify correct version

# 2. Registry setup (fresh HOME)
export HOME=$(mktemp -d)
fimod registry setup --yes
fimod registry list  # verify registry exists

# 3. Migration test (simulate existing user with "official")
export HOME=$(mktemp -d)
mkdir -p $HOME/.config/fimod
cat > $HOME/.config/fimod/sources.toml << 'EOF'
[official]
url = "https://raw.githubusercontent.com/pytgaen/fimod-powered/main"
priority = 50
EOF
fimod registry setup --yes
# verify: "official" gone, "examples" (or new name) present
fimod registry list --output-format json | grep -v official
fimod registry list --output-format json | grep examples

# 4. Smoke test
echo '[{"a":1},{"a":2}]' | fimod shape -e 'data[0]' --output-format json
```

## 4. Modify `release.yml`

Current trigger `v[0-9]+.*` matches rc tags too. **Option A (recommended):** restrict to `v[0-9]+.[0-9]+.[0-9]+` (no suffix) so rc tags are handled exclusively by `prerelease.yml`. Two separate workflows = cleaner separation.

## 5. What stays private (fimod-tools)

Only `install_private.sh` (requires GitHub PAT for private repo access). Everything else — install from public release, migration, registry setup — can live in the public CI.

## 6. Local pre-release testing (`/local-prerelease`)

**Implemented:** `scripts/local-prerelease.sh` + `.claude/skills/local-prerelease/SKILL.md`

Builds the release binary locally and runs e2e tests in a disposable container (docker or incus, auto-detected). No publishing, no GitHub needed.

```bash
./scripts/local-prerelease.sh              # full: build + test
./scripts/local-prerelease.sh --skip-build # reuse last binary
./scripts/local-prerelease.sh --keep       # keep container for debugging
```

## 7. Registry migration test cases (0.2.0 → 0.3.0)

Migration logic lives in `registry::setup()`. The legacy "official" registry URL is `https://github.com/pytgaen/fimod/tree/main/molds` — migration triggers only when this exact URL matches.

Implemented in `scripts/local-prerelease.sh`:

- [x] **1: Fresh install** — no config → creates `fimod-powered` (P10) + `examples` (P99)
- [x] **2: Migrate 0.2.0 user** — `official` (legacy URL) → renamed to `examples` (P99) + `fimod-powered` (P10) added
- [x] **3: Migrate with custom registries** — `official` + `mycompany` → migration preserves custom registries
- [x] **4: Already migrated** — second `setup --yes` is a no-op ("already configured")
- [x] **5: Name collision** — `fimod-powered` name already taken by custom registry → installs under `fimod-fimod-powered`
- [x] **6: Smoke test** — inline expression pipeline works
- [x] **7: install.sh** — `FIMOD_SKIP_DOWNLOAD=1 FIMOD_SETUP_REGISTRY=yes` path works

Still to discuss:
- [ ] Partial/corrupt sources.toml → graceful handling?
- [ ] Priority values: are custom priorities preserved or overwritten during migration?

## Files summary

| File | Action |
|---|---|
| `.claude/skills/prerelease/SKILL.md` | New — `/prerelease` skill |
| `.github/workflows/prerelease.yml` | New — build + pre-release + e2e |
| `.github/workflows/release.yml` | Modify trigger to exclude `-rc.*` |
| `scripts/local-prerelease.sh` | New — local e2e test runner (docker/incus) |
| `.claude/skills/local-prerelease/SKILL.md` | New — `/local-prerelease` skill |
