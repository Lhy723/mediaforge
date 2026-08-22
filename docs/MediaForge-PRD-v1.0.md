# MediaForge 产品需求文档（PRD）

**文档版本：** V1.0
**产品名称：** MediaForge
**产品定位：** 面向 AI Agent 的确定性媒体处理工具
**目标形态：** CLI + Skill / Tool API
**核心引擎：** FFmpeg + FFprobe
**目标平台：** macOS、Linux，后续支持 Windows
**目标用户：** AI Agent、开发者、自动化工作流构建者

---

## 1. 产品背景

FFmpeg 是目前最成熟、能力最完整的音视频处理基础设施之一，但它本质上是一个面向专业用户设计的底层命令行工具。

对于 AI Agent 而言，直接操作 FFmpeg 存在明显问题：

1. 参数数量庞大，组合复杂。
2. 参数顺序具有语义，容易生成错误命令。
3. 容器、视频编码、音频编码之间存在兼容性问题。
4. Agent 很难判断某次转换是否需要重新编码。
5. 缺乏稳定、统一的 JSON 输出。
6. FFmpeg 日志对人类友好，但不适合 Agent 解析。
7. Agent 很容易误覆盖源文件。
8. 很难在执行前判断操作成本、画质损失和潜在风险。
9. 执行成功并不代表输出文件一定满足用户要求。
10. 不同平台的硬件编码器、FFmpeg 编译配置存在差异。

例如用户提出：

> 把这个 MKV 转成 MP4。

Agent 如果直接使用 FFmpeg，可能生成：

```bash
ffmpeg -i input.mkv -c:v libx264 -c:a aac output.mp4
```

但原文件可能本身已经是 H.264 + AAC，只需要更换封装：

```bash
ffmpeg -i input.mkv -c copy output.mp4
```

前一种方式可能需要几十分钟，并造成不必要的画质损失；后一种方式可能几秒即可完成且完全无损。

因此需要在 FFmpeg 与 AI Agent 之间增加一个确定性中间层。

---

# 2. 产品愿景

MediaForge 希望成为：

> **A deterministic media toolkit designed for AI agents.**

即：

**一个专门为 AI Agent 设计的、稳定、安全、结构化的媒体处理基础设施。**

MediaForge 不试图替代 FFmpeg，而是将 FFmpeg 的底层能力抽象为适合 Agent 理解和调用的高级语义接口。

核心理念：

```text
Understand
   ↓
Inspect
   ↓
Plan
   ↓
Execute
   ↓
Verify
```

其中最重要的工作流为：

```text
inspect → plan → execute → verify
```

---

# 3. 产品目标

## 3.1 V1 核心目标

V1 重点解决以下问题：

* Agent 可以可靠获取媒体文件信息。
* Agent 不需要掌握复杂 FFmpeg 参数。
* 转换前可以获得执行计划。
* 自动判断 Remux 与 Transcode。
* 默认避免不必要的重新编码。
* 默认避免覆盖源文件。
* 提供稳定的 JSON 输入输出。
* 自动选择合理编码参数。
* 支持常见音视频操作。
* 操作结束后自动验证输出文件。
* 支持单文件与批量处理。
* 支持 dry-run。
* 支持 AI Skill 集成。

---

## 3.2 非目标

V1 暂不追求：

* 完整封装 FFmpeg 所有功能。
* 专业 NLE 视频剪辑能力。
* 复杂时间线编辑。
* After Effects 类特效系统。
* 视频内容理解。
* AI 视频生成。
* 自动字幕识别。
* 在线视频下载。
* DRM 内容处理。
* GUI 桌面客户端。

这些能力可在后续版本逐步扩展。

---

# 4. 目标用户

## 4.1 AI Agent

例如：

* ChatGPT
* Codex
* Claude Code
* OpenCode
* DeepSeek Harness
* 自定义 Agent
* MCP Agent
* Workflow Agent

主要需求：

> 我需要一个可以直接调用、输出稳定、失败原因明确、不容易破坏文件的媒体工具。

