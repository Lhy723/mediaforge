# Core inspect, planning, conversion, and media-operation scenarios.

"$BIN" inspect "$SRC" --json > "$WORK_DIR/inspect.json"
assert_json "$WORK_DIR/inspect.json" "v['status'] == 'success' and len(v['video']) == 1 and len(v['audio']) == 1"

"$BIN" plan "$SRC" --to mkv --json > "$WORK_DIR/plan-convert.json"
assert_json "$WORK_DIR/plan-convert.json" "v['status'] == 'planned' and v['operation'] == 'convert' and v['strategy'] in ('copy', 'remux')"

"$BIN" plan "$SRC" --target-size 1MB --json > "$WORK_DIR/plan-compress.json"
assert_json "$WORK_DIR/plan-compress.json" "v['status'] == 'planned' and v['operation'] == 'compress' and v['passes'] == 2 and v['pass_strategy'] == 'two_pass'"

ffmpeg -hide_banner -loglevel error -i "$SRC" -c copy "$WORK_DIR/source.mov"
ffmpeg -hide_banner -loglevel error -i "$SRC" -c copy "$WORK_DIR/source.mkv"
ffmpeg -hide_banner -loglevel error -i "$SRC" -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -c:a libopus "$WORK_DIR/source.webm"
SRC_JSON="$(native_path "$SRC")"
SOURCE_MKV_JSON="$(native_path "$WORK_DIR/source.mkv")"
for media in "$SRC" "$WORK_DIR/source.mkv" "$WORK_DIR/source.mov" "$WORK_DIR/source.webm"; do
  name="$(basename "$media" | tr '.' '-')"
  "$BIN" inspect "$media" --json > "$WORK_DIR/inspect-$name.json"
  assert_json "$WORK_DIR/inspect-$name.json" "v['status'] == 'success' and len(v['video']) == 1 and len(v['audio']) == 1"
done

"$BIN" plan "$WORK_DIR/source.mkv" --to mp4 --json > "$WORK_DIR/plan-remux.json"
assert_json "$WORK_DIR/plan-remux.json" "v['strategy'] == 'remux' and v['video']['action'] == 'copy' and v['audio']['action'] == 'copy'"
"$BIN" convert "$WORK_DIR/source.mkv" --to mp4 --output "$WORK_DIR/remuxed.mp4" --json > "$WORK_DIR/remux.json"
assert_json "$WORK_DIR/remux.json" "v['status'] == 'success' and v['strategy'] == 'remux' and v['verification']['valid'] is True"

"$BIN" convert "$SRC" --to mkv --output "$WORK_DIR/converted.mkv" --json > "$WORK_DIR/convert.json"
assert_json "$WORK_DIR/convert.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" convert "$SRC" --to webm --output "$WORK_DIR/converted.webm" --json > "$WORK_DIR/convert-webm.json"
assert_json "$WORK_DIR/convert-webm.json" "v['status'] == 'success' and v['video']['codec'] == 'vp9' and v['audio']['to'] == 'opus' and v['verification']['valid'] is True"
[[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=codec_name -of csv=p=0 "$WORK_DIR/converted.webm")" == "vp9" ]] || {
  echo "automatic WebM conversion did not select VP9" >&2
  exit 1
}
[[ "$(ffprobe -v error -select_streams a:0 -show_entries stream=codec_name -of csv=p=0 "$WORK_DIR/converted.webm")" == "opus" ]] || {
  echo "automatic WebM conversion did not select Opus" >&2
  exit 1
}

if "$BIN" convert "$SRC" --to webm --video-codec h264 --dry-run --json > "$WORK_DIR/webm-invalid-codec.json"; then
  echo "convert unexpectedly accepted H.264 for WebM" >&2
  exit 1
fi
assert_json "$WORK_DIR/webm-invalid-codec.json" "v['code'] == 'UNSUPPORTED_CODEC'"

"$BIN" convert "$SRC" --to mkv --video-codec h265 --quality tiny --dry-run --json > "$WORK_DIR/convert-quality.json"
assert_json "$WORK_DIR/convert-quality.json" "v['quality'] == 'tiny' and '-crf' in v['ffmpeg_args'] and v['ffmpeg_args'][v['ffmpeg_args'].index('-crf') + 1] == '34'"

