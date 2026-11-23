#!/bin/sh
# Curated cryptographic one-liner installer for Garde-fou (`garde`).
# Usage:
#   curl -fsSL https://github.com/aptitek/garde-fou/releases/latest/download/install.sh | sh
#   sh scripts/install.sh [--version <TAG>] [--install-dir <DIR>] [--repo <OWNER/REPO>] [--from-dir <DIR>]

set -eu

DEFAULT_REPO="aptitek/garde-fou"
VERSION="latest"
INSTALL_DIR="${GARDE_INSTALL_DIR:-$HOME/.local/bin}"
REPO="${GARDE_REPO:-}"
FROM_DIR=""

usage() {
    cat <<'EOF'
Install the `garde` static analysis binary with mandatory SHA-256 verification.

Options:
  --version <TAG>       Release tag to install (default: latest, e.g. v0.1.0)
  --install-dir <DIR>   Target directory (default: $GARDE_INSTALL_DIR or ~/.local/bin)
  --repo <OWNER/REPO>   GitHub repository slug (default: $GARDE_REPO or aptitek/garde-fou)
  --from-dir <DIR>      Offline installation directory containing archive + SHA256SUMS
  -h, --help            Print this help message
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || { echo "garde-install: --version requires an argument" >&2; exit 1; }
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            [ "$#" -ge 2 ] || { echo "garde-install: --install-dir requires an argument" >&2; exit 1; }
            INSTALL_DIR="$2"
            shift 2
            ;;
        --repo)
            [ "$#" -ge 2 ] || { echo "garde-install: --repo requires an argument" >&2; exit 1; }
            REPO="$2"
            shift 2
            ;;
        --from-dir)
            [ "$#" -ge 2 ] || { echo "garde-install: --from-dir requires an argument" >&2; exit 1; }
            FROM_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "garde-install: unknown option '$1'" >&2
            usage >&2
            exit 1
            ;;
    esac
done

detect_repo() {
    if [ -n "$REPO" ]; then
        printf '%s\n' "$REPO"
        return
    fi
    if command -v git >/dev/null 2>&1; then
        origin_url="$(git remote get-url origin 2>/dev/null || true)"
        case "$origin_url" in
            *github.com[:/]*/garde-fou*|*github.com[:/]*/guarde-fou*)
                slug="$(printf '%s' "$origin_url" | sed -e 's#.*github\.com[:/]##' -e 's#\.git$##')"
                if [ -n "$slug" ]; then
                    printf '%s\n' "$slug"
                    return
                fi
                ;;
        esac
    fi
    printf '%s\n' "$DEFAULT_REPO"
}

detect_target() {
    if [ -n "${GARDE_TARGET:-}" ]; then
        printf '%s\n' "$GARDE_TARGET"
        return
    fi
    os_name="$(uname -s)"
    arch_name="$(uname -m)"
    case "$os_name" in
        Linux)
            case "$arch_name" in
                x86_64|amd64) printf 'x86_64-unknown-linux-musl\n' ;;
                aarch64|arm64) printf 'aarch64-unknown-linux-gnu\n' ;;
                *) echo "garde-install: unsupported Linux architecture '$arch_name'" >&2; exit 1 ;;
            esac
            ;;
        Darwin)
            case "$arch_name" in
                x86_64|amd64) printf 'x86_64-apple-darwin\n' ;;
                arm64|aarch64) printf 'aarch64-apple-darwin\n' ;;
                *) echo "garde-install: unsupported macOS architecture '$arch_name'" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "garde-install: unsupported operating system '$os_name' (use install.ps1 on Windows)" >&2
            exit 1
            ;;
    esac
}

download_file() {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        echo "garde-install: neither 'curl' nor 'wget' is available" >&2
        exit 1
    fi
}

compute_sha256() {
    file_path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file_path" | awk '{print tolower($1)}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file_path" | awk '{print tolower($1)}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$file_path" | awk '{print tolower($NF)}'
    else
        echo "garde-install: no SHA-256 tool found (expected sha256sum, shasum, or openssl)" >&2
        exit 1
    fi
}

RESOLVED_REPO="$(detect_repo)"
TARGET="$(detect_target)"
ARCHIVE="garde-${TARGET}.tar.gz"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'garde-install')"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

if [ -n "$FROM_DIR" ]; then
    [ -f "$FROM_DIR/$ARCHIVE" ] || { echo "garde-install: missing archive '$FROM_DIR/$ARCHIVE'" >&2; exit 1; }
    [ -f "$FROM_DIR/SHA256SUMS" ] || { echo "garde-install: missing '$FROM_DIR/SHA256SUMS'" >&2; exit 1; }
    cp "$FROM_DIR/$ARCHIVE" "$TMP_DIR/$ARCHIVE"
    cp "$FROM_DIR/SHA256SUMS" "$TMP_DIR/SHA256SUMS"
else
    if [ "$VERSION" = "latest" ]; then
        BASE_URL="https://github.com/${RESOLVED_REPO}/releases/latest/download"
    else
        case "$VERSION" in
            v*) TAG="$VERSION" ;;
            *) TAG="v$VERSION" ;;
        esac
        BASE_URL="https://github.com/${RESOLVED_REPO}/releases/download/${TAG}"
    fi
    echo "garde-install: downloading ${ARCHIVE} from ${RESOLVED_REPO} (${VERSION})..."
    download_file "${BASE_URL}/${ARCHIVE}" "$TMP_DIR/$ARCHIVE"
    download_file "${BASE_URL}/SHA256SUMS" "$TMP_DIR/SHA256SUMS"
fi

EXPECTED_HASH="$(
    tr -d '\r' < "$TMP_DIR/SHA256SUMS" | awk -v asset="$ARCHIVE" '
        {
            fname = $2
            sub(/^\*/, "", fname)
            if (fname == asset) {
                print tolower($1)
                exit
            }
        }
    '
)"

if [ -z "$EXPECTED_HASH" ]; then
    echo "garde-install: entry for '${ARCHIVE}' not found in SHA256SUMS; aborting" >&2
    exit 1
fi

ACTUAL_HASH="$(compute_sha256 "$TMP_DIR/$ARCHIVE")"
if [ "$ACTUAL_HASH" != "$EXPECTED_HASH" ]; then
    echo "garde-install: SHA-256 mismatch for ${ARCHIVE}!" >&2
    echo "  expected: ${EXPECTED_HASH}" >&2
    echo "  actual:   ${ACTUAL_HASH}" >&2
    exit 1
fi

tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"
if [ ! -f "$TMP_DIR/garde" ]; then
    echo "garde-install: archive did not contain 'garde' binary at root" >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
cp "$TMP_DIR/garde" "$INSTALL_DIR/garde"
chmod 0755 "$INSTALL_DIR/garde"

INSTALLED_VERSION="$("$INSTALL_DIR/garde" --version)"
echo "garde-install: installed ${INSTALLED_VERSION} to ${INSTALL_DIR}/garde (SHA-256 verified)"

case ":${PATH:-}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo "garde-install: note: '${INSTALL_DIR}' is not in your PATH."
        echo "  Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
