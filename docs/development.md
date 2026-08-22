# Development Guide

## Prerequisites

- Rust stable with `rustfmt` and `clippy` (the repository pins these through
  [`rust-toolchain.toml`](../rust-toolchain.toml));
- FFmpeg and FFprobe on `PATH` for integration tests and real media checks;
- Git and, optionally, the GitHub CLI for publishing releases.

## Local checks

Run the same checks used by CI:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --bin media
./scripts/acceptance.sh target/release/media
```

Smoke-test the agent contract with a dry run:

```bash
printf '%s\n' '{"operation":"capabilities"}' | target/release/media tool
printf '%s\n' '{"operation":"ffmpeg","args":["-version"],"dry_run":true}' \
  | target/release/media tool
```

For a real media regression, generate or provide a small local sample and run
`inspect`, `plan`, one transformation, and `verify`. Keep generated media out
of the repository; build artifacts and local caches are ignored by Git.

The acceptance smoke test creates a short synthetic MP4 in a temporary
directory and exercises the PRD V1 command loop, safety defaults, Tool API,
progress channel, and structured verification failures. It requires both
`ffmpeg` and `ffprobe` on `PATH`.

The script also creates MP4, MKV, MOV, and WebM fixtures, checks subtitle
conversion, exercises both human and Agent progress channels, and verifies
output-parent safety.

## Configuration during development

Set `MEDIAFORGE_CONFIG` to a temporary TOML file when testing codec or quality
defaults. The config may select defaults, but it cannot turn on overwrite by
itself.

## Release process

1. Update `Cargo.toml` and `CHANGELOG.md`.
2. Run all local checks and review the generated `Cargo.lock`.
3. Create and push a tag such as `v0.1.0`.
4. GitHub Actions builds the supported targets and publishes the archives to a
   GitHub Release.

## Changing the Tool API

Treat [`schemas/tool-api.json`](../schemas/tool-api.json), the Rust request
model, README examples, and [`skills/mediaforge/SKILL.md`](../skills/mediaforge/SKILL.md)
as one change set. Preserve one-request/one-response behavior and add a test
for any new alias, validation rule, or structured error. Keep progress and
debug output on stderr so stdout remains directly parseable JSON.