"$BIN" plan "$SRC" --to mkv --quality high --dry-run --json > "$WORK_DIR/plan-convert-quality.json"
assert_json "$WORK_DIR/plan-convert-quality.json" "v['operation'] == 'convert' and v['quality'] == 'high'"

printf '1\n00:00:00,000 --> 00:00:01,000\nHello MediaForge\n' > "$WORK_DIR/caption.srt"
ffmpeg -hide_banner -loglevel error -i "$SRC" -f srt -i "$WORK_DIR/caption.srt" \
  -map 0 -map 1:0 -c:v copy -c:a copy -c:s srt "$WORK_DIR/caption.mkv"
"$BIN" plan "$WORK_DIR/caption.mkv" --to mp4 --json > "$WORK_DIR/plan-subtitle.json"
assert_json "$WORK_DIR/plan-subtitle.json" "v['subtitle']['action'] == 'convert_to_mov_text' and len(v['warnings']) == 1"
"$BIN" edit "$SRC" --subtitle "$WORK_DIR/caption.srt" \
  --subtitle-style 'FontSize=18,PrimaryColour=&H00FFFFFF' --dry-run --json \
  > "$WORK_DIR/plan-subtitle-style.json"
assert_json "$WORK_DIR/plan-subtitle-style.json" "v['status'] == 'planned' and v['subtitle_style'].startswith('FontSize=18') and 'force_style' in ' '.join(v['ffmpeg_args'])"
"$BIN" convert "$WORK_DIR/caption.mkv" --to mp4 --output "$WORK_DIR/caption.mp4" --json > "$WORK_DIR/caption.json"
assert_json "$WORK_DIR/caption.json" "v['status'] == 'success' and v['verification']['valid'] is True"
[[ "$(ffprobe -v error -select_streams s -show_entries stream=codec_name -of csv=p=0 "$WORK_DIR/caption.mp4")" == "mov_text" ]] || {
  echo "subtitle stream was not converted to mov_text" >&2
  exit 1
}

"$BIN" compress "$SRC" --quality tiny --output "$WORK_DIR/compressed.mp4" --json > "$WORK_DIR/compress.json"
assert_json "$WORK_DIR/compress.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" compress "$SRC" --target-size 100KB --output "$WORK_DIR/compressed-target.mp4" --json > "$WORK_DIR/compress-target.json"
assert_json "$WORK_DIR/compress-target.json" "v['status'] == 'success' and v['passes'] == 2 and v['pass_strategy'] == 'two_pass' and v['verification']['valid'] is True and v['verification']['checks']['target_size_match'] is True"

"$BIN" resize "$SRC" --resolution 120p --output "$WORK_DIR/resized.mp4" --json > "$WORK_DIR/resize.json"
assert_json "$WORK_DIR/resize.json" "v['status'] == 'success' and v['verification']['valid'] is True and v['verification']['checks']['target_dimension_match'] is True and v['verification']['checks']['even_dimensions_match'] is True"

"$BIN" resize "$SRC" --width 321 --dry-run --json > "$WORK_DIR/resize-odd.json"
assert_json "$WORK_DIR/resize-odd.json" "v['target_dimension']['requested'] == 321 and v['target_dimension']['effective'] == 322 and 'scale=322:-2' == v['filter']"

ffmpeg -hide_banner -loglevel error -f lavfi -i "color=c=blue:size=3840x2160:rate=1" \
  -t 1 -c:v libx264 -preset ultrafast -pix_fmt yuv420p "$WORK_DIR/source-4k.mp4"
"$BIN" resize "$WORK_DIR/source-4k.mp4" --resolution 1080p --output "$WORK_DIR/resized-1080p.mp4" --json > "$WORK_DIR/resize-4k.json"
assert_json "$WORK_DIR/resize-4k.json" "v['status'] == 'success' and v['verification']['valid'] is True and v['verification']['checks']['target_dimension_match'] is True"
[[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 "$WORK_DIR/resized-1080p.mp4")" == "1920x1080" ]] || {
  echo "4K resize did not produce 1920x1080 output" >&2
  exit 1
}

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

