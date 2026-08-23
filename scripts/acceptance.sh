#!/usr/bin/env bash
set -Eeuo pipefail

trap 'status=$?; echo "acceptance: failed at line ${LINENO}: ${BASH_COMMAND}" >&2; exit "$status"' ERR

BIN="${1:-target/release/media}"

if [[ ! -f "$BIN" && -f "${BIN}.exe" ]]; then
  BIN="${BIN}.exe"
fi
if [[ ! -f "$BIN" ]]; then
  echo "acceptance: binary was not found: $BIN" >&2
  exit 2
fi
if [[ "$BIN" != *.exe && ! -x "$BIN" ]]; then
  echo "acceptance: binary is not executable: $BIN" >&2
  exit 2
fi
command -v ffmpeg >/dev/null || { echo "acceptance: ffmpeg is required" >&2; exit 2; }
command -v ffprobe >/dev/null || { echo "acceptance: ffprobe is required" >&2; exit 2; }
if command -v python3 >/dev/null; then
  PYTHON_BIN="python3"
elif command -v python >/dev/null; then
  PYTHON_BIN="python"
else
  echo "acceptance: Python 3 is required" >&2
  exit 2
fi

TMP_ROOT="${TMPDIR:-/tmp}"
WORK_DIR="$(mktemp -d "$TMP_ROOT/mediaforge-acceptance.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

SRC="$WORK_DIR/source.mp4"
mkdir -p "$WORK_DIR/batch-input" "$WORK_DIR/batch-output"

ffmpeg -hide_banner -loglevel error \
  -f lavfi -i "testsrc=size=320x240:rate=24" \
  -f lavfi -i "sine=frequency=1000:sample_rate=48000" \
  -t 3 -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest "$SRC"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Keep the public acceptance entry point stable while grouping scenarios by concern.
source "$SCRIPT_DIR/acceptance/common.sh"

SOURCE_SHA256="$(file_sha256 "$SRC")"

source "$SCRIPT_DIR/acceptance/media.sh"
source "$SCRIPT_DIR/acceptance/agent.sh"
source "$SCRIPT_DIR/acceptance/safety.sh"

echo "MediaForge acceptance: PASS"
