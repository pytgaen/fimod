#!/bin/bash
# prerelease-local.sh — Build release binary + run e2e tests in a disposable container
#
# Usage:
#   ./scripts/prerelease-local.sh                  # full: build + test
#   ./scripts/prerelease-local.sh --skip-build      # reuse last release binary
#   ./scripts/prerelease-local.sh --keep            # don't destroy container after test
#   ./scripts/prerelease-local.sh --runtime docker   # force docker (default: auto-detect)
#   ./scripts/prerelease-local.sh --runtime incus    # force incus
#
# Requires: docker or incus, cargo

set -euo pipefail

FIMOD_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_BUILD=false
KEEP_CONTAINER=false
RUNTIME=""
CONTAINER_NAME="fimod-prerelease-test"
IMAGE="ubuntu:24.04"
INCUS_IMAGE="images:ubuntu/24.04"

# ── Parse args ───────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-build)  SKIP_BUILD=true; shift ;;
        --keep)        KEEP_CONTAINER=true; shift ;;
        --runtime)     RUNTIME="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,10s/^# //p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

# ── Colors ───────────────────────────────────────────────────────────

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m'

step()    { echo -e "\n${CYAN}── $1${NC}"; }
ok()      { echo -e "${GREEN}  ✓ $1${NC}"; }
fail()    { echo -e "${RED}  ✗ $1${NC}"; FAILURES=$((FAILURES + 1)); }
info()    { echo -e "${DIM}  $1${NC}"; }

FAILURES=0
TESTS_RUN=0
TESTS_PASSED=0

assert_ok() {
    local desc="$1"; shift
    TESTS_RUN=$((TESTS_RUN + 1))
    if "$@" >/dev/null 2>&1; then
        ok "$desc"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "$desc"
    fi
}

assert_output_contains() {
    local desc="$1"; local pattern="$2"; shift 2
    TESTS_RUN=$((TESTS_RUN + 1))
    local output
    output=$("$@" 2>&1) || true
    if echo "$output" | grep -q "$pattern"; then
        ok "$desc"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "$desc (expected '$pattern', got: $output)"
    fi
}

# ── Detect runtime ───────────────────────────────────────────────────

detect_runtime() {
    if [[ -n "$RUNTIME" ]]; then
        if ! command -v "$RUNTIME" >/dev/null 2>&1; then
            echo "Error: $RUNTIME not found" >&2
            exit 1
        fi
        echo "$RUNTIME"
        return
    fi
    if command -v incus >/dev/null 2>&1; then
        echo "incus"
    elif command -v docker >/dev/null 2>&1; then
        echo "docker"
    else
        echo "Error: neither docker nor incus found" >&2
        exit 1
    fi
}

RUNTIME=$(detect_runtime)
info "Runtime: $RUNTIME"

# ── Container abstraction ────────────────────────────────────────────

container_start() {
    case "$RUNTIME" in
        docker)
            docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
            docker run -d --name "$CONTAINER_NAME" "$IMAGE" sleep infinity >/dev/null
            # wait for container to be ready
            docker exec "$CONTAINER_NAME" true
            ;;
        incus)
            incus rm "$CONTAINER_NAME" --force 2>/dev/null || true
            incus launch "$INCUS_IMAGE" "$CONTAINER_NAME"
            sleep 3  # wait for network
            incus exec "$CONTAINER_NAME" -- apt-get update -qq >/dev/null
            incus exec "$CONTAINER_NAME" -- apt-get install -y -qq curl >/dev/null 2>&1
            ;;
    esac
}

container_exec() {
    case "$RUNTIME" in
        docker) docker exec "$CONTAINER_NAME" "$@" ;;
        incus)  incus exec "$CONTAINER_NAME" -- "$@" ;;
    esac
}

container_exec_bash() {
    case "$RUNTIME" in
        docker) docker exec "$CONTAINER_NAME" bash -c "$1" ;;
        incus)  incus exec "$CONTAINER_NAME" -- bash -c "$1" ;;
    esac
}

container_push() {
    local src="$1" dest="$2"
    case "$RUNTIME" in
        docker) docker cp "$src" "$CONTAINER_NAME:$dest" ;;
        incus)  incus file push "$src" "$CONTAINER_NAME$dest" ;;
    esac
}

