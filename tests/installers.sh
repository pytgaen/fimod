#!/bin/sh

set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
INSTALLER="${ROOT_DIR}/install.sh"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT

VERSION="v0.0.0-test"
ASSET="fimod-${VERSION}-x86_64-unknown-linux-musl.tar.gz"
FIXTURE_DIR="${TEST_ROOT}/fixture"
MOCK_BIN_DIR="${TEST_ROOT}/mock-bin"
mkdir -p "${FIXTURE_DIR}/package" "$MOCK_BIN_DIR"

cat >"${FIXTURE_DIR}/package/fimod" <<'EOF'
#!/bin/sh
case "${1:-}" in
  --version) echo "fimod test standard" ;;
  setup) exit 0 ;;
  *) exit 0 ;;
esac
EOF
chmod +x "${FIXTURE_DIR}/package/fimod"
tar czf "${FIXTURE_DIR}/${ASSET}" -C "${FIXTURE_DIR}/package" fimod

if command -v sha256sum >/dev/null 2>&1; then
  ASSET_HASH=$(sha256sum "${FIXTURE_DIR}/${ASSET}" | awk '{print $1}')
else
  ASSET_HASH=$(shasum -a 256 "${FIXTURE_DIR}/${ASSET}" | awk '{print $1}')
fi

cat >"${MOCK_BIN_DIR}/curl" <<'EOF'
#!/bin/sh
set -eu

OUTPUT=""
URL=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      OUTPUT=$2
      shift 2
      ;;
    -*)
      shift
      ;;
    *)
      URL=$1
      shift
      ;;
  esac
done

if [ "${TEST_CHECKSUM_MODE}" = "forbid" ]; then
  echo "curl must not be called" >&2
  exit 99
fi

case "$URL" in
  *"/${TEST_ASSET_NAME}")
    cp "${TEST_ASSET_FILE}" "$OUTPUT"
    ;;
  *-sha256sums.txt)
    case "$TEST_CHECKSUM_MODE" in
      valid)
        printf '%s  %s\n' "$TEST_ASSET_HASH" "$TEST_ASSET_NAME" >"$OUTPUT"
        ;;
      missing-sums)
        exit 22
        ;;
      asset-absent)
        printf '%s  %s.sig\n' "$TEST_ASSET_HASH" "$TEST_ASSET_NAME" >"$OUTPUT"
        ;;
      mismatch)
        printf '%064d  %s\n' 0 "$TEST_ASSET_NAME" >"$OUTPUT"
        ;;
      *)
        echo "unexpected checksum mode: ${TEST_CHECKSUM_MODE}" >&2
        exit 98
        ;;
    esac
    ;;
  *)
    echo "unexpected URL: ${URL}" >&2
    exit 97
    ;;
esac
EOF
chmod +x "${MOCK_BIN_DIR}/curl"

fail() {
  echo "installer test failed: $*" >&2
  exit 1
}

run_case() {
  CASE_NAME=$1
  CHECKSUM_MODE=$2
  SOURCE=$3
  EXPECTED_STATUS=$4
  EXPECTED_OUTPUT=$5
  CASE_DIR="${TEST_ROOT}/${CASE_NAME}"
  INSTALL_DIR="${CASE_DIR}/bin"
  OUTPUT_FILE="${CASE_DIR}/output"
  mkdir -p "$INSTALL_DIR"

  STATUS=0
  env \
    PATH="${MOCK_BIN_DIR}:$PATH" \
    TEST_ASSET_FILE="${FIXTURE_DIR}/${ASSET}" \
    TEST_ASSET_HASH="$ASSET_HASH" \
    TEST_ASSET_NAME="$ASSET" \
    TEST_CHECKSUM_MODE="$CHECKSUM_MODE" \
    FIMOD_INSTALL="$INSTALL_DIR" \
    FIMOD_VERSION="$VERSION" \
    FIMOD_SOURCE="$SOURCE" \
    FIMOD_SETUP_ALL=no \
    sh "$INSTALLER" >"$OUTPUT_FILE" 2>&1 || STATUS=$?

  if [ "$EXPECTED_STATUS" = "success" ]; then
    [ "$STATUS" -eq 0 ] || fail "${CASE_NAME} returned ${STATUS}"
    [ -x "${INSTALL_DIR}/fimod" ] || fail "${CASE_NAME} did not install fimod"
  else
    [ "$STATUS" -ne 0 ] || fail "${CASE_NAME} unexpectedly succeeded"
    [ ! -e "${INSTALL_DIR}/fimod" ] || fail "${CASE_NAME} installed an unverified binary"
  fi

  grep -Fq "$EXPECTED_OUTPUT" "$OUTPUT_FILE" || {
    sed -n '1,160p' "$OUTPUT_FILE" >&2
    fail "${CASE_NAME} output did not contain: ${EXPECTED_OUTPUT}"
  }
}

run_case valid valid github success "SHA256 verified"
run_case gitlab-missing-sums missing-sums gitlab failure "could not download required checksums file"
run_case exact-asset-required asset-absent github failure "not found exactly once in checksums file"
run_case mismatch mismatch github failure "SHA256 mismatch"

UNPINNED_DIR="${TEST_ROOT}/gitlab-unpinned/bin"
UNPINNED_OUTPUT="${TEST_ROOT}/gitlab-unpinned/output"
mkdir -p "$UNPINNED_DIR"
UNPINNED_STATUS=0
env \
  PATH="${MOCK_BIN_DIR}:$PATH" \
  TEST_CHECKSUM_MODE=forbid \
  FIMOD_INSTALL="$UNPINNED_DIR" \
  FIMOD_VERSION= \
  FIMOD_SOURCE=gitlab \
  FIMOD_SETUP_ALL=no \
  sh "$INSTALLER" >"$UNPINNED_OUTPUT" 2>&1 || UNPINNED_STATUS=$?
[ "$UNPINNED_STATUS" -ne 0 ] || fail "unpinned GitLab install unexpectedly succeeded"
grep -Fq "requires an explicit FIMOD_VERSION" "$UNPINNED_OUTPUT" || \
  fail "unpinned GitLab install did not explain the pin requirement"

SKIP_DIR="${TEST_ROOT}/skip/bin"
SKIP_OUTPUT="${TEST_ROOT}/skip/output"
mkdir -p "$SKIP_DIR"
cp "${FIXTURE_DIR}/package/fimod" "${SKIP_DIR}/fimod"
env \
  PATH="${MOCK_BIN_DIR}:$PATH" \
  TEST_CHECKSUM_MODE=forbid \
  FIMOD_INSTALL="$SKIP_DIR" \
  FIMOD_VERSION= \
  FIMOD_SKIP_DOWNLOAD=1 \
  FIMOD_SETUP_ALL=no \
  sh "$INSTALLER" >"$SKIP_OUTPUT" 2>&1 || fail "FIMOD_SKIP_DOWNLOAD failed"
grep -Fq "Skipping download (FIMOD_SKIP_DOWNLOAD=1)" "$SKIP_OUTPUT" || \
  fail "FIMOD_SKIP_DOWNLOAD was not reported"

echo "installer checksum tests passed"
