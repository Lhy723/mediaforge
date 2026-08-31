<a id="readme-top"></a>

<div align="right">
  <a href="README.md"><strong>English</strong></a> |
  <a href="README.zh-CN.md">简体中文</a>
</div>

<div align="center">
  <h1>MediaForge</h1>
  <p>
    <strong>Deterministic media processing for AI agents</strong><br />
    <sub>Inspect → plan → execute → verify, powered by FFmpeg.</sub>
  </p>
  <p>
    <img src="https://img.shields.io/github/actions/workflow/status/Lhy723/mediaforge/ci.yml?style=flat&label=CI" alt="CI" />
    <img src="https://img.shields.io/badge/License-MIT-89B4FA?style=flat&logo=opensourceinitiative&logoColor=white" alt="License: MIT" />
    <img src="https://img.shields.io/github/stars/Lhy723/mediaforge?style=flat&color=F5C2E7&label=stars" alt="Stars" />
    <img src="https://img.shields.io/badge/CLI-media-A6E3A1?style=flat&logo=gnu-bash&logoColor=black" alt="CLI: media" />
    <img src="https://img.shields.io/badge/OS-macOS%20%7C%20Linux%20%7C%20Windows-1793D1?style=flat" alt="Platforms" />
    <img src="https://img.shields.io/badge/Agent-Tool%20%2B%20Skill-F5C2E7?style=flat" alt="Agent Tool and Skill" />
  </p>
  <p><code>inspect</code> · <code>plan</code> · <code>execute</code> · <code>verify</code></p>
  <p>
    <a href="https://lhy723.github.io/mediaforge/">Documentation site</a>
    ·
    <a href="https://github.com/Lhy723/mediaforge/issues">Issues</a>
  </p>
</div>

MediaForge is a small, local control plane around [FFmpeg](https://ffmpeg.org/) and FFprobe.
It turns media intent into an inspectable plan, executes it with safe defaults, and verifies the result.
It provides both a CLI and a stdin/stdout JSON Tool interface; it is not a GUI, remote-storage service, or job server.

## At a glance

| Local-first | Agent-ready | Capability-aware |
| --- | --- | --- |
| Runs on your machine through FFmpeg/FFprobe. | One JSON request in, one JSON response out. | Probes codecs, hardware, filters, and optional tools at runtime. |

## Features

- **Inspect** — normalized container, stream, codec, HDR, subtitle, metadata, and duration data.
- **Plan** — explain copy/remux/transcode choices, quality loss, output paths, warnings, and FFmpeg arguments before execution.
- **Transform** — convert, compress, resize, clip, extract audio, make thumbnails/GIFs, edit, merge, repair, and process discs.
- **Verify** — parseability, size, duration, streams, dimensions, codecs, and a decode sample, with operation-specific checks.
- **Agent interface** — a stable Tool API schema, operation aliases, structured errors, progress events, and an installable Skill.
- **Safe by default** — never modifies sources, rejects path collisions, avoids implicit overwrite, and reports every decision.
- **Raw escape hatch** — pass an explicit FFmpeg argument vector for jobs outside the semantic API.

## Install

MediaForge requires `ffmpeg` and `ffprobe` on `PATH`. The prebuilt installer does not bundle either dependency.

### Standalone (macOS / Linux)

```bash
# Install the runtime dependency first (choose your platform)
brew install ffmpeg                 # macOS
sudo apt install ffmpeg             # Debian/Ubuntu

# Install the latest MediaForge binary
curl -fsSL https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.sh | sh
```

The installer selects macOS Apple Silicon/Intel or Linux x64, installs `media` to `~/.local/bin`,
and prints a PATH hint. Pin a release with `MEDIAFORGE_VERSION=v0.1.0`.

### Standalone (Windows PowerShell)

```powershell
choco install ffmpeg
irm https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.ps1 | iex
```

The Windows binary is `media.exe` and is installed under `%LOCALAPPDATA%\\MediaForge\\bin`.

MediaForge is tested and packaged for macOS, Linux, and Windows x64. FFmpeg and FFprobe are
not bundled; both programs must be available on `PATH`.

### From source

```bash
cargo install --path . --bin media

# Or build a local release
cargo build --release --bin media
./target/release/media capabilities --json
```

## Quick start

The recommended loop is `inspect → plan → execute → verify`:

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

Preview any transformation with `--dry-run`. Use `--progress` for percentage, elapsed time,
estimated remaining time, and speed. JSON/Tool mode keeps stdout machine-readable and emits progress NDJSON on stderr.

Common global flags are `--json`, `--dry-run`, `--overwrite`, `--verbose`, `--debug`, and `--progress`.

## Operations

### Media

| Command | What it does |
| --- | --- |
| `inspect` | Returns normalized file, container, video, audio, subtitle, and metadata information. |
| `plan` | Produces a no-write plan with strategy, codecs, hardware, quality loss, warnings, and FFmpeg arguments. |
| `convert` | Converts containers/codecs and can apply iPhone, iPad, Android, PSP, or car presets. |
| `compress` | Uses quality presets or a target size; software target-size jobs normally use two passes. |
| `resize` | Resizes by width or a height such as `1080p`, preserving aspect ratio and using even dimensions. |
| `clip` | Clips by start + duration/end; uses stream copy when the requested MP4 clip is compatible. |
| `extract-audio` | Extracts audio and copies a compatible source codec before transcoding. |
| `thumbnail` | Extracts a JPEG frame at seconds, `HH:MM:SS`, or a percentage. |
| `image` | Converts, resizes, rotates, watermarks, and controls still-image quality. |
| `gif` | Creates an animated, palette-optimized GIF from video. |

### Edit and compose

| Command | What it does |
| --- | --- |
| `edit` | Crop (`WIDTH:HEIGHT:X:Y`), rotate, speed `0.25–4`, volume `0–10`, named filters, subtitle burn-in, ASS/SSA styles, and time ranges. |
| `merge` | `concat` joins inputs; `mux` combines video + audio; `mix` mixes two audio tracks. |
| `audio` | Converts audio and controls format, bitrate, sample rate, channels, volume, and ranges. |
| `repair` | Performs timestamp/corruption-tolerant remuxing or optional H.264/AAC re-encoding. |
| `disc` | Extracts DVD/CD/ISO sources or creates an ISO from a directory using an available authoring tool. |

### Automation and interfaces

| Command | What it does |
| --- | --- |
| `batch` | Converts files, directories, or globs recursively and reports partial success; currently uses `--convert FORMAT`. |
| `verify` | Validates an input/output pair, including operation-specific duration, size, and dimension checks. |
| `capabilities` | Reports the installed FFmpeg version, encoders, hardware acceleration, formats, filters, devices, and tools. |
| `presets` | Lists deterministic device profiles for iPhone, iPad, Android, PSP, and car players. |
| `tool` | Reads one JSON request from stdin or `--request` and returns one JSON response. |
| `ffmpeg` | Passes native FFmpeg arguments through for advanced cases. |

## Format matrix

| Media | Supported formats |
| --- | --- |
| Containers | MP4, MKV/Matroska, MOV/QuickTime, WebM, AVI, WMV/ASF, FLV, OGV, 3GP, MPG/MPEG, VOB, SWF |
| Images | PNG, JPEG (`jpg`/`jpeg`), WebP, GIF, BMP, TIFF (`tif`/`tiff`), ICO, TGA, AVIF |
| Audio | MP3, AAC/M4A, FLAC, WAV, Opus, OGG/Vorbis, WMA, AIFF, ALAC, AMR, AC-3, MP2 |
| Video codecs | H.264, H.265/HEVC, VP9, AV1, MPEG-4, MPEG-2, FLV1, WMV2, Theora, `auto`, `copy` |

Container-aware defaults are H.264/AAC for MP4/MOV, VP9/Opus for WebM, Theora/Opus for OGV,
WMV2/WMA for WMV, and MPEG-2/MP2 for MPEG/VOB. Actual encoder availability depends on the installed FFmpeg build.

## For AI agents

### JSON Tool

Use the stdio Tool entrypoint when the host can pipe structured input:

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

The Tool API accepts semantic operations and stable aliases such as `inspect_media`, `plan_media_operation`,
`convert_media`, `create_thumbnail`, `image_convert`, `video_to_gif`, `edit_media`, `audio_convert`,
`repair_media`, `verify_media`, and `device_presets`. Requests can include `dry_run`, `overwrite`,
`verify_after_execute`, `progress`, codecs, quality, hardware, device presets, image/edit/audio parameters,
disc actions, merge inputs, and raw `ffmpeg` arguments.

Errors always use the shape `status`, `code`, `message`, `details`, and `suggestions`.
The complete contract is [`schemas/tool-api.json`](schemas/tool-api.json).

### Agent Skill

The repository includes an installable Skill for hosts that support local Agent Skills:

- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — workflow, safety rules, examples, and error handling.
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml) — discovery metadata and default prompt.

