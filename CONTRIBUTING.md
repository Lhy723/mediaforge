# Contributing to MediaForge

Thanks for helping improve an agent-native media tool.

## Development setup

Install a stable Rust toolchain and make sure both `ffmpeg` and `ffprobe` are
available on `PATH`. Then run:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --bin media
```

## Pull requests

- Keep changes focused and explain the user-facing behavior in the PR body.
- Add or update unit tests for parsing, planning, safety, or response changes.
- Preserve the JSON contract: stdout is for the response, stderr is for logs.
- Do not weaken collision protection or make overwrite implicit.
- Update the schema, Agent Skill, and README when the Tool API changes.
- Run the full local checks before pushing.

## Adding an operation

Add the request/response shape and validation first, then wire the operation
through the plan and execution layers. Include dry-run behavior, structured
errors, output safety, and a verification path. Update
[`schemas/tool-api.json`](schemas/tool-api.json) and
[`skills/mediaforge/SKILL.md`](skills/mediaforge/SKILL.md) in the same change.

## Commit and release conventions

Use concise Conventional Commit-style subjects such as `feat:`, `fix:`, or
`docs:`. Releases are cut from tags matching `v*.*.*`; the release workflow
builds the supported binaries and attaches them to a GitHub Release.
