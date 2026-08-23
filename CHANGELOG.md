# Changelog

All notable changes to MediaForge are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Rust CLI with semantic media operations backed by FFmpeg and FFprobe.
- Verb-style Tool operation aliases matching the PRD (`convert_media`,
  `plan_media_operation`, `create_thumbnail`, and related names).
- Inspect, plan, convert, compress, resize, clip, audio extraction, thumbnail,
  batch, verification, and capability commands.
- JSON-over-stdin Tool entrypoint with structured errors and a Raw FFmpeg escape
  hatch.
- Schema conditions for input/output requirements and an explicit
  `verify_after_execute` control.
- Explicit GPU requests now probe available hardware encoders and return a
  structured `HARDWARE_UNAVAILABLE` error when the runtime cannot create a
  hardware session.
- Safe output naming, collision detection, dry-run support, and post-operation
  verification.
- TOML configuration, Tool API schema, Agent Skill, architecture notes, and
  contributor documentation.
- Continuous integration and tagged-release workflows.
- Semantic plan inference for compression, resize, clipping, audio extraction,
  and thumbnails, plus explicit copy-versus-transcode strategies.
- Safe audio copy extraction, clip stream-copy selection, subtitle/metadata
  preservation metadata, structured capability maps, and post-operation checks
  for size, codecs, stream counts, and decode errors.
- CLI/Tool progress events on stderr, `--debug`, and configuration defaults
  shared by CLI and Tool calls.
- Convert quality presets now flow through both CLI and Tool API requests;
  semantic FFmpeg stderr is consumed incrementally, and human progress text is
  available alongside Agent NDJSON events.
- Inspect derives bit depth from pixel formats, warns on unsafe subtitle
  conversions, and validates invalid CLI arguments as structured JSON when
  requested.
- Target-size compression now performs and reports two-pass software encoding,
  with temporary pass logs cleaned after execution.
- Semantic video operations now select software encoders from the live FFmpeg
  encoder list, so capability reports and generated plans stay aligned across
  FFmpeg builds (including AV1).
- Acceptance coverage now proves the PRD release gates for H.264/AAC remux,
  4K-to-1080p resize, all required audio extraction formats, source
  immutability, and corrupt/mismatched output detection.
- The bundled Agent Skill now includes OpenAI/Codex discovery metadata.
- Progress reporting now derives duration from the media input when needed and
  includes elapsed and estimated remaining time in both human and Agent modes.
- Resize rounds odd requested dimensions to an explicit even target and
  verifies the rendered geometry; target-size compression verifies the actual
  output size against the requested byte limit.
- WebM conversion now selects VP9/Opus automatically and rejects incompatible
  explicit codec/container combinations during planning.
- Semantic FFmpeg execution now keeps a bounded diagnostic tail while
  consuming process output incrementally.
- Successful execution responses now carry forward plan warnings, quality-loss,
  hardware, subtitle, and metadata decisions for Agent consumers.
- CI and release workflows now use the current checkout runtime, and
  Dependabot tracks both Cargo and GitHub Actions dependencies.
- Added image conversion (PNG/JPEG/WebP/GIF/BMP/TIFF/ICO/TGA/AVIF), resize,
  rotate, watermark, and quality controls with encoder-aware failures.
- Added edit, merge/concat/mux/mix, extended audio conversion controls, repair,
  device presets, and capability-aware DVD/CD/ISO entry points.
- Expanded video/audio container and codec routing, Tool API schema fields,
  capability reports, acceptance coverage, and Agent Skill guidance.
- Added the dependency-free static documentation page at `docs/index.html` for
  agent workflows and operation examples.
- Added palette-based video-to-GIF conversion with bounded FPS, duration, and
  width controls, plus post-operation verification that the result is video-only.
- Added ASS/SSA subtitle style forwarding through `force_style`, capability
  probing for the FFmpeg `subtitles` filter, and structured `FILTER_UNAVAILABLE`
  errors when the host lacks the required filter.
- Added ISO authoring actions with capability-aware selection of `xorriso`,
  `genisoimage`, `mkisofs`, or macOS `hdiutil`, including structured
  `DISC_TOOL_UNAVAILABLE` and `DISC_TOOL_FAILED` errors.
- Promoted Windows x64 to a first-class platform: `.exe`/`PATHEXT` tool
  discovery, `APPDATA` config fallback, Windows acceptance in CI, and zipped
  release artifacts are now covered alongside macOS and Linux.
