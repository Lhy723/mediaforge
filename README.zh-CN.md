<a id="readme-top"></a>

<div align="right">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md"><strong>简体中文</strong></a>
</div>

<div align="center">
  <h1>MediaForge</h1>
  <p>
    <strong>面向 AI Agent 的确定性媒体处理工具</strong><br />
    <sub>检查 → 规划 → 执行 → 验证，由 FFmpeg 驱动。</sub>
  </p>
  <p>
    <img src="https://img.shields.io/github/actions/workflow/status/Lhy723/mediaforge/ci.yml?style=flat&label=CI" alt="CI" />
    <img src="https://img.shields.io/badge/License-MIT-89B4FA?style=flat&logo=opensourceinitiative&logoColor=white" alt="许可证：MIT" />
    <img src="https://img.shields.io/github/stars/Lhy723/mediaforge?style=flat&color=F5C2E7&label=stars" alt="Stars" />
    <img src="https://img.shields.io/badge/CLI-media-A6E3A1?style=flat&logo=gnu-bash&logoColor=black" alt="CLI：media" />
    <img src="https://img.shields.io/badge/OS-macOS%20%7C%20Linux%20%7C%20Windows-1793D1?style=flat" alt="支持平台" />
    <img src="https://img.shields.io/badge/Agent-Tool%20%2B%20Skill-F5C2E7?style=flat" alt="Agent Tool 与 Skill" />
  </p>
  <p><code>inspect</code> · <code>plan</code> · <code>execute</code> · <code>verify</code></p>
  <p>
    <a href="https://lhy723.github.io/mediaforge/">文档网站</a>
    ·
    <a href="https://github.com/Lhy723/mediaforge/issues">问题反馈</a>
  </p>
  <img src="./docs/assets/mediaforge-hero-minimal.png" alt="MediaForge 媒体品牌视觉" width="92%" />
</div>

