#!/usr/bin/env bash
set -euo pipefail

BIN="${1:-target/release/media}"

if [[ ! -x "$BIN" ]]; then
  echo "acceptance: binary is not executable: $BIN" >&2
  exit 2
fi
command -v ffmpeg >/dev/null || { echo "acceptance: ffmpeg is required" >&2; exit 2; }
command -v ffprobe >/dev/null || { echo "acceptance: ffprobe is required" >&2; exit 2; }
command -v python3 >/dev/null || { echo "acceptance: python3 is required" >&2; exit 2; }

TMP_ROOT="${TMPDIR:-/tmp}"
WORK_DIR="$(mktemp -d "$TMP_ROOT/mediaforge-acceptance.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

SRC="$WORK_DIR/source.mp4"
mkdir -p "$WORK_DIR/batch-input" "$WORK_DIR/batch-output"

ffmpeg -hide_banner -loglevel error \
  -f lavfi -i "testsrc=size=320x240:rate=24" \
  -f lavfi -i "sine=frequency=1000:sample_rate=48000" \
  -t 3 -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest "$SRC"

assert_json() {
  local file="$1"
  local expression="$2"
  python3 - "$file" "$expression" <<'PY'
import json
import sys

path, expression = sys.argv[1:]
with open(path, encoding="utf-8") as handle:
    value = json.load(handle)
if not eval(expression, {"__builtins__": {"len": len, "isinstance": isinstance, "dict": dict}}, {"v": value}):
    raise SystemExit(f"assertion failed for {path}: {expression}\n{value}")
PY
}

"$BIN" inspect "$SRC" --json > "$WORK_DIR/inspect.json"
assert_json "$WORK_DIR/inspect.json" "v['status'] == 'success' and len(v['video']) == 1 and len(v['audio']) == 1"

"$BIN" plan "$SRC" --to mkv --json > "$WORK_DIR/plan-convert.json"
assert_json "$WORK_DIR/plan-convert.json" "v['status'] == 'planned' and v['operation'] == 'convert' and v['strategy'] in ('copy', 'remux')"

"$BIN" plan "$SRC" --target-size 1MB --json > "$WORK_DIR/plan-compress.json"
assert_json "$WORK_DIR/plan-compress.json" "v['status'] == 'planned' and v['operation'] == 'compress' and v['passes'] == 2 and v['pass_strategy'] == 'two_pass'"

ffmpeg -hide_banner -loglevel error -i "$SRC" -c copy "$WORK_DIR/source.mov"
ffmpeg -hide_banner -loglevel error -i "$SRC" -c copy "$WORK_DIR/source.mkv"
ffmpeg -hide_banner -loglevel error -i "$SRC" -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -c:a libopus "$WORK_DIR/source.webm"
for media in "$SRC" "$WORK_DIR/source.mkv" "$WORK_DIR/source.mov" "$WORK_DIR/source.webm"; do
  name="$(basename "$media" | tr '.' '-')"
  "$BIN" inspect "$media" --json > "$WORK_DIR/inspect-$name.json"
  assert_json "$WORK_DIR/inspect-$name.json" "v['status'] == 'success' and len(v['video']) == 1 and len(v['audio']) == 1"
done

"$BIN" convert "$SRC" --to mkv --output "$WORK_DIR/converted.mkv" --json > "$WORK_DIR/convert.json"
assert_json "$WORK_DIR/convert.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" convert "$SRC" --to mkv --video-codec h265 --quality tiny --dry-run --json > "$WORK_DIR/convert-quality.json"
assert_json "$WORK_DIR/convert-quality.json" "v['quality'] == 'tiny' and '-crf' in v['ffmpeg_args'] and v['ffmpeg_args'][v['ffmpeg_args'].index('-crf') + 1] == '34'"

"$BIN" plan "$SRC" --to mkv --quality high --dry-run --json > "$WORK_DIR/plan-convert-quality.json"
assert_json "$WORK_DIR/plan-convert-quality.json" "v['operation'] == 'convert' and v['quality'] == 'high'"