container_destroy() {
    if [[ "$KEEP_CONTAINER" == true ]]; then
        echo -e "\n${CYAN}Container '$CONTAINER_NAME' kept for inspection:${NC}"
        case "$RUNTIME" in
            docker) echo "  docker exec -it $CONTAINER_NAME bash"
                    echo "  docker rm -f $CONTAINER_NAME" ;;
            incus)  echo "  incus exec $CONTAINER_NAME -- bash"
                    echo "  incus rm $CONTAINER_NAME --force" ;;
        esac
        return
    fi
    case "$RUNTIME" in
        docker) docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true ;;
        incus)  incus rm "$CONTAINER_NAME" --force 2>/dev/null || true ;;
    esac
}

# ── Cleanup on exit ──────────────────────────────────────────────────

cleanup() {
    if [[ "$KEEP_CONTAINER" == false ]]; then
        container_destroy
    fi
}
trap cleanup EXIT

# ── Phase 1: Build ───────────────────────────────────────────────────

BINARY="$FIMOD_DIR/target/release/fimod"

if [[ "$SKIP_BUILD" == false ]]; then
    step "Building release binary"
    (cd "$FIMOD_DIR" && cargo build --release --quiet)
    ok "Binary built"
else
    if [[ ! -f "$BINARY" ]]; then
        echo "Error: no release binary at $BINARY (run without --skip-build)" >&2
        exit 1
    fi
    step "Reusing existing release binary"
    ok "Binary found"
fi

EXPECTED_VERSION=$("$BINARY" --version 2>/dev/null | awk '{print $2}')
info "Version: $EXPECTED_VERSION"

# ── Phase 2: Start container ────────────────────────────────────────

step "Starting container ($RUNTIME)"
container_start
ok "Container ready"

# ── Phase 3: Push binary + install.sh ────────────────────────────────

step "Pushing binary and install.sh"
container_push "$BINARY" "/usr/local/bin/fimod"
container_exec chmod +x /usr/local/bin/fimod
container_push "$FIMOD_DIR/install.sh" "/tmp/install.sh"

ok "Files pushed"

# ── Phase 4: Tests ───────────────────────────────────────────────────

step "Test: version check"
assert_output_contains \
    "fimod --version reports $EXPECTED_VERSION" \
    "$EXPECTED_VERSION" \
    container_exec fimod --version

# The legacy "official" registry URL that triggers the migration in setup()
LEGACY_OFFICIAL_URL="https://github.com/pytgaen/fimod/tree/main/molds"

# ── Test 1: Fresh install (no existing config) ─────────────────────

step "Test 1: fresh install (no prior config)"
container_exec_bash '
    export HOME=/tmp/test-fresh
    mkdir -p $HOME
    fimod registry setup --yes
'
assert_ok "fimod-powered registry created (P10)" \
    container_exec_bash '
        export HOME=/tmp/test-fresh
        fimod registry list --output-format json | grep -q "fimod-powered"
    '
assert_ok "examples registry created (P99)" \
    container_exec_bash '
        export HOME=/tmp/test-fresh
        fimod registry list --output-format json | grep -q "examples"
    '

# ── Test 2: Migration official → examples ───────────────────────────

step "Test 2: migrate 0.2.0 user (official → examples + add fimod-powered)"
container_exec_bash "
    export HOME=/tmp/test-migration
    mkdir -p \$HOME/.config/fimod
    cat > \$HOME/.config/fimod/sources.toml << 'TOML'
[priority]
official = 50

[sources.official]
type = \"github\"
url = \"$LEGACY_OFFICIAL_URL\"
TOML
    fimod registry setup --yes
"
assert_ok "official registry removed" \
    container_exec_bash '
        export HOME=/tmp/test-migration
        ! fimod registry list --output-format json | grep -q "\"official\""
    '
assert_ok "examples registry present (renamed from official, P99)" \
    container_exec_bash '
        export HOME=/tmp/test-migration
        fimod registry list --output-format json | grep -q "examples"
    '
assert_ok "fimod-powered registry added (P10)" \
    container_exec_bash '
        export HOME=/tmp/test-migration
        fimod registry list --output-format json | grep -q "fimod-powered"
    '

# ── Test 2b: Migration from 0.1.0 (default= field, no priority) ────

step "Test 2b: migrate 0.1.0 user (default= → priority + official → examples)"
container_exec_bash "
    export HOME=/tmp/test-migration-010
    mkdir -p \$HOME/.config/fimod
    cat > \$HOME/.config/fimod/sources.toml << 'TOML'
default = \"official\"

[sources.official]
type = \"github\"
url = \"$LEGACY_OFFICIAL_URL\"
TOML
    fimod registry setup --yes
"
assert_ok "official registry removed" \
    container_exec_bash '
        export HOME=/tmp/test-migration-010
        ! fimod registry list --output-format json | grep -q "\"official\""
    '
