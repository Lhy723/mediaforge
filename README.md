# MediaForge

[![CI](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml/badge.svg)](https://github.com/Lhy723/mediaforge/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English version](README.en.md) · [文档网站](https://lhy723.github.io/mediaforge/)

MediaForge 是一个面向 AI Agent 的本地媒体处理工具。它为
[FFmpeg](https://ffmpeg.org/) 和 FFprobe 提供小而确定性的控制层：检查媒体、规划操作、安全执行并验证结果。
它同时提供命令行接口（CLI）和基于 stdin/stdout 的 JSON Tool 接口，不是前端项目或 GUI 应用。

## 核心特性

- 使用语义化操作替代常见任务中的手写 FFmpeg filter graph。
- 稳定的 JSON 响应和机器可读错误码。
- 标准工作流：`inspect → plan → execute → verify`。
- 安全的输出命名、路径冲突检查和显式覆盖控制。
- 长任务进度输出，不污染机器可读的 stdout。
- 为高级场景保留 Raw FFmpeg 逃生口。
- 提供版本化 Tool API Schema 和可安装的 Agent Skill。

FFmpeg 负责真正的媒体编解码，MediaForge 负责把它包装成 Agent 更容易理解和调用的可靠执行层。
Agent 不必手写复杂 FFmpeg 参数，也能获得可预测的计划、结构化错误和执行后验证。

## 安装

MediaForge 运行时依赖 FFmpeg 和 FFprobe。普通用户无需安装 Rust，先安装 FFmpeg，再执行对应平台的一键安装命令。

### macOS / Linux（预编译版）

```bash
# macOS
brew install ffmpeg

# Debian/Ubuntu
sudo apt install ffmpeg

# 安装 MediaForge
curl -fsSL https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.sh | sh
```

### Windows PowerShell（预编译版）

```powershell
choco install ffmpeg
irm https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.ps1 | iex
```

安装器会自动选择 macOS Apple Silicon/Intel、Linux x64 或 Windows x64 的最新版本，
Unix 安装到 `~/.local/bin`，Windows 安装到 `%LOCALAPPDATA%\\MediaForge\\bin`，并提示 PATH 配置方式。
可以通过 `MEDIAFORGE_VERSION=v0.1.0` 固定安装版本。

### 从源码安装

开发者或需要使用尚未发布代码时，可以使用 Rust/Cargo：

```bash
cargo install --path . --bin media

# 本地 release 构建
cargo build --release --bin media
./target/release/media capabilities --json
```

## 支持平台

当前测试和发布平台为 macOS、Linux 和 Windows x64。MediaForge 不捆绑 FFmpeg，
请确保 `ffmpeg` 和 `ffprobe` 已经在 `PATH` 中。Windows 发布包中的程序名为 `media.exe`，
命令语义和 JSON Tool API 与 Unix 平台一致。

## Agent 工作流

推荐流程是先检查媒体，再预览计划，执行操作，最后验证结果：

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

所有转换命令都支持 `--dry-run`。`--json` 只向 stdout 输出 JSON；FFmpeg 诊断信息写入 stderr。
长任务使用 `--progress` 时，普通 CLI 输出百分比、耗时、预计剩余时间和速度；JSON/Tool 模式在 stderr 输出进度 NDJSON，最终 stdout 仍只有一个 JSON 响应。

## 支持的操作

| 命令 | 功能 |
| --- | --- |
| `inspect` | 返回容器、文件大小、时长、码率、标签，以及视频/音频/字幕流的结构化信息。 |
| `plan` | 只生成计划，不写文件；报告复制、封装或转码策略、编码器、硬件、质量损失、字幕/元数据处理和警告。 |
| `convert` | 容器和编解码转换；自动选择 stream copy、remux 或转码；支持设备预设。 |
| `compress` | 按 `lossless`、`very-high`、`high`、`balanced`、`small`、`tiny` 压缩，也支持目标大小。 |
| `resize` | 按宽度或 `1080p` 等分辨率缩放，保持比例并调整为偶数尺寸。 |
| `clip` | 按开始时间加时长或结束时间剪辑；可在兼容场景下无损复制。 |
| `extract-audio` | 从视频提取音频；兼容编码优先复制，否则转码。 |
| `thumbnail` | 在秒数、时间码或百分比位置提取 JPEG 缩略图。 |
| `image` | 图片格式转换、缩放、旋转、水印和质量控制。 |
| `gif` | 视频转调色板优化的动画 GIF，支持起始时间、时长、FPS 和宽度。 |
| `edit` | 裁剪、旋转、倍速、音量、灰度/模糊/锐化/复古滤镜、外挂字幕烧录和时间范围。 |
| `merge` | `concat` 拼接、`mux` 视频加音频、`mix` 音频混音。 |
| `audio` | 音频格式转换，以及码率、采样率、声道、音量和时间范围处理。 |
| `repair` | 容错处理时间戳和损坏帧；可选择 H.264/AAC 重新编码。 |
| `disc` | DVD/CD/ISO 提取；也可从目录创建 ISO 镜像。 |
| `batch` | 对文件、目录或 glob 批量转换，支持递归、输出目录和部分成功结果。 |
| `verify` | 检查输出可解析性、大小、时长、流、分辨率、编码和 FFmpeg 解码抽样结果。 |
| `capabilities` | 检测 FFmpeg 版本、硬件加速、编码器、格式、滤镜、外部工具和设备预设。 |
| `presets` | 输出 iPhone、iPad、Android、PSP、车载播放器等确定性设备配置。 |
| `tool` | 通过 stdin 或 `--request` 接收一个 JSON 请求并返回一个 JSON 响应。 |
| `ffmpeg` | 透传原生 FFmpeg 参数，用于语义命令未覆盖的高级场景。 |

常用示例：

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

## 格式、编码和硬件

支持的容器包括 `mp4`、`mkv`、`mov`、`webm`、`avi`、`wmv/asf`、`flv`、`ogv`、`3gp`、
`mpg/mpeg`、`vob` 和 `swf`。图片支持 PNG、JPEG、WebP、GIF、BMP、TIFF、ICO、TGA 和 AVIF；
音频支持 MP3、AAC/M4A、FLAC、WAV、Opus、OGG/Vorbis、WMA、AIFF、ALAC、AMR、AC-3 和 MP2。

视频编码支持 H.264、H.265/HEVC、VP9、AV1、MPEG-4、MPEG-2、FLV1、WMV2 和 Theora，
并提供 `auto` 与 `copy`。默认组合为 MP4/MOV 使用 H.264/AAC，WebM 使用 VP9/Opus，
OGV 使用 Theora/Opus，WMV 使用 WMV2/WMA，MPEG/VOB 使用 MPEG-2/MP2。

硬件模式为 `auto`、`cpu` 和 `gpu`，可检测 VideoToolbox、NVENC、QSV、VAAPI、AMF。
默认 `auto` 使用确定性的 CPU 编码；`gpu` 会探测当前 FFmpeg 可用的硬件编码器。
运行 `media capabilities --json` 查看当前环境的真实能力，编码器是否存在取决于 FFmpeg 构建版本。

目标大小压缩在未选择硬件编码时通常使用两遍软件编码，并在验证中检查文件是否超过目标大小。
GIF 的 FPS 范围为 1–60，时长最多 600 秒，宽度最多 16384；编辑倍速范围为 0.25–4，音量范围为 0–10。

## JSON Tool API 和 Agent Skill

Agent 可以通过 `media tool` 使用一个请求/一个响应的 stdin/stdout 协议：

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

Tool API 支持语义操作、稳定别名、`dry_run`、`overwrite`、`verify_after_execute`、`progress`、
设备预设、图像/视频/音频参数以及 `operation: "ffmpeg"` 原生参数入口。完整字段定义见
[`schemas/tool-api.json`](schemas/tool-api.json)。

仓库包含可安装的 Agent Skill：

- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md)：工作流、安全规则、调用示例和错误处理。
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml)：Agent 发现元数据和默认提示词。