---

## 4.2 开发者

希望使用简单命令完成媒体操作：

```bash
media convert input.mkv --to mp4
```

而非手写：

```bash
ffmpeg -i input.mkv ...
```

---

## 4.3 自动化工作流

例如：

```text
上传视频
↓
自动检查
↓
压缩
↓
生成缩略图
↓
提取音轨
↓
检查结果
↓
上传 CDN
```

---

# 5. 核心设计原则

## 5.1 Semantic over Flags

Agent 应描述：

```text
我要压缩这个视频
```

而不是：

```text
我要使用 libx265 + CRF 25 + preset medium
```

因此提供：

```bash
media compress input.mp4 --quality balanced
```

而非要求 Agent直接构造底层 FFmpeg 参数。

---

## 5.2 Safe by Default

默认行为必须安全：

* 不覆盖输入文件。
* 不覆盖已有输出文件。
* 不删除源文件。
* 不改变源文件。
* 自动产生安全输出路径。
* 高风险行为必须显式声明。

例如：

```text
video.mp4
```

如果输出文件已存在：

```text
video_1.mp4
video_2.mp4
```

而非直接覆盖。

---

## 5.3 Lossless When Possible

在满足目标格式的情况下：

> 尽可能避免重新编码。

优先级：

```text
Remux
↓
Copy compatible streams
↓
Transcode incompatible streams
↓
Full transcode
```

---

## 5.4 Structured by Default

所有 Agent-facing 命令必须支持：

```bash
--json
```

并输出稳定的数据结构。

---

## 5.5 Explainable Operations

执行计划必须能够解释：

* 为什么需要重新编码。
* 哪些流会被复制。
* 哪些流会被转换。
* 是否存在画质损失。
* 是否删除或丢弃轨道。
* 是否使用硬件加速。

---

## 5.6 Verifiable Output

不能只依赖 FFmpeg：

```text
exit code = 0
```

判断成功。

完成后必须验证实际输出。

---

# 6. V1 功能范围

V1 提供以下核心命令：

```text
media inspect
media plan
media convert
media compress
media resize
media clip
media extract-audio
media thumbnail
media batch
media verify
```

同时所有适用命令支持：

```text
--json
--dry-run
```

---

# 7. 功能需求

## 7.1 Inspect

### 目标

读取媒体文件并返回完整、标准化的结构化信息。

### CLI

```bash
media inspect input.mkv
```

Agent 模式：

```bash
media inspect input.mkv --json
```

### 应返回

文件级信息：

* 文件路径
* 文件大小
* Container
* Duration
* Bitrate
* Metadata

视频流：

* Codec
* Profile
* Resolution
* FPS
* Pixel Format
* Bit Depth
* HDR
* Bitrate
* Language
* Default stream

音频流：

* Codec
* Sample Rate
* Channel Count
* Channel Layout
* Bitrate
* Language

字幕流：

* Codec
* Language
* Forced
* Default

### 示例

```json
{
  "status": "success",
  "file": {
    "path": "/media/movie.mkv",
    "size_bytes": 8358219231,
    "container": "matroska",
    "duration_seconds": 7261.4
  },
  "video": [
    {
      "index": 0,
      "codec": "hevc",
      "width": 3840,
      "height": 2160,
      "fps": 23.976,
      "bit_depth": 10,
      "hdr": "HDR10"
    }
  ],
  "audio": [
    {
      "index": 1,
      "codec": "truehd",
      "channels": 8,
      "language": "jpn"
    }
  ],
  "subtitle": []
}
```

---

# 7.2 Plan

## 目标

在不实际执行转换的情况下，分析目标并产生执行方案。

### CLI

```bash
media plan input.mkv --to mp4
```

### 输出信息

必须包含：

* Operation 类型
* Remux / Transcode 判断
* Video strategy
* Audio strategy
* Subtitle strategy
* Metadata strategy
* Hardware strategy
* 画质损失情况
* 警告
* 输出路径

### 示例

