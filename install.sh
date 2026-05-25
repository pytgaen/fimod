#!/bin/sh
# fimod installer — https://github.com/pytgaen/fimod
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pytgaen/fimod/main/install.sh | sh
#
# Options (environment variables):
#   FIMOD_VARIANT   standard (default), slim (without HTTP), or fast (speed optimized)
#   FIMOD_SET_DEFAULT yes=also install slim/fast as the default `fimod` command, no=skip, unset=interactive prompt
#   FIMOD_INSTALL   install directory (default: /usr/local/bin, falls back to ~/.local/bin)
#   FIMOD_VERSION   specific version to install (default: latest)
#   FIMOD_SOURCE    github (default) or gitlab
#   FIMOD_SKIP_DOWNLOAD  set to 1 to skip download (binary must already be installed)
#   FIMOD_SETUP_REGISTRY yes=setup registries, no=skip, unset=prompt if needed
#   FIMOD_SETUP_SANDBOX  yes=setup sandbox, no=skip, unset=prompt if needed
#   FIMOD_SETUP_ALL      yes|no default for both when granulars are unset

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

# ── Build asset name ─────────────────────────────────────────────────

case "$VARIANT" in
  standard)
    PREFIX="fimod"
    BIN_BASENAME="fimod"
    ;;
  slim)
    PREFIX="fimod-slim"
    BIN_BASENAME="fimod-slim"
    ;;
  fast)
    PREFIX="fimod-fast"
    BIN_BASENAME="fimod-fast"
    ;;
  *)
    echo "Error: unsupported FIMOD_VARIANT=${VARIANT}" >&2
    echo "Supported variants: standard, slim, fast" >&2
    exit 1
    ;;
esac

echo "Installing fimod ${VERSION} (${VARIANT}) for ${OS}/${ARCH}..."

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

BIN_NAME="$BIN_BASENAME"
CANONICAL_BIN="fimod"
if [ "$OS" = "windows" ]; then
  BIN_NAME="${BIN_BASENAME}.exe"
  CANONICAL_BIN="fimod.exe"
fi
TARGET_BIN="${INSTALL_DIR}/${BIN_NAME}"
CANONICAL_TARGET="${INSTALL_DIR}/${CANONICAL_BIN}"
DEFAULT_INSTALLED=0

if [ "${FIMOD_SKIP_DOWNLOAD:-}" = "1" ]; then
  echo "Skipping download (FIMOD_SKIP_DOWNLOAD=1)"
  if [ ! -x "$TARGET_BIN" ]; then
    echo "Error: ${TARGET_BIN} not found — cannot skip download" >&2
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

  EXTRACTED_BIN="${TMPDIR}/${BIN_NAME}"
  if [ ! -f "$EXTRACTED_BIN" ] && [ -f "${TMPDIR}/${CANONICAL_BIN}" ]; then
    # Backward compatibility for older slim archives that contained `fimod`.
    EXTRACTED_BIN="${TMPDIR}/${CANONICAL_BIN}"
  fi
  if [ ! -f "$EXTRACTED_BIN" ]; then
    echo "Error: archive did not contain ${BIN_NAME}" >&2
    exit 1
  fi

  chmod +x "$EXTRACTED_BIN"
  mv "$EXTRACTED_BIN" "$TARGET_BIN"
fi

# ── Optional default command copy for slim/fast ──────────────────────

set_default_pref() {
  case "${FIMOD_SET_DEFAULT:-}" in
    yes|no) echo "${FIMOD_SET_DEFAULT}"; return ;;
  esac
  echo "ask"
}

copy_as_default() {
  cp "$TARGET_BIN" "$CANONICAL_TARGET"
  chmod +x "$CANONICAL_TARGET"
  DEFAULT_INSTALLED=1
}

if [ "$VARIANT" != "standard" ]; then
  DEFAULT_PREF=$(set_default_pref)
  case "$DEFAULT_PREF" in
    yes)
      copy_as_default
      ;;
    no)
      ;;
    ask)
      if [ -t 0 ] || (: </dev/tty) 2>/dev/null; then
        echo ""
        echo "Install the ${VARIANT} variant as the default 'fimod' command too? [y/N]"
        printf "  > "
        read -r REPLY </dev/tty
        case "$REPLY" in
          [yY]*)
            copy_as_default
            ;;
        esac
      fi
      ;;
  esac
fi

# ── Verify ───────────────────────────────────────────────────────────

INSTALLED=$("$TARGET_BIN" --version 2>/dev/null || echo "unknown")
echo ""
echo "✅ ${BIN_BASENAME} installed to ${TARGET_BIN}"
echo "   ${INSTALLED}"
if [ "$DEFAULT_INSTALLED" -eq 1 ]; then
  DEFAULT_VERSION=$("$CANONICAL_TARGET" --version 2>/dev/null || echo "unknown")
  echo "✅ fimod installed to ${CANONICAL_TARGET}"
  echo "   ${DEFAULT_VERSION}"
fi

if echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
  : # already in PATH
else
  echo ""
  echo "⚠️  ${INSTALL_DIR} is not in your PATH. Add it:"
  echo "   export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo ""

# ── Post-install setup (registry + sandbox) ─────────────────────────

echo "───────────────────────────────────────────────"
echo "Post-install setup"
if "$TARGET_BIN" setup all defaults --if-needed; then
  :
else
  echo "Warning: post-install setup did not complete." >&2
  echo "Run '${BIN_BASENAME} setup all defaults --if-needed' later to configure registries and sandbox." >&2
fi
echo "───────────────────────────────────────────────"
