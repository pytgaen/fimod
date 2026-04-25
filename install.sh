#!/bin/sh
# fimod installer — https://github.com/pytgaen/fimod
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
#
# Options (environment variables):
#   FIMOD_VARIANT   standard (default, includes HTTP mold loading) or slim (without HTTP)
#   FIMOD_INSTALL   install directory (default: /usr/local/bin, falls back to ~/.local/bin)
#   FIMOD_VERSION   specific version to install (default: latest)
#   FIMOD_SOURCE    github (default) or gitlab
#   FIMOD_SKIP_DOWNLOAD  set to 1 to skip download (binary must already be installed)
#   FIMOD_SETUP_REGISTRY yes=auto-setup registries, no=skip, unset=fall through
#   FIMOD_SETUP_SANDBOX  yes=auto-setup sandbox, no=skip, unset=fall through (fimod >= 0.5.0)
#   FIMOD_SETUP_ALL      yes|no default for both when granulars are unset; unset=interactive prompt

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

if [ "${FIMOD_SKIP_DOWNLOAD:-}" = "1" ]; then
  # Binary already installed — skip version resolution entirely
  VERSION="(skip)"
  DOWNLOAD_TAG=""
elif [ -n "${FIMOD_VERSION:-}" ]; then
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

if [ "$VARIANT" = "slim" ]; then
  PREFIX="fimod-slim"
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

BIN_NAME="fimod"
[ "$OS" = "windows" ] && BIN_NAME="fimod.exe"

if [ "${FIMOD_SKIP_DOWNLOAD:-}" = "1" ]; then
  echo "Skipping download (FIMOD_SKIP_DOWNLOAD=1)"
  if [ ! -x "${INSTALL_DIR}/${BIN_NAME}" ]; then
    echo "Error: ${INSTALL_DIR}/${BIN_NAME} not found — cannot skip download" >&2
    exit 1
  fi
else
  TMPDIR=$(mktemp -d)
  trap 'rm -rf "$TMPDIR"' EXIT

  echo "Downloading ${URL}..."
  curl -fsSL -o "${TMPDIR}/${ASSET}" "$URL" || {
    echo "Error: download failed — check that version ${VERSION} exists" >&2
    echo "Available releases: ${BASE_URL}" >&2
    exit 1
  }

  # ── SHA256 verification ──────────────────────────────────────────────
  SUMS_FILE="fimod-${VERSION}-sha256sums.txt"
  case "$SOURCE" in
    gitlab) SUMS_URL="${GL_PKG_BASE}/${VERSION}/${SUMS_FILE}" ;;
    *)      SUMS_URL="${BASE_URL}/download/${DOWNLOAD_TAG}/${SUMS_FILE}" ;;
  esac

  if curl -fsSL -o "${TMPDIR}/${SUMS_FILE}" "$SUMS_URL" 2>/dev/null; then
    EXPECTED=$(grep "$(basename "${ASSET}")" "${TMPDIR}/${SUMS_FILE}" | awk '{print $1}')
    if [ -n "$EXPECTED" ]; then
      ACTUAL=$(sha256sum "${TMPDIR}/${ASSET}" | awk '{print $1}')
      if [ "$ACTUAL" != "$EXPECTED" ]; then
        echo "Error: SHA256 mismatch!" >&2
        echo "  expected: ${EXPECTED}" >&2
        echo "  got:      ${ACTUAL}" >&2
        exit 1
      fi
      echo "SHA256 verified ✓"
    else
      echo "Warning: asset not found in checksums file, skipping verification" >&2
    fi
  else
    echo "Warning: could not download checksums file, skipping verification" >&2
  fi

  case "$EXT" in
    tar.gz)
      tar xzf "${TMPDIR}/${ASSET}" -C "$TMPDIR"
      ;;
    zip)
      unzip -q "${TMPDIR}/${ASSET}" -d "$TMPDIR"
      ;;
  esac

  chmod +x "${TMPDIR}/${BIN_NAME}"
  mv "${TMPDIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
fi

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