assert_ok "examples registry present (renamed)" \
    container_exec_bash '
        export HOME=/tmp/test-migration-010
        fimod registry list --output-format json | grep -q "examples"
    '
assert_ok "fimod-powered registry added" \
    container_exec_bash '
        export HOME=/tmp/test-migration-010
        fimod registry list --output-format json | grep -q "fimod-powered"
    '

# ── Test 3: Migration with custom registries preserved ──────────────

step "Test 3: migrate 0.2.0 user with custom registry preserved"
container_exec_bash "
    export HOME=/tmp/test-migration-custom
    mkdir -p \$HOME/.config/fimod
    cat > \$HOME/.config/fimod/sources.toml << 'TOML'
[priority]
official = 50
mycompany = 20

[sources.official]
type = \"github\"
url = \"$LEGACY_OFFICIAL_URL\"

[sources.mycompany]
type = \"http\"
url = \"https://git.internal/team/fimod-molds\"
TOML
    fimod registry setup --yes
"
assert_ok "custom registry preserved" \
    container_exec_bash '
        export HOME=/tmp/test-migration-custom
        fimod registry list --output-format json | grep -q "mycompany"
    '
assert_ok "official removed, examples present" \
    container_exec_bash '
        export HOME=/tmp/test-migration-custom
        OUT=$(fimod registry list --output-format json)
        echo "$OUT" | grep -q "examples" && ! echo "$OUT" | grep -q "\"official\""
    '

# ── Test 4: Already migrated (idempotent) ───────────────────────────

step "Test 4: already migrated (no-op)"
container_exec_bash '
    export HOME=/tmp/test-already
    mkdir -p $HOME
    fimod registry setup --yes
    fimod registry setup --yes
'
assert_output_contains \
    "second setup is a no-op" \
    "already configured" \
    container_exec_bash '
        export HOME=/tmp/test-already
        fimod registry setup --yes
    '

# ── Test 5: Name collision (fimod-powered already taken) ────────────

step "Test 5: name collision (fimod-powered name already used)"
container_exec_bash '
    export HOME=/tmp/test-collision
    mkdir -p $HOME/.config/fimod
    cat > $HOME/.config/fimod/sources.toml << TOML
[sources.fimod-powered]
type = "http"
url = "https://example.com/my-custom-registry"
TOML
    fimod registry setup --yes
'
assert_ok "original fimod-powered preserved" \
    container_exec_bash '
        export HOME=/tmp/test-collision
        fimod registry list --output-format json | grep -q "example.com"
    '
assert_ok "real fimod-powered added under alternate name" \
    container_exec_bash '
        export HOME=/tmp/test-collision
        fimod registry list --output-format json | grep -q "fimod-fimod-powered"
    '

# ── Test 6: Smoke test (inline expression) ──────────────────────────

step "Test 6: smoke test (inline expression)"
assert_output_contains \
    "inline expression works" \
    '"a":1' \
    container_exec_bash 'echo "[{\"a\":1},{\"a\":2}]" | fimod shape -e "data[0]" --output-format json-compact'

# ── Test 6b: Smoke test (remote mold from examples registry) ──────

step "Test 6b: smoke test (@flatten_nested from examples)"
assert_output_contains \
    "examples mold works" \
    '"a.b":1' \
    container_exec_bash '
        export HOME=/tmp/test-fresh
        echo "{\"a\":{\"b\":1}}" | fimod shape -m @flatten_nested --output-format json-compact
    '

# ── Test 6c: Smoke test (remote mold from fimod-powered) ─────────

step "Test 6c: smoke test (@dockerfile from fimod-powered)"
assert_output_contains \
    "fimod-powered mold works" \
    'FROM python' \
    container_exec_bash '
        export HOME=/tmp/test-fresh
        echo "{\"language\":\"python\",\"package_manager\":\"pip\",\"python_version\":\"3.12\",\"app_port\":8000}" | fimod shape -m @dockerfile --output-format txt
    '

# ── Test 7: install.sh with FIMOD_SKIP_DOWNLOAD=1 ──────────────────

step "Test 7: install.sh with FIMOD_SKIP_DOWNLOAD=1"
container_exec_bash '
    export HOME=/tmp/test-install-sh
    mkdir -p $HOME
    FIMOD_SKIP_DOWNLOAD=1 FIMOD_SETUP_REGISTRY=yes sh /tmp/install.sh
'
assert_ok "install.sh completes without download" \
    container_exec_bash '
        export HOME=/tmp/test-install-sh
        fimod --version
    '