This release provides the stdio JSON Tool and Skill, but not a native MCP Server. An MCP client can wrap
`media tool` as a subprocess; direct MCP configuration requires an adapter.

## Safety and observability

- Source files are never modified.
- Identical input/output paths are rejected.
- Existing outputs receive `_1`, `_2`, … suffixes unless `--overwrite` is explicit.
- Successful transforms run an operation-appropriate verification; trusted jobs may opt out explicitly.
- `--verbose` and `--debug` send diagnostics to stderr.
- `--progress` sends human progress or NDJSON events to stderr without breaking stdout parsing.
- DVD/CD access, subtitle burn-in, ISO authoring, and hardware encoding are capability-dependent and return actionable warnings/errors.

## Configuration

Load optional TOML defaults from `MEDIAFORGE_CONFIG`, `$XDG_CONFIG_HOME/mediaforge/config.toml`,
or `~/.config/mediaforge/config.toml`:

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

Defaults apply to both CLI and Tool calls. Configuration cannot implicitly enable overwrite.

## Repository map

- [`schemas/tool-api.json`](schemas/tool-api.json) — machine-readable Tool API contract.
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — Agent usage instructions.
- [`docs/architecture.md`](docs/architecture.md) — control-plane design and invariants.
- [`docs/development.md`](docs/development.md) — development, testing, and release process.
- [`scripts/install.sh`](scripts/install.sh) / [`scripts/install.ps1`](scripts/install.ps1) — prebuilt installers.
- [MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/) — static introduction and API guide.

## Status and license

MediaForge is an evolving Rust implementation focused on deterministic local FFmpeg/FFprobe processing.
Remote storage, model-powered editing decisions, and long-running job services are outside the current CLI scope.

Bug reports and focused pull requests are welcome. MediaForge is released under the [MIT License](LICENSE).