for format in mp3 aac wav opus alac; do
  "$BIN" extract-audio "$SRC" --format "$format" --output "$WORK_DIR/audio.$format" --json > "$WORK_DIR/audio-$format.json"
  assert_json "$WORK_DIR/audio-$format.json" "v['status'] == 'success' and v['verification']['valid'] is True"
done

"$BIN" thumbnail "$SRC" --at 50% --output "$WORK_DIR/thumbnail.jpg" --json > "$WORK_DIR/thumbnail.json"
assert_json "$WORK_DIR/thumbnail.json" "v['status'] == 'success' and v['verification']['valid'] is True"

"$BIN" image "$SRC" --to jpg --width 160 --image-quality 85 \
  --output "$WORK_DIR/frame.jpg" --json > "$WORK_DIR/image.json"
assert_json "$WORK_DIR/image.json" "v['status'] == 'success' and v['operation'] == 'image' and v['verification']['valid'] is True"
[[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$WORK_DIR/frame.jpg")" == "160" ]] || {
  echo "image conversion did not honor the requested width" >&2
  exit 1
}

"$BIN" gif "$SRC" --duration 1 --fps 8 --width 96 \
  --output "$WORK_DIR/preview.gif" --json > "$WORK_DIR/gif.json"
assert_json "$WORK_DIR/gif.json" "v['status'] == 'success' and v['operation'] == 'gif' and v['verification']['valid'] is True"
[[ "$(ffprobe -v error -show_entries format=format_name -of csv=p=0 "$WORK_DIR/preview.gif")" == "gif" ]] || {
  echo "GIF conversion did not produce a GIF container" >&2
  exit 1
}

"$BIN" edit "$SRC" --crop 160:120:0:0 --rotate 90 --speed 1.0 \
  --output "$WORK_DIR/edited.mp4" --json > "$WORK_DIR/edit.json"
assert_json "$WORK_DIR/edit.json" "v['status'] == 'success' and v['operation'] == 'edit' and v['verification']['valid'] is True"

"$BIN" merge "$SRC" "$WORK_DIR/source.mkv" --mode concat \
  --output "$WORK_DIR/merged.mp4" --json > "$WORK_DIR/merge.json"
assert_json "$WORK_DIR/merge.json" "v['status'] == 'success' and v['operation'] == 'merge' and v['verification']['valid'] is True"

"$BIN" audio "$SRC" --format mp3 --bitrate 128k --sample-rate 44100 --channels 1 \
  --output "$WORK_DIR/audio-converted.mp3" --json > "$WORK_DIR/audio-converted.json"
assert_json "$WORK_DIR/audio-converted.json" "v['status'] == 'success' and v['operation'] == 'audio' and v['verification']['valid'] is True"

"$BIN" repair "$SRC" --output "$WORK_DIR/repaired.mp4" --json > "$WORK_DIR/repair.json"
assert_json "$WORK_DIR/repair.json" "v['status'] == 'success' and v['operation'] == 'repair' and v['verification']['valid'] is True"

"$BIN" presets --json > "$WORK_DIR/presets.json"
assert_json "$WORK_DIR/presets.json" "v['status'] == 'success' and len(v['presets']) >= 5"

"$BIN" convert "$SRC" --device psp --dry-run --json > "$WORK_DIR/device.json"
assert_json "$WORK_DIR/device.json" "v['status'] == 'planned' and v['device']['id'] == 'psp' and v['device']['max_height'] == 480"

"$BIN" disc "$SRC" --kind dvd --to mp4 --dry-run --json > "$WORK_DIR/disc.json"
assert_json "$WORK_DIR/disc.json" "v['status'] == 'planned' and v['operation'] == 'disc' and v['kind'] == 'dvd'"
mkdir -p "$WORK_DIR/disc-input"
printf 'MediaForge ISO fixture\n' > "$WORK_DIR/disc-input/readme.txt"
"$BIN" disc "$WORK_DIR/disc-input" --kind dvd --action create-iso \
  --volume-label MEDIAFORGE --output "$WORK_DIR/fixture.iso" --dry-run --json \
  > "$WORK_DIR/disc-create.json"
assert_json "$WORK_DIR/disc-create.json" "v['status'] == 'planned' and v['action'] == 'create_iso' and v['tool_available'] in (True, False)"