```json
{
  "status": "success",
  "operation": "convert",
  "strategy": "partial_transcode",
  "output": "input.mp4",
  "video": {
    "action": "copy",
    "codec": "hevc"
  },
  "audio": {
    "action": "transcode",
    "from": "truehd",
    "to": "aac"
  },
  "quality_loss": "audio_only",
  "reason": [
    "HEVC video is compatible with MP4.",
    "TrueHD audio is not compatible with the selected compatibility profile."
  ]
}
```

---

# 7.3 Convert

## 目标

在媒体容器或编码格式之间进行智能转换。

### CLI

```bash
media convert input.mkv --to mp4
```

指定编码：

```bash
media convert input.mkv \
  --to mp4 \
  --video-codec h265 \
  --audio-codec aac
```

### 自动策略

如果：

```text
输入：
MKV
H264
AAC

目标：
MP4
```

应该：

```text
Video → copy
Audio → copy
Container → MP4
```

而不是重新编码。

---

# 7.4 Compress

## 目标

降低文件体积。

### CLI

```bash
media compress input.mp4 --quality balanced
```

支持：

```text
lossless
very-high
high
balanced
small
tiny
```

同时支持目标体积：

```bash
media compress input.mp4 --target-size 500MB
```

### 系统负责自动决定

* codec
* CRF
* bitrate
* audio bitrate
* preset
* two-pass

---

# 7.5 Resize

## CLI

```bash
media resize input.mp4 --width 1920
```

或者：

```bash
media resize input.mp4 --resolution 1080p
```

支持：

```text
2160p
1440p
1080p
720p
480p
```

必须：

* 保持宽高比。
* 自动保证编码需要的偶数尺寸。
* 避免默认拉伸。

---

# 7.6 Clip

## CLI

```bash
media clip input.mp4 \
  --start 00:10:00 \
  --duration 30
```

或者：

```bash
media clip input.mp4 \
  --start 10 \
  --end 40
```

系统应该根据情况选择：

```text
stream copy
```

或者：

```text
precise re-encode
```

---

# 7.7 Extract Audio

## CLI

```bash
media extract-audio input.mp4
```

指定：

```bash
media extract-audio input.mp4 --format mp3
```

支持：

```text
mp3
aac
m4a
flac
wav
opus
```

如果原始编码已经符合目标，应优先直接复制。

---

# 7.8 Thumbnail

## CLI

```bash
media thumbnail input.mp4 --at 00:01:30
```

支持：

```bash
media thumbnail input.mp4 --at 50%
```

后续可支持：

```bash
media thumbnail input.mp4 --auto
```

自动寻找代表性帧。

---

# 7.9 Batch

## 目标

批量执行媒体任务。

### CLI

```bash
media batch "./videos/*.mov" \
  --convert mp4
```

递归：

```bash
media batch ./videos \
  --recursive \
  --convert mp4
```

输出：

```json
{
  "status": "partial_success",
  "total": 100,
  "success": 98,
  "failed": 2,
  "results": []
}
```

单个文件失败不应默认终止整个批处理。

---

# 7.10 Verify

## 目标

检查媒体处理结果是否满足预期。

### CLI

```bash
media verify input.mp4 output.mp4
```

检查：

* 文件是否存在。
* 是否可以正常解析。
* Duration 是否匹配。
* 视频轨是否存在。
* 音频轨是否存在。
* Resolution 是否符合预期。
* Codec 是否符合预期。
* Streams 是否意外丢失。
* 文件大小。
* FFmpeg decode error。

示例：

```json
{
  "status": "success",
  "valid": true,
  "checks": {
    "readable": true,
    "duration_match": true,
    "video_present": true,
    "audio_present": true,
    "resolution_match": true
  },
  "warnings": []
}
```

---

# 8. Dry Run

所有转换操作应支持：

```bash
--dry-run
```

例如：

```bash
media convert input.mkv --to mp4 --dry-run --json
```

返回：