# ── Test 8: install.sh delegates post-install setup to fimod ─────────

step "Test 8: install.sh setup delegation"
container_exec_bash '
    export HOME=/tmp/test-setup-delegation
    mkdir -p $HOME /tmp/fimod-bin-setup
    cp /usr/local/bin/fimod /tmp/fimod-bin-setup/fimod
    chmod +x /tmp/fimod-bin-setup/fimod
    FIMOD_INSTALL=/tmp/fimod-bin-setup FIMOD_SKIP_DOWNLOAD=1 FIMOD_SETUP_ALL=yes sh /tmp/install.sh
'
assert_ok "registry configured by delegated setup" \
    container_exec_bash '
        export HOME=/tmp/test-setup-delegation
        fimod registry list --output-format json | grep -q "examples"
    '
assert_ok "sandbox.toml written by delegated setup" \
    container_exec_bash '
        test -f /tmp/test-setup-delegation/.config/fimod/sandbox.toml
    '

# ── Test 9: install.sh if-needed preserves existing sandbox ──────────

step "Test 9: install.sh setup if-needed preserves sandbox"
container_exec_bash '
    export HOME=/tmp/test-if-needed-existing
    mkdir -p $HOME/.config/fimod /tmp/fimod-bin-existing
    echo "# custom sandbox" > $HOME/.config/fimod/sandbox.toml
    cp /usr/local/bin/fimod /tmp/fimod-bin-existing/fimod
    chmod +x /tmp/fimod-bin-existing/fimod
    FIMOD_INSTALL=/tmp/fimod-bin-existing FIMOD_SKIP_DOWNLOAD=1 FIMOD_SETUP_ALL=yes sh /tmp/install.sh
'
assert_ok "registry configured when sandbox already exists" \
    container_exec_bash '
        export HOME=/tmp/test-if-needed-existing
        fimod registry list --output-format json | grep -q "examples"
    '
assert_ok "sandbox.toml preserved by if-needed" \
    container_exec_bash '
        grep -q "custom sandbox" /tmp/test-if-needed-existing/.config/fimod/sandbox.toml
    '

# ── Test 10: install.sh interactive defaults via pseudo-TTY ─────────

step "Test 10: install.sh interactive setup accepts defaults"
container_exec_bash '
    export HOME=/tmp/test-interactive-accept
    mkdir -p $HOME /tmp/fimod-bin-interactive-accept
    cp /usr/local/bin/fimod /tmp/fimod-bin-interactive-accept/fimod
    chmod +x /tmp/fimod-bin-interactive-accept/fimod
    printf "\n\n" | script -qec "FIMOD_INSTALL=/tmp/fimod-bin-interactive-accept FIMOD_SKIP_DOWNLOAD=1 sh /tmp/install.sh" /dev/null
'
assert_ok "interactive default installs registries" \
    container_exec_bash '
        export HOME=/tmp/test-interactive-accept
        fimod registry list --output-format json | grep -q "examples"
    '
assert_ok "interactive default writes sandbox" \
    container_exec_bash '
        test -f /tmp/test-interactive-accept/.config/fimod/sandbox.toml
    '

# ── Test 11: install.sh interactive explicit no via pseudo-TTY ──────

step "Test 11: install.sh interactive setup can decline"
container_exec_bash '
    export HOME=/tmp/test-interactive-decline
    mkdir -p $HOME /tmp/fimod-bin-interactive-decline
    cp /usr/local/bin/fimod /tmp/fimod-bin-interactive-decline/fimod
    chmod +x /tmp/fimod-bin-interactive-decline/fimod
    printf "n\nn\n" | script -qec "FIMOD_INSTALL=/tmp/fimod-bin-interactive-decline FIMOD_SKIP_DOWNLOAD=1 sh /tmp/install.sh" /dev/null
'
assert_ok "interactive decline leaves registries absent" \
    container_exec_bash '
        test ! -e /tmp/test-interactive-decline/.config/fimod/sources.toml
    '
assert_ok "interactive decline leaves sandbox absent" \
    container_exec_bash '
        test ! -e /tmp/test-interactive-decline/.config/fimod/sandbox.toml
    '

# ── Results ──────────────────────────────────────────────────────────

container_destroy
trap - EXIT

echo ""
if [[ $FAILURES -eq 0 ]]; then
    echo -e "${GREEN}══ All $TESTS_RUN tests passed ══${NC}"
    exit 0
else
    echo -e "${RED}══ $TESTS_PASSED/$TESTS_RUN tests passed, $FAILURES failed ══${NC}"
    exit 1
fi