当前版本提供 stdio JSON Tool 和 Skill，但没有原生 MCP Server。MCP 客户端可以把 `media tool` 作为子进程包装使用；如需直接配置 MCP，需要额外的 MCP 适配层。

## 配置

可通过 `MEDIAFORGE_CONFIG` 指定 TOML 配置，也可以使用 `$XDG_CONFIG_HOME/mediaforge/config.toml`
或 `~/.config/mediaforge/config.toml`。配置同时作用于 CLI 和 Tool API：

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

配置可以设置默认质量、硬件模式、验证、进度和首选编码器，但不能隐式开启覆盖写入。
可信的高吞吐任务可以设置 `verify_after_execute = false`，响应会明确标记验证被跳过。

## 安全和错误处理

- 源文件不会被修改。
- 输入和输出路径相同会被拒绝。
- 已存在的输出默认使用 `_1`、`_2` 等后缀，只有显式 `--overwrite` 才会覆盖。
- 转换成功后执行与操作类型匹配的验证。
- JSON 错误包含 `status`、`code`、`message`、`details` 和 `suggestions`。
- 常见错误码包括 `FILE_NOT_FOUND`、`INVALID_MEDIA`、`UNSUPPORTED_FORMAT`、`UNSUPPORTED_CODEC`、
  `ENCODER_UNAVAILABLE`、`HARDWARE_UNAVAILABLE`、`FFMPEG_NOT_FOUND`、`FFMPEG_FAILED` 和 `VERIFY_FAILED`。
- DVD/CD 设备、字幕烧录和 ISO 创建依赖操作系统权限、FFmpeg 构建以及可选工具；相关限制会以警告或结构化错误返回。

## 项目入口、状态和许可证

- [`schemas/tool-api.json`](schemas/tool-api.json)：机器可读的 Tool API 合约。
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md)：Agent 使用说明。
- [`docs/architecture.md`](docs/architecture.md)：控制层设计和不变量。
- [`docs/development.md`](docs/development.md)：开发、测试和发布流程。
- [`scripts/install.sh`](scripts/install.sh) / [`scripts/install.ps1`](scripts/install.ps1)：预编译版安装器。
- [MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/)：静态介绍和 Agent API 指南。

MediaForge 当前是一个持续迭代中的 Rust 实现，聚焦本地、确定性的 FFmpeg/FFprobe 处理。
远程存储、模型驱动的剪辑决策和长期运行的任务服务不属于当前 CLI 版本范围。

欢迎提交 Bug 和聚焦明确的 Pull Request。项目采用 [MIT License](LICENSE)。
