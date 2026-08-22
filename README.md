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

## Agent workflow

The normal CLI is useful during development and for shell-based agents:

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

All transformation commands accept `--dry-run`. JSON is emitted only on stdout; FFmpeg diagnostics stay on stderr and can be enabled with `--verbose`.

## Operations

```text
inspect, plan, convert, compress, resize, clip,
extract-audio, thumbnail, batch, verify, capabilities
```

Examples:

```bash
media compress video.mp4 --quality balanced --json
media resize video.mp4 --resolution 1080p --dry-run --json
media clip video.mp4 --start 00:10:00 --duration 30 --json
media extract-audio video.mp4 --format flac --json
media thumbnail video.mp4 --at 50% --json
media batch './videos/*.mov' --convert mp4 --json
```

Safety defaults:

- source files are never modified;
- explicit input/output path collisions are rejected;
- existing outputs receive a `_1`, `_2`, … suffix unless `--overwrite` is explicit;
- successful transforms run a post-operation verification appropriate to the output type.

## JSON Tool entrypoint

Agents can send one request object over stdin. The response is always one JSON object on stdout:

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

The Tool entrypoint accepts the semantic operations above plus `operation: "ffmpeg"` for the explicit escape hatch. For integrations that prefer verb-style names, `inspect_media`, `plan_media_operation`, `convert_media`, `compress_media`, `resize_media`, `clip_media`, `create_thumbnail`, and `verify_media` are stable aliases. Request fields include `dry_run`, `overwrite`, `verify_after_execute`, quality, hardware, codecs, and operation-specific arguments. The full contract is [`schemas/tool-api.json`](schemas/tool-api.json).

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

[video]
preferred_codec = "auto"

[audio]
preferred_codec = "aac"
```

Configuration can provide preferred codecs and quality, but cannot implicitly enable overwrite. `verify_after_execute = false` is available for trusted high-throughput jobs; the response then marks verification as skipped. Start from [`config.example.toml`](config.example.toml).

## Project map

- [`schemas/tool-api.json`](schemas/tool-api.json) — machine-readable Tool API contract.
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — instructions for an AI agent using the tool.
- [`docs/architecture.md`](docs/architecture.md) — control-plane design and invariants.
- [`docs/development.md`](docs/development.md) — local development, testing, and release notes.
- [`docs/MediaForge-PRD-v1.0.md`](docs/MediaForge-PRD-v1.0.md) — the product requirements document supplied for this project.

## Status

MediaForge is an early, working Rust implementation. The current release focuses on deterministic local processing through FFmpeg/FFprobe. Remote storage, model-powered editing decisions, and a long-running job service are intentionally outside this first CLI milestone.

## Contributing and license

Bug reports and focused pull requests are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`SECURITY.md`](SECURITY.md) before opening an issue.

MediaForge is released under the [MIT License](LICENSE).
