# MediaForge

[![CI](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml/badge.svg)](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[中文版](README.md) · [Documentation site](https://lhy723.github.io/mediaforge/)

MediaForge is a local media toolkit designed for AI agents. It provides a small,
deterministic control plane around [FFmpeg](https://ffmpeg.org/) and FFprobe:
inspect media, plan an operation, execute it safely, and verify the result.
It offers both a command-line interface (CLI) and a stdin/stdout JSON Tool interface;
it is not a frontend project or GUI application.

## Key features

- Semantic operations instead of hand-written FFmpeg filter graphs for common jobs.
- Stable JSON responses with machine-readable error codes.
- A standard `inspect → plan → execute → verify` workflow.
- Safe output naming, collision checks, and explicit overwrite control.
- Progress output for long jobs without polluting machine-readable stdout.
- An explicit Raw FFmpeg escape hatch for advanced cases.
- A versioned Tool API schema and an installable Agent Skill.

FFmpeg remains the media engine; MediaForge provides the reliable agent-facing contract.
An agent can receive a predictable plan, structured errors, and post-operation verification
without constructing complex FFmpeg arguments by hand.

## Installation

MediaForge requires FFmpeg and FFprobe at runtime. Regular users do not need Rust:
install FFmpeg once, then run the installer for the target platform.

### macOS / Linux (prebuilt binary)

```bash
# macOS
brew install ffmpeg

# Debian/Ubuntu
sudo apt install ffmpeg

# Install MediaForge
curl -fsSL https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.sh | sh
```

### Windows PowerShell (prebuilt binary)

```powershell
choco install ffmpeg
irm https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.ps1 | iex
```

The installer selects the latest macOS Apple Silicon/Intel, Linux x64, or Windows x64 asset.
It installs to `~/.local/bin` on Unix and `%LOCALAPPDATA%\\MediaForge\\bin` on Windows,
then prints the required PATH hint. Pin a release with `MEDIAFORGE_VERSION=v0.1.0`.

### From source

Use Rust/Cargo for development or unreleased source:

```bash
cargo install --path . --bin media

# Local release build
cargo build --release --bin media
./target/release/media capabilities --json
```

## Supported platforms

MediaForge is tested and packaged for macOS, Linux, and Windows x64. FFmpeg and FFprobe are
not bundled, so both programs must be available on `PATH`. The Windows release binary is
`media.exe`; semantic commands and the JSON Tool contract are the same on Unix and Windows.

## Agent workflow

Inspect the source, preview the decision, execute the operation, and verify the result:

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

Every transformation command accepts `--dry-run`. With `--json`, only JSON is written to stdout;
FFmpeg diagnostics go to stderr. With `--progress`, human CLI mode reports percentage, elapsed time,
estimated remaining time, and speed. JSON/Tool mode emits progress NDJSON on stderr while keeping one final JSON response on stdout.

## Supported operations

| Command | Function |
| --- | --- |
| `inspect` | Returns structured container, file, duration, bitrate, tag, video, audio, and subtitle information. |
| `plan` | Produces a no-write plan and reports copy/remux/transcode strategy, encoders, hardware, quality loss, subtitle/metadata handling, and warnings. |
| `convert` | Converts containers and codecs, automatically choosing stream copy, remux, or transcode; supports device presets. |
| `compress` | Compresses video with `lossless`, `very-high`, `high`, `balanced`, `small`, or `tiny`, plus target-size mode. |
| `resize` | Resizes by width or resolutions such as `1080p`, preserving aspect ratio and normalizing to even dimensions. |
| `clip` | Clips with a start plus duration or end time; uses lossless copy when compatible. |
| `extract-audio` | Extracts audio from video, copying a compatible source codec before transcoding. |
| `thumbnail` | Extracts a JPEG frame at a second, timecode, or percentage position. |
| `image` | Converts, resizes, rotates, watermarks, and controls image quality. |
| `gif` | Creates a palette-optimized animated GIF from video with start, duration, FPS, and width controls. |
| `edit` | Supports crop, rotate, speed, volume, grayscale/blur/sharpen/vintage filters, subtitle burn-in, and time ranges. |
| `merge` | Supports `concat`, video-plus-audio `mux`, and audio `mix`. |
| `audio` | Converts and processes audio bitrate, sample rate, channels, volume, and time ranges. |
| `repair` | Performs timestamp/corruption-tolerant repair and optional H.264/AAC re-encoding. |
| `disc` | Extracts DVD/CD/ISO sources and creates ISO images from directories. |
| `batch` | Converts files, directories, or globs recursively, with output-directory and partial-success reporting. |
| `verify` | Checks parseability, size, duration, streams, resolution, codecs, and an FFmpeg decode sample. |
| `capabilities` | Reports FFmpeg version, hardware acceleration, encoders, formats, filters, external tools, and presets. |
| `presets` | Lists deterministic iPhone, iPad, Android, PSP, and car-player profiles. |
| `tool` | Reads one JSON request from stdin or `--request` and returns one JSON response. |
| `ffmpeg` | Passes native FFmpeg arguments through for advanced cases outside the semantic API. |

Common examples:

```bash
media compress video.mp4 --quality balanced --json
media convert video.mkv --to mp4 --video-codec h265 --quality high --json
media resize video.mp4 --resolution 1080p --dry-run --json
media clip video.mp4 --start 00:10:00 --duration 30 --json
media extract-audio video.mp4 --format flac --json
media thumbnail video.mp4 --at 50% --json
media image poster.png --to webp --width 1280 --image-quality 85 --json
media gif video.mp4 --start 00:00:10 --duration 3 --fps 12 --width 480 --json
media edit video.mp4 --crop 1280:720:0:0 --rotate 90 --speed 1.25 --json
media merge first.mp4 second.mp4 --mode concat --json
media audio video.mp4 --format mp3 --bitrate 128k --sample-rate 44100 --json
media repair damaged.mp4 --reencode --json
media convert video.mp4 --device psp --json
media batch './videos/*.mov' --convert mp4 --json
```

## Formats, codecs, and hardware

Supported containers include `mp4`, `mkv`, `mov`, `webm`, `avi`, `wmv/asf`, `flv`, `ogv`, `3gp`,
`mpg/mpeg`, `vob`, and `swf`. Images include PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, TGA, and AVIF.
Audio includes MP3, AAC/M4A, FLAC, WAV, Opus, OGG/Vorbis, WMA, AIFF, ALAC, AMR, AC-3, and MP2.

Video codecs include H.264, H.265/HEVC, VP9, AV1, MPEG-4, MPEG-2, FLV1, WMV2, and Theora,
plus `auto` and `copy`. Container-aware defaults are H.264/AAC for MP4/MOV, VP9/Opus for WebM,
Theora/Opus for OGV, WMV2/WMA for WMV, and MPEG-2/MP2 for MPEG/VOB.

Hardware modes are `auto`, `cpu`, and `gpu`. The capability probe detects VideoToolbox, NVENC,
QSV, VAAPI, and AMF. `auto` intentionally uses deterministic CPU encoding; `gpu` probes the
hardware encoders available in the installed FFmpeg build. Run `media capabilities --json` to inspect the actual environment.

Target-size compression normally uses two-pass software encoding when hardware encoding is not selected,
and verification checks that the rendered file does not exceed the requested target. GIF FPS is limited to
1–60, duration to 600 seconds, and width to 16384 pixels. Edit speed is limited to 0.25–4 and volume to 0–10.

## JSON Tool API and Agent Skill

Agents can use `media tool` as a one-request/one-response stdin/stdout protocol:

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

The Tool API supports semantic operations, stable aliases, `dry_run`, `overwrite`,
`verify_after_execute`, `progress`, device presets, image/video/audio parameters, and
`operation: "ffmpeg"` for native arguments. The complete contract is [`schemas/tool-api.json`](schemas/tool-api.json).

The repository includes an installable Agent Skill:

- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md): workflow, safety rules, examples, and error handling.
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml): discovery metadata and default prompt.

This release provides the stdio JSON Tool and Skill, but not a native MCP Server. An MCP client
can wrap `media tool` as a subprocess; direct MCP configuration requires an additional adapter.

## Configuration

Load an optional TOML file through `MEDIAFORGE_CONFIG`, `$XDG_CONFIG_HOME/mediaforge/config.toml`,
or `~/.config/mediaforge/config.toml`. The same defaults apply to the CLI and Tool API:

```toml
default_quality = "balanced"
hardware = "auto"
overwrite = false
verify_after_execute = true
progress = false

[video]
preferred_codec = "auto"

[audio]
preferred_codec = "aac"
```

Configuration can set default quality, hardware, verification, progress, and preferred codecs,
but cannot implicitly enable overwriting. Trusted high-throughput jobs may set
`verify_after_execute = false`; the response marks verification as skipped.

## Safety and errors

- Source files are never modified.
- Identical input and output paths are rejected.
- Existing outputs receive `_1`, `_2`, … suffixes unless `--overwrite` is explicit.
- Successful transforms run an operation-appropriate verification.
- JSON errors contain `status`, `code`, `message`, `details`, and `suggestions`.
- Common error codes include `FILE_NOT_FOUND`, `INVALID_MEDIA`, `UNSUPPORTED_FORMAT`, `UNSUPPORTED_CODEC`,
  `ENCODER_UNAVAILABLE`, `HARDWARE_UNAVAILABLE`, `FFMPEG_NOT_FOUND`, `FFMPEG_FAILED`, and `VERIFY_FAILED`.
- DVD/CD devices, subtitle burn-in, and ISO authoring depend on OS permissions, the FFmpeg build, and optional tools; limitations are surfaced as warnings or structured errors.

## Project map, status, and license

- [`schemas/tool-api.json`](schemas/tool-api.json): machine-readable Tool API contract.
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md): Agent usage instructions.
- [`docs/architecture.md`](docs/architecture.md): control-plane design and invariants.
- [`docs/development.md`](docs/development.md): development, testing, and release process.
- [`scripts/install.sh`](scripts/install.sh) / [`scripts/install.ps1`](scripts/install.ps1): prebuilt installers.
- [MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/): static introduction and Agent API guide.

MediaForge is an actively evolving Rust implementation focused on deterministic local processing
through FFmpeg/FFprobe. Remote storage, model-powered editing decisions, and long-running job services
are outside the current CLI scope.

Bug reports and focused pull requests are welcome. MediaForge is released under the [MIT License](LICENSE).