printf '1\n00:00:00,000 --> 00:00:01,000\nHello MediaForge\n' > "$WORK_DIR/caption.srt"
ffmpeg -hide_banner -loglevel error -i "$SRC" -f srt -i "$WORK_DIR/caption.srt" \
  -map 0 -map 1:0 -c:v copy -c:a copy -c:s srt "$WORK_DIR/caption.mkv"
"$BIN" plan "$WORK_DIR/caption.mkv" --to mp4 --json > "$WORK_DIR/plan-subtitle.json"
assert_json "$WORK_DIR/plan-subtitle.json" "v['subtitle']['action'] == 'convert_to_mov_text' and len(v['warnings']) == 1"
"$BIN" convert "$WORK_DIR/caption.mkv" --to mp4 --output "$WORK_DIR/caption.mp4" --json > "$WORK_DIR/caption.json"
assert_json "$WORK_DIR/caption.json" "v['status'] == 'success' and v['verification']['valid'] is True"
[[ "$(ffprobe -v error -select_streams s -show_entries stream=codec_name -of csv=p=0 "$WORK_DIR/caption.mp4")" == "mov_text" ]] || {
  echo "subtitle stream was not converted to mov_text" >&2
  exit 1
}

"$BIN" compress "$SRC" --quality tiny --output "$WORK_DIR/compressed.mp4" --json > "$WORK_DIR/compress.json"
assert_json "$WORK_DIR/compress.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" compress "$SRC" --target-size 100KB --output "$WORK_DIR/compressed-target.mp4" --json > "$WORK_DIR/compress-target.json"
assert_json "$WORK_DIR/compress-target.json" "v['status'] == 'success' and v['passes'] == 2 and v['pass_strategy'] == 'two_pass' and v['verification']['valid'] is True"

"$BIN" resize "$SRC" --resolution 120p --output "$WORK_DIR/resized.mp4" --json > "$WORK_DIR/resize.json"
assert_json "$WORK_DIR/resize.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" clip "$SRC" --start 0 --duration 2 --output "$WORK_DIR/clip-copy.mp4" --json > "$WORK_DIR/clip-copy.json"
assert_json "$WORK_DIR/clip-copy.json" "v['strategy'] == 'stream_copy' and v['verification']['valid'] is True"

"$BIN" clip "$SRC" --start 1 --duration 1 --output "$WORK_DIR/clip-precise.mp4" --json > "$WORK_DIR/clip-precise.json"
assert_json "$WORK_DIR/clip-precise.json" "v['strategy'] == 'precise_transcode' and v['verification']['valid'] is True"

"$BIN" clip "$SRC" --start 1 --end 2 --output "$WORK_DIR/clip-end.mp4" --json > "$WORK_DIR/clip-end.json"
assert_json "$WORK_DIR/clip-end.json" "v['strategy'] == 'precise_transcode' and v['verification']['valid'] is True"

"$BIN" extract-audio "$SRC" --format m4a --output "$WORK_DIR/audio.m4a" --json > "$WORK_DIR/audio-copy.json"
assert_json "$WORK_DIR/audio-copy.json" "v['strategy'] == 'copy' and v['verification']['valid'] is True"

"$BIN" extract-audio "$SRC" --format flac --output "$WORK_DIR/audio.flac" --json > "$WORK_DIR/audio-transcode.json"
assert_json "$WORK_DIR/audio-transcode.json" "v['strategy'] == 'transcode' and v['verification']['valid'] is True"

"$BIN" thumbnail "$SRC" --at 50% --output "$WORK_DIR/thumbnail.jpg" --json > "$WORK_DIR/thumbnail.json"
assert_json "$WORK_DIR/thumbnail.json" "v['status'] == 'success' and v['verification']['valid'] is True"

printf '%s\n' '{"operation":"plan","target_operation":"resize","input":"'"$SRC"'","resolution":"120p"}' \
  | "$BIN" tool > "$WORK_DIR/tool.json"
assert_json "$WORK_DIR/tool.json" "v['status'] == 'planned' and v['operation'] == 'resize'"

