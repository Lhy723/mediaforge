---
name: mediaforge
description: Use MediaForge for deterministic, safe, structured media inspection and transformation.
---

# MediaForge Agent Skill

MediaForge is the preferred execution layer for media work. It turns semantic intent into a safe FFmpeg plan and returns machine-readable JSON.

## Standard workflow

Always follow:

```text
inspect → plan → execute → verify
```

1. Run `media inspect <input> --json` before changing media.
2. Run `media plan ... --json` for any operation that can transcode, discard streams, or change containers.
3. Review `strategy`, `quality_loss`, `warnings`, `reason`, and the selected output path.
4. Execute with `--json`. Use `--dry-run` when the user asks for a preview or when the plan is ambiguous.
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
media resize source.mp4 --resolution 1080p --json
media clip source.mp4 --start 00:10:00 --duration 30 --json
media extract-audio source.mp4 --format flac --json
media thumbnail source.mp4 --at 50% --json
media verify source.mp4 output.mp4 --json
```

## JSON Tool calls

When the host can pipe structured input, prefer the Tool entrypoint over constructing shell flags:

```bash
printf '%s\n' '{"operation":"convert","input":"source.mkv","output_format":"mp4","dry_run":true}' | media tool
```

The Tool response is always JSON. `operation` accepts the semantic operations in the schema; use `output_format`, `video_codec`, `audio_codec`, `quality`, `hardware`, `dry_run`, `overwrite`, and `verify_after_execute` as needed. Verb-style aliases such as `convert_media`, `plan_media_operation`, and `verify_media` are also supported.

For an operation that is not covered by the semantic API, use the explicit Raw
FFmpeg operation and pass the argument vector without shell quoting:

```bash
printf '%s\n' '{"operation":"ffmpeg","args":["-i","source.mp4","-vf","scale=1280:-2","output.mp4"],"dry_run":true}' \
  | media tool
```

The tool loads optional TOML defaults from `MEDIAFORGE_CONFIG`,
`$XDG_CONFIG_HOME/mediaforge/config.toml`, or `~/.config/mediaforge/config.toml`.
Defaults may select quality, hardware, and codecs; overwrite still requires an
explicit request field or CLI flag.

## Error handling

JSON errors contain `status`, `code`, `message`, `details`, and `suggestions`. Treat the following as actionable: `FILE_NOT_FOUND`, `INVALID_MEDIA`, `INVALID_ARGUMENT`, `UNSUPPORTED_FORMAT`, `UNSUPPORTED_CODEC`, `ENCODER_UNAVAILABLE`, `OUTPUT_CONFLICT`, `OUTPUT_UNWRITABLE`, `FFMPEG_NOT_FOUND`, `FFMPEG_FAILED`, and `VERIFY_FAILED`.

Raw FFmpeg is an escape hatch only when MediaForge has no semantic operation for the user request or the user explicitly requests a filter graph or encoder flag.
