# PRD V1 Compliance Matrix

This matrix links each MediaForge V1 release requirement to repeatable evidence
in the repository. The authoritative specification remains
[`MediaForge-PRD-v1.0.md`](MediaForge-PRD-v1.0.md).

| PRD area | Implemented behavior | Repeatable evidence |
| --- | --- | --- |
| Inspect | Normalizes file, video, audio, subtitle, HDR, bit-depth, language, and disposition metadata. | `scripts/acceptance.sh` inspects generated MP4, MKV, MOV, and WebM fixtures; Rust tests cover bit-depth derivation. |
| Plan | Reports operation, copy/transcode strategy, stream decisions, metadata/subtitle policy, hardware, quality loss, reasons, warnings, output, and FFmpeg arguments without writing output. | Acceptance checks conversion, compression, resize, subtitle, and Tool plans, including dry-run directory immutability. |
| Convert | Remuxes compatible streams; transcodes only incompatible streams; selects container-compatible defaults. | Acceptance executes H.264/AAC MKV to MP4 as `remux`, and H.264/AAC MP4 to VP9/Opus WebM as `transcode`; incompatible explicit WebM codecs are rejected before execution. |
| Compress | Supports all quality presets and target byte sizes, with two-pass software encoding. | Acceptance executes quality and target-size compression and requires `target_size_match: true`; the plan and response report pass count and strategy. |
| Resize | Preserves aspect ratio, targets width or height, and guarantees even encoder dimensions. | Acceptance executes a 3840×2160 to 1920×1080 resize and checks FFprobe dimensions; an odd width plan is normalized from 321 to 322; post-operation verification checks geometry. |
| Clip | Supports start plus duration or end, choosing stream copy at a safe zero boundary and precise re-encode otherwise. | Acceptance executes copy, duration-based precise, and end-based precise clips and validates output duration. |
| Extract audio | Supports MP3, AAC, M4A, FLAC, WAV, and Opus, copying compatible audio when possible. | Acceptance executes M4A copy plus MP3, AAC, FLAC, WAV, and Opus outputs; codec routing is covered by Rust tests. |
| Thumbnail | Extracts a frame by timestamp or percentage. | Acceptance extracts and decodes a 50% JPEG thumbnail. |
| Batch | Supports glob and recursive directory discovery; one failed input does not stop the batch. | Acceptance requires `partial_success` for two valid files plus one corrupt file, and separately verifies recursive discovery. |
| Verify | Checks parseability, positive size, duration, required streams, resolution/codec/stream differences, and FFmpeg decode errors. | Acceptance rejects corrupt output, severe duration drift, missing audio, and missing video. Transform-specific checks validate clip duration, resize geometry, and compression target size. |
| Safety | Never mutates input, rejects input/output identity, avoids implicit overwrite, and does not create directories during dry-run. | Acceptance hashes the source before/after, executes collision suffixing while preserving the existing file, rejects identical paths and invalid output parents, and checks dry-run filesystem behavior. |
| Agent JSON contract | Keeps one parseable response on stdout and diagnostics/progress on stderr; supports semantic aliases and Raw FFmpeg escape hatch. | Acceptance parses every response, invokes Tool plan/convert aliases, and exercises Raw FFmpeg dry-run. Invalid CLI and runtime errors use the common structured error envelope. |
| Progress | Reports normalized progress, elapsed time, estimated remaining time, and speed without buffering media. | Acceptance checks Agent NDJSON and human stderr modes, including duration derived from an ordinary conversion input. FFmpeg stderr is consumed incrementally with a bounded diagnostic tail. |
| Capabilities and hardware | Reports FFmpeg/platform/architecture, acceleration backends, and available encoders; explicit GPU requests probe an encoder before execution. | Acceptance validates the capabilities object; Rust tests cover encoder selection. Actual GPU session creation remains host-dependent and returns `HARDWARE_UNAVAILABLE` with a CPU fallback when the runtime cannot create it. |
| Configuration | Applies quality, codec, hardware, verification, and progress defaults without allowing config to weaken overwrite safety. | Acceptance loads a temporary TOML config and checks effective plan defaults; overwrite still requires an explicit CLI/Tool request. |
| Agent Skill | Provides workflow, safety, error handling, Tool use, and Raw FFmpeg routing instructions plus discovery metadata. | `skills/mediaforge/SKILL.md` and `skills/mediaforge/agents/openai.yaml` pass the bundled skill validator. |
| Platforms and release gate | Builds and runs on the V1 target platforms. | GitHub Actions runs formatting, tests, Clippy, release build, and the full acceptance script on Ubuntu and macOS. |

## Release gate

Before a V1 release, all of the following must pass from a clean checkout:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --bin media
./scripts/acceptance.sh target/release/media
```

Hardware encoder availability is intentionally not a universal CI assertion:
the public runners do not guarantee GPU devices. MediaForge instead verifies
the encoder list while planning and reports a structured runtime failure if a
listed backend cannot create a session on that host.