"$BIN" --progress clip "$SRC" --start 0 --duration 1 --output "$WORK_DIR/progress.mp4" --json \
  > "$WORK_DIR/progress.json" 2> "$WORK_DIR/progress.ndjson"
assert_json "$WORK_DIR/progress.json" "v['status'] == 'success'"
python3 - "$WORK_DIR/progress.ndjson" <<'PY'
import json
import sys

events = []
with open(sys.argv[1], encoding="utf-8") as handle:
    for line in handle:
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass
if not any(event.get("event") == "start" for event in events):
    raise SystemExit("progress stream did not contain a start event")
if not any(event.get("event") == "complete" for event in events):
    raise SystemExit("progress stream did not contain a complete event")
PY

"$BIN" --progress clip "$SRC" --start 0 --duration 1 --output "$WORK_DIR/progress-human.mp4" \
  > "$WORK_DIR/progress-human.out" 2> "$WORK_DIR/progress-human.err"
grep -q "Converting media" "$WORK_DIR/progress-human.err"
grep -q "Complete" "$WORK_DIR/progress-human.err"

"$BIN" capabilities --json > "$WORK_DIR/capabilities.json"
assert_json "$WORK_DIR/capabilities.json" "isinstance(v['hardware_acceleration'], dict) and 'encoders' in v"

touch "$WORK_DIR/existing.mp4"
"$BIN" convert "$SRC" --to mp4 --output "$WORK_DIR/existing.mp4" --dry-run --json > "$WORK_DIR/safety.json"
assert_json "$WORK_DIR/safety.json" "v['status'] == 'planned' and v['output'].endswith('existing_1.mp4')"

MISSING_DIR="$WORK_DIR/not-created"
"$BIN" convert "$SRC" --to mp4 --output "$MISSING_DIR/output.mp4" --dry-run --json > "$WORK_DIR/dry-run.json"
[[ ! -d "$MISSING_DIR" ]] || { echo "dry-run created an output directory" >&2; exit 1; }

touch "$WORK_DIR/not-a-directory"
if "$BIN" convert "$SRC" --to mp4 --output "$WORK_DIR/not-a-directory/output.mp4" --json > "$WORK_DIR/unwritable.json"; then
  echo "convert unexpectedly accepted a file as the output parent" >&2
  exit 1
fi
assert_json "$WORK_DIR/unwritable.json" "v['code'] == 'OUTPUT_UNWRITABLE'"

cat > "$WORK_DIR/config.toml" <<'EOF'
default_quality = "tiny"
hardware = "cpu"
verify_after_execute = false
progress = false

[video]
preferred_codec = "h264"

[audio]
preferred_codec = "aac"
EOF
MEDIAFORGE_CONFIG="$WORK_DIR/config.toml" "$BIN" compress "$SRC" --dry-run --json > "$WORK_DIR/configured.json"
assert_json "$WORK_DIR/configured.json" "v['quality'] == 'tiny' and v['hardware']['requested'] == 'cpu'"

cp "$SRC" "$WORK_DIR/batch-input/one.mp4"
cp "$SRC" "$WORK_DIR/batch-input/two.mp4"
printf 'not media\n' > "$WORK_DIR/batch-input/broken.mp4"
"$BIN" batch "$WORK_DIR/batch-input/*.mp4" --convert mp4 --output-dir "$WORK_DIR/batch-output" --json > "$WORK_DIR/batch.json"
assert_json "$WORK_DIR/batch.json" "v['status'] == 'partial_success' and v['total'] == 3 and v['success'] == 2 and v['failed'] == 1"

ffmpeg -hide_banner -loglevel error -i "$SRC" -map 0:v:0 -c copy "$WORK_DIR/no-audio.mp4"
if "$BIN" verify "$SRC" "$WORK_DIR/no-audio.mp4" --json > "$WORK_DIR/verify-failed.json"; then
  echo "verify unexpectedly accepted an output with missing audio" >&2
  exit 1
fi
assert_json "$WORK_DIR/verify-failed.json" "v['code'] == 'VERIFY_FAILED' and v['details']['checks']['audio_match'] is False"

echo "MediaForge acceptance: PASS"
