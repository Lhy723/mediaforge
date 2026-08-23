#!/bin/sh
set -eu

REPO="${MEDIAFORGE_REPO:-Lhy723/mediaforge}"
VERSION="${MEDIAFORGE_VERSION:-latest}"
INSTALL_DIR="${MEDIAFORGE_INSTALL_DIR:-${HOME:-.}/.local/bin}"
OS="$(uname -s)"
ARCH="$(uname -m)"

if [ -n "${MEDIAFORGE_TARGET:-}" ]; then
    TARGET="$MEDIAFORGE_TARGET"
else
    case "$OS:$ARCH" in
        Darwin:arm64|Darwin:aarch64) TARGET="aarch64-apple-darwin" ;;
        Darwin:x86_64|Darwin:amd64) TARGET="x86_64-apple-darwin" ;;
        Linux:x86_64|Linux:amd64) TARGET="x86_64-unknown-linux-gnu" ;;
        *)
            echo "MediaForge: unsupported platform ($OS/$ARCH)." >&2
            echo "Set MEDIAFORGE_TARGET to download a compatible release asset." >&2
            exit 1
            ;;
    esac
fi

if [ "$VERSION" = "latest" ]; then
    ASSET="mediaforge-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"
else
    ASSET="mediaforge-${VERSION}-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
fi

if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
    echo "MediaForge: curl or wget is required to download the release." >&2
    exit 1
fi
if ! command -v tar >/dev/null 2>&1; then
    echo "MediaForge: tar is required to unpack the release." >&2
    exit 1
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mediaforge-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM
ARCHIVE="$TMP_DIR/$ASSET"

if command -v curl >/dev/null 2>&1; then
    curl --fail --location --retry 3 --silent --show-error --output "$ARCHIVE" "$URL"
else
    wget --quiet --output-document="$ARCHIVE" "$URL"
fi

tar -xzf "$ARCHIVE" -C "$TMP_DIR"
if [ ! -f "$TMP_DIR/media" ]; then
    echo "MediaForge: the downloaded archive did not contain media." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
if command -v install >/dev/null 2>&1; then
    install -m 0755 "$TMP_DIR/media" "$INSTALL_DIR/media"
else
    cp "$TMP_DIR/media" "$INSTALL_DIR/media"
    chmod 0755 "$INSTALL_DIR/media"
fi

echo "MediaForge installed to $INSTALL_DIR/media"
case ":${PATH:-}:" in
    *:"$INSTALL_DIR":*) ;;
    *)
        echo "Add it to PATH for the current shell:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

if ! command -v ffmpeg >/dev/null 2>&1 || ! command -v ffprobe >/dev/null 2>&1; then
    echo "FFmpeg and FFprobe were not found on PATH. Install them before processing media." >&2
    case "$OS" in
        Darwin) echo "  brew install ffmpeg" >&2 ;;
        Linux) echo "  sudo apt install ffmpeg" >&2 ;;
    esac
fi

echo "Try: $INSTALL_DIR/media capabilities --json"
