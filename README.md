# MediaForge

[![CI](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml/badge.svg)](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

MediaForge is an agent-native media processing toolkit. It gives an AI agent a small, deterministic control plane over [FFmpeg](https://ffmpeg.org/) and FFprobe: inspect media, plan an operation, execute it safely, and verify the result. It is a CLI and a JSON-over-stdin tool—not a frontend application.

## Why MediaForge

FFmpeg is powerful but exposes a large, stateful command surface. MediaForge adds the agent-facing contract around it:

- semantic operations instead of hand-written filter graphs for common jobs;
- stable JSON responses with machine-readable error codes;
- `inspect → plan → execute → verify` workflow;
- safe output naming, collision checks, and no implicit overwrite;
- an explicit Raw FFmpeg escape hatch for advanced cases;
- a versioned Tool API schema and an installable Agent Skill.

## Install

FFmpeg and FFprobe must be available on `PATH`.

```bash
cargo install --path . --bin media
```

For a local release build:

```bash
cargo build --release --bin media
./target/release/media capabilities --json
```

## Supported platforms

MediaForge is tested and packaged for macOS, Linux, and Windows x64. Install
FFmpeg and FFprobe on `PATH` before using the CLI. On Windows, the release
binary is `media.exe`; the same semantic commands and JSON Tool API apply.

```powershell
choco install ffmpeg --yes
cargo build --release --bin media
.\target\release\media.exe capabilities --json
```

The Windows acceptance workflow runs through Git Bash and accepts either
`python3` or `python` for its small JSON assertions.

## Agent workflow

The normal CLI is useful during development and for shell-based agents:

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

All transformation commands accept `--dry-run`. JSON is emitted only on stdout; FFmpeg diagnostics stay on stderr and can be enabled with `--verbose` or `--debug`. Long-running transformations support `--progress`: human CLI mode prints percentage, elapsed time, estimated remaining time, and speed, while `--json`/Tool mode emits the same measurements as progress NDJSON on stderr without breaking the one-response JSON contract.

## Operations

```text
inspect, plan, convert, compress, resize, clip, extract-audio, thumbnail,
image, gif, edit, merge, audio, repair, disc, batch, verify, capabilities, presets
```

Examples:

```bash
media compress video.mp4 --quality balanced --json
media convert video.mkv --to mp4 --video-codec h265 --quality high --json
media plan video.mp4 --target-size 500MB --json
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
media disc /Volumes/DVD --kind dvd --to mp4 --dry-run --json
media disc /Volumes/DVD --kind dvd --action create-iso --output dvd.iso --dry-run --json
media batch './videos/*.mov' --convert mp4 --json
```

`media plan` accepts `--operation` when the target operation is known explicitly. If it is omitted, MediaForge infers `compress`, `resize`, `clip`, `extract_audio`, or `thumbnail` from operation-specific flags; otherwise it plans a container conversion. Plans report copy versus transcode decisions, metadata and subtitle handling, hardware selection, quality loss, warnings, and the collision-safe output path.

Clipping uses stream copy when a zero-offset MP4 clip is compatible with the source streams and uses precise re-encoding when the requested boundary requires it. Audio extraction also copies a compatible source codec (for example AAC to M4A) before falling back to a transcode.

Container-aware defaults select H.264/AAC for MP4 and MOV, VP9/Opus for WebM, Theora/Opus for OGV, WMV2/WMA for WMV, and MPEG-2/MP2 for MPEG/VOB. Explicit codec/container combinations are validated before FFmpeg starts. The format matrix also includes AVI, FLV, 3GP, SWF, and common still-image formats; availability is checked against the installed FFmpeg build at execution time.

The image subsystem supports PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, TGA, and AVIF where the local FFmpeg image encoders are present. It can resize, rotate, apply a watermark, and select a quality value. `gif` creates an animated, palette-optimized GIF from a video with bounded start/duration/FPS/width controls. `edit` covers crop, rotate, speed, volume, named filters (`grayscale`, `blur`, `sharpen`, `vintage`), subtitle burn-in, ASS/SSA `force_style` parameters, and time ranges. Subtitle burn-in reports `FILTER_UNAVAILABLE` when FFmpeg lacks the libass subtitles filter. `merge` offers concat, video-plus-audio mux, and audio mix modes.

`audio` supports MP3, AAC/M4A, FLAC, WAV, Opus, OGG/Vorbis, WMA, AIFF, ALAC, AMR, AC-3, and MP2, with bitrate, sample-rate, channel, volume, and range controls. `repair` attempts timestamp/corruption-tolerant remuxing and can opt into H.264/AAC re-encoding. `presets --json` exposes deterministic iPhone, iPad, Android, PSP, and car-player profiles. `disc` is a capability-aware FFmpeg bridge for DVD/CD/ISO sources; `--action create-iso` delegates filesystem-image creation to xorriso/genisoimage/mkisofs/hdiutil when available. Optical-device permissions, CSS/DRM, mounting, and optional tools remain platform-dependent and are surfaced as warnings or structured errors.

Target-size compression uses two-pass software encoding when a hardware encoder is not selected, and reports the pass strategy in its plan and execution response. Hardware encodes remain single-pass because encoder support varies by platform.

Resize plans preserve aspect ratio and normalize an odd requested width or height to the next even dimension. Post-operation verification checks the effective target dimension. Target-size compression similarly verifies that the rendered file does not exceed the requested byte target.

Safety defaults:

- source files are never modified;
- explicit input/output path collisions are rejected;
- existing outputs receive a `_1`, `_2`, … suffix unless `--overwrite` is explicit;
- successful transforms run a post-operation verification appropriate to the output type.

Execution responses retain the plan's stream choices, reasons, `quality_loss`,
warnings, hardware, subtitle, and metadata decisions alongside the
verification result so an Agent does not need to reconstruct those decisions
from FFmpeg output.

## JSON Tool entrypoint

Agents can send one request object over stdin. The response is always one JSON object on stdout:

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

The Tool entrypoint accepts the semantic operations above plus `operation: "ffmpeg"` for the explicit escape hatch. For integrations that prefer verb-style names, the `*_media` aliases are stable, and image/gif/edit/audio/repair/disc aliases are accepted as well. Request fields include `dry_run`, `overwrite`, `verify_after_execute`, `progress`, quality, hardware, codecs, device presets, format parameters, FPS, subtitle styles, disc actions, filter parameters, and merge inputs. A `plan` request can set `target_operation` explicitly; otherwise the same inference rules as the CLI apply. The full contract is [`schemas/tool-api.json`](schemas/tool-api.json).

```bash
printf '%s\n' '{"operation":"convert","input":"input.mkv","output":"output.mp4","dry_run":true}' \
  | media tool

printf '%s\n' '{"operation":"ffmpeg","args":["-i","input.mp4","-vf","scale=1280:-2","output.mp4"],"dry_run":true}' \
  | media tool
```

## Configuration

MediaForge reads an optional TOML file from `MEDIAFORGE_CONFIG`, `$XDG_CONFIG_HOME/mediaforge/config.toml`, or `~/.config/mediaforge/config.toml`.

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

Configuration applies to both the CLI and Tool entrypoint. It can provide preferred codecs, quality, hardware mode, and progress defaults, but cannot implicitly enable overwrite. `verify_after_execute = false` is available for trusted high-throughput jobs; the response then marks verification as skipped. Start from [`config.example.toml`](config.example.toml).

Hardware encoding is opt-in: `--hardware auto` keeps deterministic CPU encoding, while `--hardware gpu` probes FFmpeg's available hardware encoder for the requested codec. `media capabilities --json` reports both a structured `hardware_acceleration` map and the available encoder lists. If the runtime cannot create a hardware session, MediaForge returns `HARDWARE_UNAVAILABLE` with a CPU fallback suggestion.

## Project map

- [`schemas/tool-api.json`](schemas/tool-api.json) — machine-readable Tool API contract.
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — instructions for an AI agent using the tool.
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml) — OpenAI/Codex discovery metadata for the Agent Skill.
- [`docs/architecture.md`](docs/architecture.md) — control-plane design and invariants.
- [`docs/development.md`](docs/development.md) — local development, testing, and release notes.
- [`docs/MediaForge-PRD-v1.0.md`](docs/MediaForge-PRD-v1.0.md) — the product requirements document supplied for this project.
- [`docs/prd-v1-compliance.md`](docs/prd-v1-compliance.md) — requirement-to-test traceability for the V1 release gate.
- [`docs/index.html`](docs/index.html) — static agent-facing documentation page and API examples.

## Status

MediaForge is an early, working Rust implementation. The current release focuses on deterministic local processing through FFmpeg/FFprobe. Remote storage, model-powered editing decisions, and a long-running job service are intentionally outside this first CLI milestone.

## Contributing and license

Bug reports and focused pull requests are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`SECURITY.md`](SECURITY.md) before opening an issue.

MediaForge is released under the [MIT License](LICENSE).
