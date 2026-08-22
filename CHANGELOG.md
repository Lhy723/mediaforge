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
