---
name: mediaforge
description: Use MediaForge for deterministic, safe, structured media inspection and transformation.
---

# MediaForge Agent Skill

MediaForge is the preferred execution layer for media work. It turns semantic intent into a safe FFmpeg plan and returns machine-readable JSON.

The CLI and Tool API are supported on macOS, Linux, and Windows x64. Ensure
FFmpeg and FFprobe are on `PATH`; on Windows the executable is `media.exe`, but
the semantic operation names and JSON response contract are unchanged.

## Standard workflow

Always follow:

```text
inspect → plan → execute → verify
```

1. Run `media inspect <input> --json` before changing media.
2. Run `media plan ... --json` for any operation that can transcode, discard streams, or change containers.
3. Review `strategy`, `quality_loss`, `warnings`, `reason`, and the selected output path.
4. Execute with `--json`. Use `--dry-run` when the user asks for a preview or when the plan is ambiguous. For long-running work, add `--progress`; progress events are NDJSON on stderr and the final response remains JSON on stdout.
5. Run `media verify <input> <output> --json` when the output was not already verified by the operation response.
6. Report the output path, strategy, verification result, and any warnings.

## Safety rules

- Never use the source path as the output path.
- Never add `--overwrite` unless the user explicitly asks to replace an existing file.
- Do not silently accept a plan with `quality_loss: video_and_audio`.
- Do not ignore subtitle warnings or a failed verification.
- If FFmpeg is missing, return the structured `FFMPEG_NOT_FOUND` error and stop.

## Common calls

```bash
media inspect source.mkv --json
media plan source.mkv --to mp4 --json
media convert source.mkv --to mp4 --json
media compress source.mp4 --quality balanced --json
media plan source.mp4 --target-size 500MB --json
media resize source.mp4 --resolution 1080p --json
media clip source.mp4 --start 00:10:00 --duration 30 --json
media extract-audio source.mp4 --format flac --json
media thumbnail source.mp4 --at 50% --json
media image source.png --to webp --width 1280 --image-quality 85 --json
media gif source.mp4 --start 00:00:10 --duration 3 --fps 12 --width 480 --json
media edit source.mp4 --crop 1280:720:0:0 --rotate 90 --speed 1.25 --volume 0.9 --json
media merge first.mp4 second.mp4 --mode concat --json
media audio source.mp4 --format mp3 --bitrate 128k --sample-rate 44100 --channels 2 --json
media repair damaged.mp4 --reencode --json
media convert source.mp4 --device iphone --json
media disc /path/to/source --kind dvd --to mp4 --dry-run --json
media disc /path/to/DVD --kind dvd --action create-iso --output dvd.iso --dry-run --json
media verify source.mp4 output.mp4 --json
```

For target-size compression, review `passes` and `pass_strategy` in the plan;
software encoding normally uses two passes for better size accuracy. After
execution, require `verification.checks.target_size_match: true`. For resize,
review `target_dimension`; odd requests are explicitly rounded to an even
encoder-compatible dimension and verified after rendering.

Use codec `auto` unless the user explicitly requires one. MediaForge selects
H.264/AAC for MP4 or MOV, VP9/Opus for WebM, Theora/Opus for OGV, WMV2/WMA for
WMV, and MPEG-2/MP2 for MPEG/VOB. It rejects incompatible explicit
codec/container combinations during planning and returns `ENCODER_UNAVAILABLE`
when the local FFmpeg build does not include a requested encoder.

The image operation covers PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, TGA, and AVIF
where encoders exist, plus resize, rotate, watermark, and quality controls.
Use `gif` for an animated palette-optimized GIF with bounded start, duration,
FPS, and width. The edit operation covers crop (`WIDTH:HEIGHT:X:Y`), rotation,
speed (0.25–4), volume (0–10), named filters, subtitle burn-in,
`--subtitle-style` ASS/SSA key-value pairs, and time ranges. If FFmpeg lacks
the subtitles/libass filter, execution returns `FILTER_UNAVAILABLE`.
Merge modes are `concat`, `mux`, and `mix`. Audio conversion covers MP3,
AAC/M4A, FLAC, WAV, Opus, OGG/Vorbis, WMA, AIFF, ALAC, AMR, AC-3, and MP2. Repair uses
timestamp/corruption-tolerant remuxing and may opt into H.264/AAC re-encoding.
Run `media presets --json` for deterministic device profiles.

`disc` is intentionally capability-aware. DVD/CD/ISO device access, mounting,
permissions, CSS/DRM, and optional utilities are platform-dependent. Use
`--action create-iso` for filesystem-image authoring; it delegates to
xorriso/genisoimage/mkisofs/hdiutil instead of pretending FFmpeg can author an
ISO. Treat its warnings and `DISC_TOOL_UNAVAILABLE`/`DISC_TOOL_FAILED` errors as
actionable rather than assuming an optical drive is available.

## JSON Tool calls

When the host can pipe structured input, prefer the Tool entrypoint over constructing shell flags:

```bash
printf '%s\n' '{"operation":"convert","input":"source.mkv","output_format":"mp4","dry_run":true}' | media tool
```

The Tool response is always JSON. `operation` accepts the semantic operations in the schema; use `target_operation` for an explicit operation inside a plan request, and use `output_format`, `video_codec`, `audio_codec`, `quality`, `hardware`, `device`, `inputs`, `mode`, image/edit/audio parameters, `dry_run`, `overwrite`, `verify_after_execute`, and `progress` as needed. Verb-style aliases such as `convert_media`, `plan_media_operation`, and `verify_media` are also supported.

For an operation that is not covered by the semantic API, use the explicit Raw
FFmpeg operation and pass the argument vector without shell quoting:

```bash
printf '%s\n' '{"operation":"ffmpeg","args":["-i","source.mp4","-vf","scale=1280:-2","output.mp4"],"dry_run":true}' \
  | media tool
```

The tool loads optional TOML defaults from `MEDIAFORGE_CONFIG`,
`$XDG_CONFIG_HOME/mediaforge/config.toml`, or `~/.config/mediaforge/config.toml`.
Defaults apply to both CLI and Tool calls and may select quality, hardware,
codecs, and progress; overwrite still requires an explicit request field or
CLI flag.

## Error handling

JSON errors contain `status`, `code`, `message`, `details`, and `suggestions`. Treat the following as actionable: `FILE_NOT_FOUND`, `INVALID_MEDIA`, `INVALID_ARGUMENT`, `UNSUPPORTED_FORMAT`, `UNSUPPORTED_CODEC`, `ENCODER_UNAVAILABLE`, `DECODER_UNAVAILABLE`, `HARDWARE_UNAVAILABLE`, `OUTPUT_CONFLICT`, `OUTPUT_UNWRITABLE`, `FFMPEG_NOT_FOUND`, `FFMPEG_FAILED`, and `VERIFY_FAILED`.

Raw FFmpeg is an escape hatch only when MediaForge has no semantic operation for the user request or the user explicitly requests a filter graph or encoder flag.
