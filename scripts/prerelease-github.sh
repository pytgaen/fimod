#!/bin/bash
# prerelease-github.sh — Bump Cargo.toml to X.Y.Z-rc.N, commit, tag. Does NOT push.
#
# Usage:
#   ./scripts/prerelease-github.sh X.Y.Z           # auto-detect rc.N (scans existing tags)
#   ./scripts/prerelease-github.sh X.Y.Z N         # force rc.N
#
# Example:
#   ./scripts/prerelease-github.sh 0.5.0           # → v0.5.0-rc.1 (or rc.2 if rc.1 exists)
#   ./scripts/prerelease-github.sh 0.5.0 3         # → v0.5.0-rc.3
#
# Produces:
#   - bumped Cargo.toml + synced Cargo.lock
#   - commit "chore(prerelease): X.Y.Z-rc.N"
#   - tag vX.Y.Z-rc.N
#
# Push is deliberately left to the caller (the skill will ask for confirmation).

set -euo pipefail

FIMOD_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$FIMOD_DIR"

# ── args ────────────────────────────────────────────────────────────────
if [ $# -lt 1 ]; then
  echo "Usage: $0 X.Y.Z [N]" >&2
  exit 2
fi

VERSION="$1"
RC_N="${2:-}"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version '$VERSION' is not X.Y.Z" >&2
  exit 2
fi

# ── working tree check ──────────────────────────────────────────────────
if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is not clean" >&2
  git status --short >&2
  exit 1
fi

# ── auto-detect rc.N ────────────────────────────────────────────────────
if [ -z "$RC_N" ]; then
  LATEST_RC=$(git tag -l "v${VERSION}-rc.*" | sed -E "s/^v${VERSION}-rc\.([0-9]+)\$/\\1/" | sort -n | tail -1)
  if [ -z "$LATEST_RC" ]; then
    RC_N=1
  else
    RC_N=$((LATEST_RC + 1))
  fi
fi

FULL_VERSION="${VERSION}-rc.${RC_N}"
TAG="v${FULL_VERSION}"

# ── refuse if tag already exists ────────────────────────────────────────
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists" >&2
  exit 1
fi

echo "→ Prerelease ${FULL_VERSION}"
echo "  tag: ${TAG}"
echo

# ── bump Cargo.toml ─────────────────────────────────────────────────────
sed -i -E "s/^version = \"[^\"]+\"/version = \"${FULL_VERSION}\"/" Cargo.toml
CURRENT=$(grep '^version' Cargo.toml | head -1)
echo "  Cargo.toml: ${CURRENT}"

# ── sync Cargo.lock ─────────────────────────────────────────────────────
echo "  cargo build (sync Cargo.lock)…"
cargo build --quiet

# ── commit + tag ────────────────────────────────────────────────────────
git add Cargo.toml Cargo.lock
git commit -m "chore(prerelease): ${FULL_VERSION}" --quiet
git tag "${TAG}"

echo
echo "✓ Committed and tagged ${TAG}"
echo
echo "Next step (not done automatically):"
echo "  git push && git push origin ${TAG}"
