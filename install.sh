#!/bin/sh
# fimod installer — https://github.com/pytgaen/fimod
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
#
# Options (environment variables):
#   FIMOD_VARIANT   standard (default) or full (includes HTTP mold loading)
#   FIMOD_INSTALL   install directory (default: /usr/local/bin, falls back to ~/.local/bin)
#   FIMOD_VERSION   specific version to install (default: latest)
#   FIMOD_SOURCE    github (default) or gitlab

set -eu

REPO="pytgaen/fimod"
VARIANT="${FIMOD_VARIANT:-standard}"
SOURCE="${FIMOD_SOURCE:-github}"

# ── Source-specific base URLs ─────────────────────────────────────────

GL_PROJECT_PATH="pytgaen-group%2Ffimod"
GL_PKG_BASE="https://gitlab.com/api/v4/projects/${GL_PROJECT_PATH}/packages/generic/fimod"

case "$SOURCE" in
  gitlab)
    BASE_URL="$GL_PKG_BASE"
    ;;
  github|*)
    BASE_URL="https://github.com/${REPO}/releases"
    ;;
esac

# ── Detect platform ──────────────────────────────────────────────────

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "macos" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
    *) echo "unsupported" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)   echo "x86_64" ;;
    aarch64|arm64)   echo "aarch64" ;;
    *) echo "unsupported" ;;
  esac
}

OS=$(detect_os)
ARCH=$(detect_arch)

if [ "$OS" = "unsupported" ] || [ "$ARCH" = "unsupported" ]; then
  echo "Error: unsupported platform $(uname -s) / $(uname -m)" >&2
  exit 1
fi

# ── Map to Rust target triple ────────────────────────────────────────

case "${OS}-${ARCH}" in
  linux-x86_64)   TARGET="x86_64-unknown-linux-musl";  EXT="tar.gz" ;;
  linux-aarch64)  TARGET="aarch64-unknown-linux-musl";  EXT="tar.gz" ;;
  macos-aarch64)  TARGET="aarch64-apple-darwin";        EXT="tar.gz" ;;
  windows-x86_64) TARGET="x86_64-pc-windows-msvc";     EXT="zip" ;;
  *)
    echo "Error: no pre-built binary for ${OS}/${ARCH}" >&2
    echo "Build from source: cargo install --git https://github.com/${REPO}" >&2
    exit 1
    ;;
esac

# ── Resolve version ─────────────────────────────────────────────────

if [ -n "${FIMOD_VERSION:-}" ]; then
  VERSION="$FIMOD_VERSION"
  DOWNLOAD_TAG="$VERSION"
else
  echo "Fetching latest version..."
  case "$SOURCE" in
    gitlab)
      VERSION=$(curl -fsSL "${GL_PKG_BASE}/latest/VERSION") || {
        echo "Error: could not fetch latest version from GitLab" >&2
        exit 1
      }
      DOWNLOAD_TAG="$VERSION"
      ;;
    *)
      # Primary: GitHub's stable-release redirect
      # Try 1: GitHub's stable-release redirect (works for non-pre-releases)
      VERSION=$(curl -fsSL "${BASE_URL}/latest/download/VERSION" 2>/dev/null) || true
      DOWNLOAD_TAG="$VERSION"
      if [ -z "$VERSION" ]; then
        # Try 2: direct "latest" tag (works when the release tag is literally "latest")
        VERSION=$(curl -fsSL "${BASE_URL}/download/latest/VERSION" 2>/dev/null) || true
        DOWNLOAD_TAG="latest"
      fi
      if [ -z "$VERSION" ]; then
        echo "(trying GitHub API...)" >&2
        # Try 3: API — may be rate-limited for anonymous requests (60 req/h)
        DOWNLOAD_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases" 2>/dev/null \
          | grep -m1 '"tag_name"' \
          | sed 's/.*"tag_name": *"\(.*\)".*/\1/') || true
        if [ -n "$DOWNLOAD_TAG" ]; then
          VERSION=$(curl -fsSL "${BASE_URL}/download/${DOWNLOAD_TAG}/VERSION" 2>/dev/null) || true
        fi
      fi
      if [ -z "$VERSION" ]; then
        echo "Error: could not fetch latest version from GitHub" >&2
        exit 1
      fi
      ;;
  esac
fi

echo "Installing fimod ${VERSION} (${VARIANT}) for ${OS}/${ARCH}..."

# ── Build asset name ─────────────────────────────────────────────────

if [ "$VARIANT" = "full" ]; then
  PREFIX="fimod-full"
else
  PREFIX="fimod"
fi

ASSET="${PREFIX}-${VERSION}-${TARGET}.${EXT}"

case "$SOURCE" in
  gitlab)
    URL="${GL_PKG_BASE}/${VERSION}/${ASSET}"
    ;;
  *)
    URL="${BASE_URL}/download/${DOWNLOAD_TAG}/${ASSET}"
    ;;
esac

# ── Choose install directory ─────────────────────────────────────────

if [ -n "${FIMOD_INSTALL:-}" ]; then
  INSTALL_DIR="$FIMOD_INSTALL"
elif [ -w /usr/local/bin ]; then
  INSTALL_DIR="/usr/local/bin"
else
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "$INSTALL_DIR"
fi

# ── Download and install ─────────────────────────────────────────────

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${URL}..."
curl -fsSL -o "${TMPDIR}/${ASSET}" "$URL" || {
  echo "Error: download failed — check that version ${VERSION} exists" >&2
  echo "Available releases: ${BASE_URL}" >&2
  exit 1
}

case "$EXT" in
  tar.gz)
    tar xzf "${TMPDIR}/${ASSET}" -C "$TMPDIR"
    ;;
  zip)
    unzip -q "${TMPDIR}/${ASSET}" -d "$TMPDIR"
    ;;
esac

BIN_NAME="fimod"
[ "$OS" = "windows" ] && BIN_NAME="fimod.exe"

chmod +x "${TMPDIR}/${BIN_NAME}"
mv "${TMPDIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

# ── Verify ───────────────────────────────────────────────────────────

if command -v fimod >/dev/null 2>&1; then
  INSTALLED=$("${INSTALL_DIR}/${BIN_NAME}" --version 2>/dev/null || echo "unknown")
  echo ""
  echo "✅ fimod installed to ${INSTALL_DIR}/${BIN_NAME}"
  echo "   ${INSTALLED}"
else
  echo ""
  echo "✅ fimod installed to ${INSTALL_DIR}/${BIN_NAME}"
  if echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    : # already in PATH
  else
    echo ""
    echo "⚠️  ${INSTALL_DIR} is not in your PATH. Add it:"
    echo "   export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
fi

echo ""
echo "───────────────────────────────────────────────"
if [ -t 0 ] || [ -e /dev/tty ]; then
  echo "  Run 'fimod registry setup' to configure the official mold registry? [Y/n]"
  printf "  > "
  read -r REPLY </dev/tty
  case "$REPLY" in
    [nN]*)
      echo ""
      echo "  Skipped. Run 'fimod registry setup' at any time."
      ;;
    *)
      echo ""
      echo "  Setting up registry..."
      "${INSTALL_DIR}/${BIN_NAME}" registry setup
      ;;
  esac
else
  echo "  Run 'fimod registry setup' to configure the official mold registry."
fi
echo "───────────────────────────────────────────────"
