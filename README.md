# MediaForge

[![CI](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml/badge.svg)](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

文档网站：[MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/)

MediaForge 是一个面向 AI Agent 的媒体处理工具。它为 [FFmpeg](https://ffmpeg.org/) 和 FFprobe 提供小而确定性的控制层：检查媒体、规划操作、安全执行并验证结果。它是 CLI 和基于 stdin/stdout 的 JSON 工具，不是前端项目或 GUI 应用。

MediaForge is an agent-native media processing toolkit. It gives an AI agent a small, deterministic control plane over FFmpeg and FFprobe: inspect media, plan an operation, execute it safely, and verify the result.

本文档的中文说明优先覆盖安装、Agent 调用、平台支持和项目入口；下面的参数名、JSON 字段和命令示例保持英文，以便直接复制执行。

## Why MediaForge / 为什么选择 MediaForge

FFmpeg is powerful but exposes a large, stateful command surface. MediaForge adds the agent-facing contract around it:

- semantic operations instead of hand-written filter graphs for common jobs;
- stable JSON responses with machine-readable error codes;
- `inspect → plan → execute → verify` workflow;
- safe output naming, collision checks, and no implicit overwrite;
- an explicit Raw FFmpeg escape hatch for advanced cases;
- a versioned Tool API schema and an installable Agent Skill.

简单来说，FFmpeg 负责真正的媒体编解码，MediaForge 负责把它包装成 Agent 更容易理解和调用的可靠执行层：Agent 不必手写复杂 FFmpeg 参数，也能获得可预测的计划、结构化错误和执行后验证。

## Install / 安装

### 普通用户：下载预编译版（推荐）

MediaForge 本身只是一个很小的二进制文件；运行时只依赖 FFmpeg 和 FFprobe。普通用户无需安装 Rust，先安装一次 FFmpeg，再执行对应平台的一键安装命令即可。

MediaForge itself is a small binary; FFmpeg and FFprobe are the only runtime
dependencies. Install FFmpeg once, then use the matching one-line installer.

macOS or Linux:

```bash
# macOS: brew install ffmpeg
# Debian/Ubuntu: sudo apt install ffmpeg
curl -fsSL https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
choco install ffmpeg
irm https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.ps1 | iex
```

安装器会自动选择 macOS Apple Silicon/Intel、Linux x64 或 Windows x64 的最新版本，Unix 安装到 `~/.local/bin`，Windows 安装到 `%LOCALAPPDATA%\\MediaForge\\bin`，并提示 PATH 配置方式。可以通过 `MEDIAFORGE_VERSION=v0.1.0` 固定安装版本。

The installer selects the current release for macOS Apple Silicon/Intel, Linux
x64, or Windows x64, places `media` in `~/.local/bin` (Unix) or
`%LOCALAPPDATA%\\MediaForge\\bin` (Windows), and prints the final `PATH`
hint. To install a specific release, set `MEDIAFORGE_VERSION`, for example
`MEDIAFORGE_VERSION=v0.1.0`.

### From source / 源码安装

开发者或需要使用尚未发布代码时，才需要 Rust/Cargo：

For development or an unreleased checkout, Rust/Cargo is also required:

```bash
cargo install --path . --bin media
```

For a local release build:

```bash
cargo build --release --bin media
./target/release/media capabilities --json
```

## Supported platforms / 支持平台

MediaForge is tested and packaged for macOS, Linux, and Windows x64. Install
FFmpeg and FFprobe on `PATH` before using the CLI. On Windows, the release
binary is `media.exe`; the same semantic commands and JSON Tool API apply.

当前提供 macOS、Linux 和 Windows x64 支持。MediaForge 不捆绑 FFmpeg；请确保 `ffmpeg` 和 `ffprobe` 已经在 `PATH` 中。Windows 发布包中的程序名为 `media.exe`，命令语义和 JSON Tool API 与 Unix 平台一致。

For a source build on Windows:

```powershell
choco install ffmpeg --yes
cargo build --release --bin media
.\target\release\media.exe capabilities --json
```

The Windows acceptance workflow runs through Git Bash and accepts either
`python3` or `python` for its small JSON assertions.

## Agent workflow / Agent 工作流

The normal CLI is useful during development and for shell-based agents:

普通 CLI 适合开发调试，也适合由 Shell Agent 调用。推荐流程是：先 `inspect` 获取媒体信息，再 `plan` 预览决策，之后 `convert` 或其他操作，最后 `verify` 检查结果。

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

All transformation commands accept `--dry-run`. JSON is emitted only on stdout; FFmpeg diagnostics stay on stderr and can be enabled with `--verbose` or `--debug`. Long-running transformations support `--progress`: human CLI mode prints percentage, elapsed time, estimated remaining time, and speed, while `--json`/Tool mode emits the same measurements as progress NDJSON on stderr without breaking the one-response JSON contract.

## Operations / 支持的操作

下面的操作名可以直接用于 CLI，也可以作为 JSON Tool API 的 `operation` 字段。完整字段定义见 [`schemas/tool-api.json`](schemas/tool-api.json)。

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

## JSON Tool entrypoint / JSON Tool 入口

Agents can send one request object over stdin. The response is always one JSON object on stdout:

Agent 可以通过 stdin 发送一个 JSON 请求对象；stdout 始终只返回一个 JSON 对象，FFmpeg 日志和进度写入 stderr，不会污染 Agent 的响应解析。

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

## Configuration / 配置

MediaForge reads an optional TOML file from `MEDIAFORGE_CONFIG`, `$XDG_CONFIG_HOME/mediaforge/config.toml`, or `~/.config/mediaforge/config.toml`.

MediaForge 支持可选的 TOML 配置文件，可通过 `MEDIAFORGE_CONFIG` 指定，也可以放在 `$XDG_CONFIG_HOME/mediaforge/config.toml` 或 `~/.config/mediaforge/config.toml`。配置不会隐式开启覆盖写入。

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

## Project map / 项目结构

- [`schemas/tool-api.json`](schemas/tool-api.json) — machine-readable Tool API contract.
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — instructions for an AI agent using the tool.
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml) — OpenAI/Codex discovery metadata for the Agent Skill.
- [`docs/architecture.md`](docs/architecture.md) — control-plane design and invariants.
- [`docs/development.md`](docs/development.md) — local development, testing, and release notes.
- [`scripts/install.sh`](scripts/install.sh) and [`scripts/install.ps1`](scripts/install.ps1) — prebuilt binary installers for ordinary users.
- [MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/) — deployed static introduction and Agent Tool API guide.
- [`docs/MediaForge-PRD-v1.0.md`](docs/MediaForge-PRD-v1.0.md) — the product requirements document supplied for this project.
- [`docs/prd-v1-compliance.md`](docs/prd-v1-compliance.md) — requirement-to-test traceability for the V1 release gate.
- [`docs/index.html`](docs/index.html) — static agent-facing documentation page and API examples.

如果你要接入 Agent，优先阅读 `skills/mediaforge/SKILL.md` 和 `schemas/tool-api.json`；如果你要参与开发，阅读 `docs/development.md`。

## Status / 项目状态

MediaForge is an early, working Rust implementation. The current release focuses on deterministic local processing through FFmpeg/FFprobe. Remote storage, model-powered editing decisions, and a long-running job service are intentionally outside this first CLI milestone.

这是一个正在持续迭代的 Rust 实现。当前版本聚焦于本地、确定性的媒体处理；远程存储、模型驱动的剪辑决策和长期运行的任务服务暂不属于首个 CLI 里程碑。

## Contributing and license / 贡献与许可证

Bug reports and focused pull requests are welcome. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`SECURITY.md`](SECURITY.md) before opening an issue.

欢迎提交 Bug 和聚焦明确的 Pull Request。提交 Issue 前请先阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 和 [`SECURITY.md`](SECURITY.md)。

MediaForge is released under the [MIT License](LICENSE).