```json
{
  "status": "planned",
  "will_execute": false,
  "input": "input.mkv",
  "output": "input.mp4",
  "strategy": "remux",
  "video": "copy",
  "audio": "copy"
}
```

---

# 9. Agent Mode

Agent 调用时应统一使用：

```bash
--json
```

Agent 只依赖：

```text
stdout → JSON
```

调试日志应发送至：

```text
stderr
```

保证 stdout 永远可被 JSON Parser 直接解析。

---

# 10. 错误系统

错误必须结构化。

例如：

```json
{
  "status": "error",
  "code": "ENCODER_UNAVAILABLE",
  "message": "AV1 encoder is unavailable.",
  "details": {
    "requested_encoder": "av1"
  },
  "suggestions": [
    "Use h265 instead.",
    "Install an FFmpeg build containing an AV1 encoder."
  ]
}
```

建议错误 Code：

```text
FILE_NOT_FOUND
INVALID_MEDIA
INVALID_ARGUMENT
UNSUPPORTED_FORMAT
UNSUPPORTED_CODEC
ENCODER_UNAVAILABLE
DECODER_UNAVAILABLE
OUTPUT_EXISTS
OUTPUT_UNWRITABLE
INSUFFICIENT_DISK_SPACE
FFMPEG_NOT_FOUND
FFMPEG_FAILED
VERIFY_FAILED
OPERATION_CANCELLED
```

---

# 11. Capabilities

提供：

```bash
media capabilities
```

帮助 Agent 判断当前环境。

输出：

```json
{
  "ffmpeg": {
    "installed": true,
    "version": "8.x"
  },
  "platform": "macos",
  "architecture": "arm64",
  "hardware_acceleration": {
    "videotoolbox": true,
    "nvenc": false,
    "qsv": false
  },
  "encoders": {
    "h264": [
      "libx264",
      "h264_videotoolbox"
    ],
    "hevc": [
      "libx265",
      "hevc_videotoolbox"
    ]
  }
}
```

---

# 12. 硬件加速

用户可以：

```bash
media compress video.mp4 --hardware auto
```

支持策略：

```text
auto
cpu
gpu
```

自动检测：

### macOS

```text
VideoToolbox
```

### NVIDIA

```text
NVENC
```

### Intel

```text
Quick Sync / QSV
```

### AMD

```text
AMF / VAAPI
```

默认：

```text
hardware = auto
```

但质量优先任务允许内部自动选择 CPU 软件编码。

---

# 13. 输出文件策略

默认输出路径：

```text
input.mov
↓
input.mp4
```

如果存在：

```text
input.mp4
```

生成：

```text
input_1.mp4
```

绝不自动覆盖。

覆盖必须明确：

```bash
--overwrite
```

禁止：

```text
input == output
```

除非未来设计专门的 atomic replace 模式。

---

# 14. Metadata

默认尽量保留：

* creation time
* title
* artist
* language
* chapters
* stream metadata

如果因为目标格式无法保留，应在 Plan 阶段产生 warning。

---

# 15. Subtitle 策略

默认：

```text
preserve when compatible
```

如果不兼容：

```text
convert when safe
```

无法转换：

```text
warning
```

绝不无提示丢弃字幕轨。

---

# 16. Tool API

除 CLI 外，MediaForge 应设计结构化 Tool 层。

例如：

```text
inspect_media
plan_media_operation
convert_media
compress_media
resize_media
clip_media
extract_audio
create_thumbnail
verify_media
```

例如：

```typescript
convert_media({
  input: string,
  output_format?: "mp4" | "mkv" | "mov" | "webm",
  video_codec?: "auto" | "copy" | "h264" | "h265" | "av1",
  audio_codec?: "auto" | "copy" | "aac" | "opus" | "mp3",
  quality?: "lossless" | "very_high" | "high" | "balanced" | "small",
  hardware?: "auto" | "cpu" | "gpu",
  overwrite?: boolean,
  dry_run?: boolean
})
```

---

# 17. Skill 设计

项目提供独立：

```text
skills/mediaforge/
```