MediaForge 是一个运行在本地的 [FFmpeg](https://ffmpeg.org/) / FFprobe 控制层。
它把媒体意图转换成可检查的计划，使用安全默认值执行，并验证最终结果。
项目同时提供 CLI 和 stdin/stdout JSON Tool 接口；它不是 GUI、远程存储服务或长期运行的任务服务器。

## 一览

| 本地优先 | Agent 友好 | 能力感知 |
| --- | --- | --- |
| 在本机通过 FFmpeg/FFprobe 处理媒体。 | 一个 JSON 请求对应一个 JSON 响应。 | 运行时探测编码器、硬件、滤镜和可选工具。 |

## 核心特性

- **检查** — 返回规范化的容器、流、编码、HDR、字幕、元数据和时长信息。
- **规划** — 执行前解释复制、封装或转码策略、质量损失、输出路径、警告和 FFmpeg 参数。
- **处理** — 支持转换、压缩、缩放、剪辑、提取音频、缩略图/GIF、编辑、合并、修复和光盘操作。
- **验证** — 检查可解析性、大小、时长、流、尺寸、编码、解码抽样以及操作专属条件。
- **Agent 接口** — 提供稳定的 Tool API Schema、操作别名、结构化错误、进度事件和 Agent Skill。
- **默认安全** — 不修改源文件，拒绝路径冲突，不隐式覆盖，并报告每个决策。
- **原生逃生口** — 对语义 API 未覆盖的任务透传显式 FFmpeg 参数向量。

## 安装

MediaForge 运行时需要 `ffmpeg` 和 `ffprobe` 位于 `PATH` 中；预编译安装器不会捆绑这两个依赖。

### macOS / Linux（预编译版）

```bash
# 先安装运行时依赖（二选一）
brew install ffmpeg                 # macOS
sudo apt install ffmpeg             # Debian/Ubuntu

# 安装最新 MediaForge 二进制
curl -fsSL https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.sh | sh
```

安装器会自动选择 macOS Apple Silicon/Intel 或 Linux x64，将 `media` 安装到 `~/.local/bin` 并提示 PATH 配置。
可以通过 `MEDIAFORGE_VERSION=v0.1.0` 固定版本。

### Windows PowerShell（预编译版）

```powershell
choco install ffmpeg
irm https://raw.githubusercontent.com/Lhy723/mediaforge/main/scripts/install.ps1 | iex
```

Windows 程序名为 `media.exe`，默认安装到 `%LOCALAPPDATA%\\MediaForge\\bin`。

MediaForge 当前测试和发布平台为 macOS、Linux 和 Windows x64。FFmpeg 和 FFprobe 不随程序捆绑，
请确保两个命令都位于 `PATH` 中。

### 从源码安装

```bash
cargo install --path . --bin media

# 或构建本地 release
cargo build --release --bin media
./target/release/media capabilities --json
```

## 快速开始

推荐工作流是 `inspect → plan → execute → verify`：

```bash
media inspect input.mkv --json
media plan input.mkv --to mp4 --json
media convert input.mkv --to mp4 --json
media verify input.mkv output.mp4 --json
```

任何转换都可以先使用 `--dry-run` 预览。`--progress` 会显示百分比、耗时、预计剩余时间和速度。
JSON/Tool 模式保持 stdout 可解析，并将进度 NDJSON 写入 stderr。

常用全局参数包括 `--json`、`--dry-run`、`--overwrite`、`--verbose`、`--debug` 和 `--progress`。

## 支持的操作

### 媒体处理

| 命令 | 功能 |
| --- | --- |
| `inspect` | 返回文件、容器、视频、音频、字幕和元数据的规范化信息。 |
| `plan` | 生成不写文件的计划，包含策略、编码器、硬件、质量损失、警告和 FFmpeg 参数。 |
| `convert` | 转换容器和编码，可使用 iPhone、iPad、Android、PSP 或车载预设。 |
| `compress` | 使用质量预设或目标大小压缩；软件目标大小任务通常使用两遍编码。 |
| `resize` | 按宽度或 `1080p` 等高度缩放，保持比例并使用偶数尺寸。 |
| `clip` | 按开始时间加时长/结束时间剪辑；兼容时使用 stream copy。 |
| `extract-audio` | 提取音频，兼容时优先复制源编码，否则转码。 |
| `thumbnail` | 在秒数、`HH:MM:SS` 或百分比位置提取 JPEG。 |
| `image` | 转换、缩放、旋转、添加水印并控制图片质量。 |
| `gif` | 从视频生成调色板优化的动画 GIF。 |

### 编辑和合成

| 命令 | 功能 |
| --- | --- |
| `edit` | 支持裁剪（`WIDTH:HEIGHT:X:Y`）、旋转、倍速 `0.25–4`、音量 `0–10`、命名滤镜、字幕烧录、ASS/SSA 样式和时间范围。 |
| `merge` | `concat` 拼接、`mux` 合并视频与音频、`mix` 混合两条音频。 |
| `audio` | 转换音频并设置格式、码率、采样率、声道、音量和时间范围。 |
| `repair` | 容错修复时间戳/损坏帧，或选择 H.264/AAC 重新编码。 |
| `disc` | 提取 DVD/CD/ISO，或使用可用工具从目录创建 ISO。 |

### 自动化和接口

| 命令 | 功能 |
| --- | --- |
| `batch` | 递归转换文件、目录或 glob，并报告部分成功；当前使用 `--convert FORMAT`。 |
| `verify` | 验证输入/输出对，包括时长、大小、尺寸等操作专属检查。 |
| `capabilities` | 报告 FFmpeg 版本、编码器、硬件加速、格式、滤镜、设备和外部工具。 |
| `presets` | 列出 iPhone、iPad、Android、PSP 和车载播放器预设。 |
| `tool` | 从 stdin 或 `--request` 读取一个 JSON 请求并返回一个 JSON 响应。 |
| `ffmpeg` | 透传原生 FFmpeg 参数，用于高级场景。 |

## 格式矩阵

| 媒体 | 支持格式 |
| --- | --- |
| 容器 | MP4、MKV/Matroska、MOV/QuickTime、WebM、AVI、WMV/ASF、FLV、OGV、3GP、MPG/MPEG、VOB、SWF |
| 图片 | PNG、JPEG（`jpg`/`jpeg`）、WebP、GIF、BMP、TIFF（`tif`/`tiff`）、ICO、TGA、AVIF |
| 音频 | MP3、AAC/M4A、FLAC、WAV、Opus、OGG/Vorbis、WMA、AIFF、ALAC、AMR、AC-3、MP2 |
| 视频编码 | H.264、H.265/HEVC、VP9、AV1、MPEG-4、MPEG-2、FLV1、WMV2、Theora、`auto`、`copy` |

容器默认组合为：MP4/MOV 使用 H.264/AAC，WebM 使用 VP9/Opus，OGV 使用 Theora/Opus，
WMV 使用 WMV2/WMA，MPEG/VOB 使用 MPEG-2/MP2。实际编码器是否可用取决于本机 FFmpeg 构建。

## 面向 AI Agent

### JSON Tool

宿主可以通过 stdio 调用 Tool：

```bash
printf '%s\n' '{"operation":"plan","input":"input.mkv","output_format":"mp4"}' \
  | media tool
```

Tool API 支持语义操作和稳定别名，例如 `inspect_media`、`plan_media_operation`、`convert_media`、
`create_thumbnail`、`image_convert`、`video_to_gif`、`edit_media`、`audio_convert`、`repair_media`、
`verify_media` 和 `device_presets`。请求还可以包含 `dry_run`、`overwrite`、`verify_after_execute`、
`progress`、编解码器、质量、硬件、设备预设、图像/编辑/音频参数、光盘操作、合并输入以及原生 FFmpeg 参数。

完整合约见 [`schemas/tool-api.json`](schemas/tool-api.json)。JSON 错误统一包含 `status`、`code`、`message`、`details` 和 `suggestions`。

### Agent Skill

仓库包含适用于支持本地 Skill 的 Agent 宿主的安装包：

- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — 工作流、安全规则、示例和错误处理。
- [`skills/mediaforge/agents/openai.yaml`](skills/mediaforge/agents/openai.yaml) — 发现元数据和默认提示词。

当前版本提供 stdio JSON Tool 和 Skill，但没有原生 MCP Server。MCP 客户端可以把 `media tool` 包装成子进程；直接 MCP 配置需要额外适配器。

## 默认安全和可观测性

- 源文件不会被修改。
- 输入/输出路径相同会被拒绝。
- 已存在的输出默认使用 `_1`、`_2` 等后缀，只有显式 `--overwrite` 才会覆盖。
- 转换成功后执行对应的结果验证；可信任务可以显式关闭验证。
- `--verbose` 和 `--debug` 将诊断信息写入 stderr。
- `--progress` 将人类可读进度或 NDJSON 事件写入 stderr，不影响 stdout 解析。
- DVD/CD、字幕烧录、ISO 创建和硬件编码依赖运行环境，并通过警告/结构化错误说明限制。

## 配置

可通过 `MEDIAFORGE_CONFIG`、`$XDG_CONFIG_HOME/mediaforge/config.toml` 或 `~/.config/mediaforge/config.toml` 加载 TOML 默认值：

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

配置同时作用于 CLI 和 Tool API，但不能隐式开启覆盖写入。

## 项目入口

- [`schemas/tool-api.json`](schemas/tool-api.json) — 机器可读的 Tool API 合约。
- [`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) — Agent 使用说明。
- [`docs/architecture.md`](docs/architecture.md) — 控制层设计和不变量。
- [`docs/development.md`](docs/development.md) — 开发、测试和发布流程。
- [`scripts/install.sh`](scripts/install.sh) / [`scripts/install.ps1`](scripts/install.ps1) — 预编译版安装器。
- [MediaForge GitHub Pages](https://lhy723.github.io/mediaforge/) — 静态介绍和 Agent API 指南。

## 状态和许可证

MediaForge 是一个持续迭代中的 Rust 实现，当前聚焦本地、确定性的 FFmpeg/FFprobe 处理。
远程存储、模型驱动的剪辑决策和长期运行任务服务不在当前 CLI 范围内。

欢迎提交 Bug 和聚焦明确的 Pull Request。项目采用 [MIT License](LICENSE)。
