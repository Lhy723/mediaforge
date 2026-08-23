# Output safety, configuration, batching, and verification scenarios.

touch "$WORK_DIR/existing.mp4"
"$BIN" convert "$SRC" --to mp4 --output "$WORK_DIR/existing.mp4" --dry-run --json > "$WORK_DIR/safety.json"
assert_json "$WORK_DIR/safety.json" "v['status'] == 'planned' and v['output'].endswith('existing_1.mp4')"
"$BIN" convert "$SRC" --to mp4 --output "$WORK_DIR/existing.mp4" --json > "$WORK_DIR/safety-execute.json"
assert_json "$WORK_DIR/safety-execute.json" "v['status'] == 'success' and v['output'].endswith('existing_1.mp4')"
[[ ! -s "$WORK_DIR/existing.mp4" ]] || { echo "existing output was overwritten" >&2; exit 1; }

printf 'replace-me\n' > "$WORK_DIR/explicit-overwrite.mp4"
"$BIN" convert "$SRC" --to mp4 --output "$WORK_DIR/explicit-overwrite.mp4" --overwrite --json \
  > "$WORK_DIR/explicit-overwrite.json"
assert_json "$WORK_DIR/explicit-overwrite.json" "v['status'] == 'success' and v['output'].endswith('explicit-overwrite.mp4')"
[[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=codec_name -of csv=p=0 "$WORK_DIR/explicit-overwrite.mp4")" == "h264" ]] || {
  echo "explicit overwrite did not replace the target with valid media" >&2
  exit 1
}

if "$BIN" convert "$SRC" --to mp4 --output "$SRC" --dry-run --json > "$WORK_DIR/output-conflict.json"; then
  echo "convert unexpectedly accepted identical input and output paths" >&2
  exit 1
fi
assert_json "$WORK_DIR/output-conflict.json" "v['code'] == 'OUTPUT_CONFLICT'"

if "$BIN" --json convert "$SRC" --unknown-option > "$WORK_DIR/invalid-cli.json"; then
  echo "invalid CLI arguments unexpectedly succeeded" >&2
  exit 1
fi
assert_json "$WORK_DIR/invalid-cli.json" "v['status'] == 'error' and v['code'] == 'INVALID_ARGUMENT' and isinstance(v['details'], dict) and len(v['suggestions']) > 0"

MISSING_DIR="$WORK_DIR/not-created"
MISSING_OUTPUT="$(native_path "$MISSING_DIR/output.mp4")"
"$BIN" convert "$SRC" --to mp4 --output "$MISSING_OUTPUT" --dry-run --json > "$WORK_DIR/dry-run.json"
[[ ! -d "$MISSING_DIR" ]] || { echo "dry-run created an output directory" >&2; exit 1; }

touch "$WORK_DIR/not-a-directory"
UNWRITABLE_OUTPUT="$(native_path "$WORK_DIR/not-a-directory/output.mp4")"
if "$BIN" convert "$SRC" --to mp4 --output "$UNWRITABLE_OUTPUT" --json > "$WORK_DIR/unwritable.json"; then
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
CONFIG_PATH="$(native_path "$WORK_DIR/config.toml")"
MEDIAFORGE_CONFIG="$CONFIG_PATH" "$BIN" compress "$SRC" --dry-run --json > "$WORK_DIR/configured.json"
assert_json "$WORK_DIR/configured.json" "v['quality'] == 'tiny' and v['hardware']['requested'] == 'cpu'"

cp "$SRC" "$WORK_DIR/batch-input/one.mp4"
cp "$SRC" "$WORK_DIR/batch-input/two.mp4"
printf 'not media\n' > "$WORK_DIR/batch-input/broken.mp4"
BATCH_GLOB="$(native_path "$WORK_DIR/batch-input")/*.mp4"
"$BIN" batch "$BATCH_GLOB" --convert mp4 --output-dir "$WORK_DIR/batch-output" --json > "$WORK_DIR/batch.json"
assert_json "$WORK_DIR/batch.json" "v['status'] == 'partial_success' and v['total'] == 3 and v['success'] == 2 and v['failed'] == 1"

mkdir -p "$WORK_DIR/batch-recursive/nested" "$WORK_DIR/batch-recursive-output"
cp "$SRC" "$WORK_DIR/batch-recursive/root.mp4"
cp "$SRC" "$WORK_DIR/batch-recursive/nested/child.mp4"
"$BIN" batch "$WORK_DIR/batch-recursive" --recursive --convert mkv \
  --output-dir "$WORK_DIR/batch-recursive-output" --json > "$WORK_DIR/batch-recursive.json"
assert_json "$WORK_DIR/batch-recursive.json" "v['status'] == 'success' and v['total'] == 2 and v['success'] == 2"

ffmpeg -hide_banner -loglevel error -i "$SRC" -map 0:v:0 -c copy "$WORK_DIR/no-audio.mp4"
if "$BIN" verify "$SRC" "$WORK_DIR/no-audio.mp4" --json > "$WORK_DIR/verify-failed.json"; then
  echo "verify unexpectedly accepted an output with missing audio" >&2
  exit 1
fi
assert_json "$WORK_DIR/verify-failed.json" "v['code'] == 'VERIFY_FAILED' and v['details']['checks']['audio_match'] is False"

if "$BIN" verify "$SRC" "$WORK_DIR/clip-precise.mp4" --json > "$WORK_DIR/verify-duration.json"; then
  echo "verify unexpectedly accepted a severe duration mismatch" >&2
  exit 1
fi
assert_json "$WORK_DIR/verify-duration.json" "v['code'] == 'VERIFY_FAILED' and v['details']['checks']['duration_match'] is False"

ffmpeg -hide_banner -loglevel error -i "$SRC" -map 0:a:0 -c copy "$WORK_DIR/no-video.m4a"
if "$BIN" verify "$SRC" "$WORK_DIR/no-video.m4a" --json > "$WORK_DIR/verify-video.json"; then
  echo "verify unexpectedly accepted an output with missing video" >&2
  exit 1
fi
assert_json "$WORK_DIR/verify-video.json" "v['code'] == 'VERIFY_FAILED' and v['details']['checks']['video_present'] is False"

printf 'not media\n' > "$WORK_DIR/corrupt.mp4"
if "$BIN" verify "$SRC" "$WORK_DIR/corrupt.mp4" --json > "$WORK_DIR/verify-corrupt.json"; then
  echo "verify unexpectedly accepted a corrupt output" >&2
  exit 1
fi
assert_json "$WORK_DIR/verify-corrupt.json" "v['code'] == 'VERIFY_FAILED'"

[[ "$(file_sha256 "$SRC")" == "$SOURCE_SHA256" ]] || {
  echo "source media changed during acceptance" >&2
  exit 1
}