建议结构：

```text
mediaforge/
├── SKILL.md
├── agents/
│   └── openai.yaml
├── scripts/
└── references/
    ├── operations.md
    ├── codecs.md
    ├── presets.md
    └── troubleshooting.md
```

Skill 不负责实现媒体处理。

Skill 负责告诉 Agent：

```text
什么时候调用 MediaForge
如何调用
执行顺序
如何处理失败
什么情况下需要确认用户
如何验证结果
```

---

# 18. Agent 标准工作流

Skill 默认规定：

```text
1. Inspect
2. Understand requested result
3. Plan
4. Evaluate warnings
5. Execute
6. Verify
7. Return output
```

例如用户说：

> 帮我把这个视频压到 500MB 以内。

Agent：

```text
media inspect video.mkv
```

↓

```text
media plan video.mkv --target-size 500MB
```

↓

检查：

```text
预计需要重新编码
4K → 4K
HEVC
预计 487MB
```

↓

执行：

```text
media compress ...
```

↓

```text
media verify ...
```

↓

最终返回：

```text
处理完成
489 MB
3840×2160
HEVC
原文件未修改
```

---

# 19. Raw FFmpeg Escape Hatch

高级任务允许：

```bash
media ffmpeg ...
```

但属于 Escape Hatch。

Skill 应规定：

只有以下情况允许使用：

* MediaForge 当前没有对应高级操作。
* 用户明确要求 FFmpeg 参数。
* 必须使用特殊 FFmpeg Filter。
* 必须处理实验性 Codec 或 Filter Graph。

Raw FFmpeg 不应成为默认路径。

---

# 20. 技术架构

建议：

```text
┌─────────────────────┐
│       AI Agent      │
└──────────┬──────────┘
           │
      Skill / Tool
           │
┌──────────▼──────────┐
│    MediaForge CLI   │
├─────────────────────┤
│ Intent / Operation  │
│ Planner             │
│ Safety Layer        │
│ Preset Engine       │
│ Capability Detector │
│ Executor            │
│ Validator           │
└───────┬───────┬─────┘
        │       │
    ffprobe   ffmpeg
```

---

# 21. 内部模块

建议：

```text
core/
├── inspect
├── plan
├── execute
├── verify
├── capabilities
├── presets
├── codecs
├── containers
├── errors
└── safety
```

CLI：

```text
cli/
```

Agent Schema：

```text
schemas/
```

Skill：

```text
skills/
```

---

# 22. 技术栈建议

V1 推荐：

```text
Rust
+
FFmpeg CLI
+
FFprobe CLI
```

Rust 负责：

* CLI
* 参数校验
* JSON Schema
* Process management
* 文件操作
* 安全策略
* Planner
* Error handling

FFmpeg 继续作为独立系统依赖。

优点：

* 单二进制程序。
* 启动速度快。
* 适合 CLI。
* 跨平台。
* 类型安全。
* 易于分发。
* 非常适合 Agent 工具。

备选方案：

```text
TypeScript + Node.js
```

适合快速开发，但长期作为系统级 CLI，Rust 更合适。

---

# 23. 配置文件

允许：

```text
~/.config/mediaforge/config.toml
```

示例：

```toml
default_quality = "balanced"
hardware = "auto"
overwrite = false
verify_after_execute = true

[video]
preferred_codec = "h265"

[audio]
preferred_codec = "aac"
```

但配置只能改变默认值，不能降低核心安全策略。

---

# 24. 性能要求

CLI 本身：

* 冷启动 < 100ms 为理想目标。
* Inspect 额外开销尽可能低。
* 不复制无必要文件。
* FFmpeg 输出实时消费，避免 pipe buffer 堵塞。
* 大文件处理采用 streaming。
* 不将媒体整体读入内存。

---

# 25. 可观测性

支持：

```bash
--verbose
```

以及：

```bash
--debug
```

Agent 默认不应看到大量 FFmpeg 原生日志。

Agent 模式：

```text
stdout → JSON
stderr → debug log
```

