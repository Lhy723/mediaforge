# Architecture

MediaForge is a small control plane around local FFmpeg and FFprobe processes.
The binary owns the agent-facing contract; FFmpeg remains the media engine.

```text
CLI arguments or one JSON Tool request
                  |
                  v
        normalize + validate request
                  |
                  v
          inspect (FFprobe when needed)
                  |
                  v
             build an execution plan
                  |
                  v
      output safety + collision resolution
                  |
                  v
          execute FFmpeg / Raw FFmpeg
                  |
                  v
      verify output with FFprobe / filesystem
                  |
                  v
         stable JSON response or error
```

## Boundaries

- **MediaForge** validates paths, normalizes semantic options, chooses a
  deterministic command plan, and formats responses.
- **FFprobe** is the source of media metadata and post-operation checks.
- **FFmpeg** performs conversion, filtering, extraction, and thumbnail work.
- **The Tool entrypoint** reads exactly one JSON request from stdin and writes
  exactly one JSON response to stdout. Diagnostics never share stdout.
- **Raw FFmpeg** is intentionally explicit. It is available for operations
  that are not yet represented by the semantic API, but it does not bypass the
  process boundary or the JSON response contract.

## Hardware selection

Hardware encoding is opt-in. `auto` deliberately selects software encoding so
the same request remains predictable across hosts. `gpu` probes FFmpeg's
available hardware encoders for the requested codec and records the selected
encoder in the plan. A runtime failure to create the hardware session becomes
the structured `HARDWARE_UNAVAILABLE` error with a CPU fallback suggestion.

## Invariants

1. A source path is never selected as an output path.
2. Existing outputs are not overwritten unless the caller explicitly requests
   `overwrite`.
3. Every planned operation can be inspected before execution with `dry_run`.
4. Successful media transformations are verified unless verification is
   disabled by the request/configuration.
5. Errors are structured and actionable; raw process text is retained as
   details rather than becoming the API itself.

The single-file implementation is deliberate for this first milestone: the
behavioral contract is still moving. As operations grow, the next natural
split is `model`, `probe`, `plan`, `execute`, and `verify` modules without
changing the Tool API.
