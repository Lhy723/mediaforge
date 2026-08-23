# Tool API, progress reporting, and capability scenarios.

printf '%s\n' '{"operation":"plan","target_operation":"resize","input":"'"$SRC_JSON"'","resolution":"120p"}' \
  | "$BIN" tool > "$WORK_DIR/tool.json"
assert_json "$WORK_DIR/tool.json" "v['status'] == 'planned' and v['operation'] == 'resize'"

printf '%s\n' '{"operation":"convert_media","input":"'"$SOURCE_MKV_JSON"'","output_format":"mp4","dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-convert.json"
assert_json "$WORK_DIR/tool-convert.json" "v['status'] == 'planned' and v['strategy'] == 'remux'"

printf '%s\n' '{"operation":"image_convert","input":"'"$SRC_JSON"'","output_format":"jpg","width":160,"dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-image.json"
assert_json "$WORK_DIR/tool-image.json" "v['status'] == 'planned' and v['operation'] == 'image'"

printf '%s\n' '{"operation":"image_compress","input":"'"$SRC_JSON"'","output_format":"jpg","image_quality":70,"dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-image-compress.json"
assert_json "$WORK_DIR/tool-image-compress.json" "v['status'] == 'planned' and v['operation'] == 'image' and v['quality'] == 70"

printf '%s\n' '{"operation":"video_to_gif","input":"'"$SRC_JSON"'","duration":"1","fps":8,"width":96,"dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-gif.json"
assert_json "$WORK_DIR/tool-gif.json" "v['status'] == 'planned' and v['operation'] == 'gif' and v['fps'] == 8"

printf '%s\n' '{"operation":"merge","inputs":["'"$SRC_JSON"'","'"$SOURCE_MKV_JSON"'"],"mode":"concat","dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-merge.json"
assert_json "$WORK_DIR/tool-merge.json" "v['status'] == 'planned' and v['operation'] == 'merge' and v['input_count'] == 2"

printf '%s\n' '{"operation":"presets"}' | "$BIN" tool > "$WORK_DIR/tool-presets.json"
assert_json "$WORK_DIR/tool-presets.json" "v['status'] == 'success' and len(v['presets']) >= 5"

printf '%s\n' '{"operation":"ffmpeg","args":["-version"],"dry_run":true}' \
  | "$BIN" tool > "$WORK_DIR/tool-ffmpeg.json"
assert_json "$WORK_DIR/tool-ffmpeg.json" "v['status'] == 'planned' and v['operation'] == 'ffmpeg'"

"$BIN" --progress clip "$SRC" --start 0 --duration 1 --output "$WORK_DIR/progress.mp4" --json \
  > "$WORK_DIR/progress.json" 2> "$WORK_DIR/progress.ndjson"
assert_json "$WORK_DIR/progress.json" "v['status'] == 'success'"
"$PYTHON_BIN" - "$WORK_DIR/progress.ndjson" <<'PY'
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
if not any(
    event.get("event") == "complete"
    and isinstance(event.get("elapsed_seconds"), (int, float))
    for event in events
):
    raise SystemExit("progress stream did not report elapsed time")
PY

"$BIN" --progress clip "$SRC" --start 0 --duration 1 --output "$WORK_DIR/progress-human.mp4" \
  > "$WORK_DIR/progress-human.out" 2> "$WORK_DIR/progress-human.err"
grep -q "Converting media" "$WORK_DIR/progress-human.err"
grep -q "Complete" "$WORK_DIR/progress-human.err"
grep -q "Elapsed" "$WORK_DIR/progress-human.err"

"$BIN" --progress convert "$SRC" --to mkv --output "$WORK_DIR/progress-convert.mkv" --json \
  > "$WORK_DIR/progress-convert.json" 2> "$WORK_DIR/progress-convert.ndjson"
"$PYTHON_BIN" - "$WORK_DIR/progress-convert.ndjson" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    events = [json.loads(line) for line in handle if line.strip().startswith("{")]
complete = [event for event in events if event.get("event") == "complete"]
if not complete or complete[-1].get("value", 0) < 0.9:
    raise SystemExit("duration-derived conversion progress did not reach completion")
PY

"$BIN" capabilities --json > "$WORK_DIR/capabilities.json"
assert_json "$WORK_DIR/capabilities.json" "isinstance(v['hardware_acceleration'], dict) and 'encoders' in v and 'disc' in v and 'filters' in v"