---

# 26. 进度输出

长时间任务必须支持进度。

人类模式：

```text
Converting video.mp4

████████████████░░░░░ 72%

Elapsed: 03:21
Remaining: ~01:17
Speed: 1.7x
```

Agent 模式未来可支持：

```json
{
  "event": "progress",
  "progress": 0.72
}
```

例如 NDJSON：

```text
{"event":"start"}
{"event":"progress","value":0.1}
{"event":"progress","value":0.5}
{"event":"progress","value":0.9}
{"event":"complete"}
```

---

# 27. Acceptance Criteria

V1 发布必须满足：

### Inspect

* 能正确读取 MP4/MKV/MOV/WebM。
* 能识别主要视频、音频、字幕轨。
* JSON Schema 稳定。

### Convert

* MKV H264/AAC → MP4 自动 Remux。
* 不进行不必要的重编码。
* 不覆盖已有文件。

### Compress

* 可以通过 Quality preset 正常压缩。
* 输出文件可正常播放。

### Resize

* 支持 4K → 1080p。
* 保持宽高比。

### Clip

* 支持开始时间 + 时长。
* 输出 Duration 正确。

### Extract Audio

* 支持 MP3/AAC/FLAC/WAV。

### Thumbnail

* 能从指定时间提取图片。

### Batch

* 单文件错误不会中断全部任务。

### Verify

* 能检测损坏输出。
* 能发现 Duration 严重异常。
* 能发现音轨或视频轨意外缺失。

### Safety

* 不主动覆盖源文件。
* input/output 路径冲突必须拒绝。
* 输出文件已存在必须创建新名称或报错。

---

# 28. 后续版本规划

## V1.1

增加：

```text
media crop
media rotate
media concat
media gif
media normalize-audio
media replace-audio
```

---

## V1.2

增加：

```text
ImageMagick / libvips
```

扩展到：

```text
media image convert
media image resize
media image compress
```

---

## V1.3

集成：

```text
ExifTool
```

支持：

* EXIF
* Metadata 清理
* Metadata 编辑

---

## V2

扩展至：

```text
Whisper
```

增加：

```text
media transcribe
media subtitle generate
media subtitle translate
```

---

## V2.x

考虑集成：

```text
yt-dlp
Pandoc
OCR
PDF tooling
```

最终从 MediaForge 演进为更通用的：

> **Agent-native file transformation infrastructure**

形成统一模型：

```text
Inspect
Plan
Transform
Verify
```

不仅适用于视频，还可以覆盖：

```text
视频
音频
图片
文档
PDF
字幕
压缩包
```

---

# 29. 核心差异化

MediaForge 与直接使用 FFmpeg 的最大区别不是功能更多。

而是：

```text
FFmpeg
=
Powerful media engine

MediaForge
=
Reliable media interface for Agents
```

MediaForge 主要提供 FFmpeg 本身缺少的：

* 意图级 API
* 自动规划
* 安全默认值
* Remux 智能判断
* Codec 兼容性判断
* 机器可读 JSON
* 统一错误 Schema
* 自动硬件能力检测
* 执行前 Plan
* 执行后 Verify
* Agent Skill
* Tool Schema

因此 MediaForge 的核心价值不应该是：

> 「FFmpeg 的简化包装器」

而应该是：

> **「AI Agent 与媒体基础设施之间的可靠执行层。」**

---

# 30. V1 最小可行产品总结

第一版只需要把以下十个命令做好：

```text
inspect
plan
convert
compress
resize
clip
extract-audio
thumbnail
batch
verify
```

核心闭环：

```text
用户需求
   ↓
Agent
   ↓
Inspect
   ↓
Plan
   ↓
Execute
   ↓
Verify
   ↓
结构化结果
```

只要这一闭环足够稳定，MediaForge 就已经能够成为一个有明确差异化价值、适合 ChatGPT、Claude Code、Codex、OpenCode、DeepSeek Harness 等 Agent 使用的通用媒体处理基础设施。