# ── Post-install setup (registry + sandbox) ─────────────────────────
#
# Two independent blocks: registry (community molds) and sandbox (policy file).
# Each resolves its preference in order:
#   1. FIMOD_SETUP_<CAT>=yes|no   (granular, wins over the rest)
#   2. FIMOD_SETUP_ALL=yes|no     (default for both when granular unset)
#   3. interactive TTY prompt
#   4. otherwise skip with a hint
#
# The command path depends on the installed fimod version:
#   >= 0.5.0  → `fimod setup registry defaults` and `fimod setup sandbox defaults`
#   <  0.5.0  → only `fimod registry setup` (sandbox unavailable)

INSTALLED_VERSION=$("${INSTALL_DIR}/${BIN_NAME}" --version 2>/dev/null \
  | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' \
  | head -n 1)
: "${INSTALLED_VERSION:=0.0.0}"
INSTALLED_NUM=$(echo "$INSTALLED_VERSION" \
  | awk -F'[.-]' '{ printf "%d", ($1*10000)+($2*100)+$3 }')
: "${INSTALLED_NUM:=0}"

if [ "$INSTALLED_NUM" -ge 500 ]; then
  REGISTRY_CMD_ARGS="setup registry defaults"
  REGISTRY_HINT="fimod setup registry defaults"
  SANDBOX_AVAILABLE=1
else
  REGISTRY_CMD_ARGS="registry setup"
  REGISTRY_HINT="fimod registry setup"
  SANDBOX_AVAILABLE=0
fi

resolve_pref() {
  case "$1" in
    yes|no) echo "$1"; return ;;
  esac
  case "${FIMOD_SETUP_ALL:-}" in
    yes|no) echo "${FIMOD_SETUP_ALL}"; return ;;
  esac
  echo "ask"
}

REG_PREF=$(resolve_pref "${FIMOD_SETUP_REGISTRY:-}")
SB_PREF=$(resolve_pref "${FIMOD_SETUP_SANDBOX:-}")

echo "───────────────────────────────────────────────"
echo "Registry"
case "$REG_PREF" in
  yes)
    echo "  Installing community registries..."
    # shellcheck disable=SC2086
    "${INSTALL_DIR}/${BIN_NAME}" $REGISTRY_CMD_ARGS --yes
    ;;
  no)
    echo "  Skipped. Run '${REGISTRY_HINT}' at any time."
    ;;
  ask)
    if [ -t 0 ] || [ -e /dev/tty ]; then
      echo "  Install community registries? [Y/n]"
      printf "  > "
      read -r REPLY </dev/tty
      case "$REPLY" in
        [nN]*)
          echo "  Skipped. Run '${REGISTRY_HINT}' at any time."
          ;;
        *)
          # shellcheck disable=SC2086
          "${INSTALL_DIR}/${BIN_NAME}" $REGISTRY_CMD_ARGS --yes
          ;;
      esac
    else
      echo "  Run '${REGISTRY_HINT}' to configure community registries."
    fi
    ;;
esac

echo ""
echo "Sandbox"
if [ "$SANDBOX_AVAILABLE" -eq 0 ]; then
  if [ "$SB_PREF" = "yes" ]; then
    echo "  Requires fimod >= 0.5.0 (installed ${INSTALLED_VERSION}) — skipped."
  else
    echo "  Requires fimod >= 0.5.0 (installed ${INSTALLED_VERSION})."
  fi
else
  case "$SB_PREF" in
    yes)
      echo "  Installing recommended sandbox policy..."
      "${INSTALL_DIR}/${BIN_NAME}" setup sandbox defaults --yes
      ;;
    no)
      echo "  Skipped. Run 'fimod setup sandbox defaults' at any time."
      ;;
    ask)
      if [ -t 0 ] || [ -e /dev/tty ]; then
        echo "  Install recommended sandbox policy? [Y/n]"
        printf "  > "
        read -r REPLY </dev/tty
        case "$REPLY" in
          [nN]*)
            echo "  Skipped. Run 'fimod setup sandbox defaults' at any time."
            ;;
          *)
            "${INSTALL_DIR}/${BIN_NAME}" setup sandbox defaults --yes
            ;;
        esac
      else
        echo "  Run 'fimod setup sandbox defaults' to configure the sandbox policy."
      fi
      ;;
  esac
fi
echo "───────────────────────────────────────────────"
