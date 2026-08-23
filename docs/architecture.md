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

For semantic transformations, `--progress` adds an independent stderr
channel. Human CLI mode prints readable status text; JSON/Tool mode emits
(`start`, `progress`, `complete`) NDJSON events. It never mixes progress lines
into the Tool response on stdout. When an operation does not provide an
explicit duration, the progress layer probes the first media input so ordinary
convert, compress, and resize jobs still report normalized completion,
elapsed time, and an estimated remaining time.

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
- **Progress and diagnostics** are side channels. `--verbose`/`--debug` expose
  human-readable diagnostics on stderr, while `--progress` emits one JSON
  object per event on stderr.
- **Process output** is consumed incrementally. Failure diagnostics retain a
  bounded stderr tail so long FFmpeg jobs cannot grow agent memory without
  limit.

## Hardware selection

Hardware encoding is opt-in. `auto` deliberately selects software encoding so
the same request remains predictable across hosts. `gpu` probes FFmpeg's
available hardware encoders for the requested codec and records the selected
encoder in the plan. A runtime failure to create the hardware session becomes
the structured `HARDWARE_UNAVAILABLE` error with a CPU fallback suggestion.
Software video operations also probe the live FFmpeg encoder list before
building a plan, so the selected encoder is one that the current build can
actually execute.

## Semantic operation families

The current command surface is intentionally grouped by the kind of decision
an Agent needs to make:

- **Container/video:** `convert`, `compress`, `resize`, `clip`, and `batch`
  choose container-compatible codecs and preserve or explain stream changes.
- **Image:** `image` uses FFmpeg's image encoders for format conversion,
  scaling, rotation, watermark overlays, and quality/compression controls.
- **Edit/join:** `edit` builds a bounded filter chain for crop, rotate, speed,
  volume, named filters, subtitles, and ranges; `merge` handles concat, mux,
  and mix with explicit stream expectations.
- **Audio:** `audio` and `extract-audio` share format routing while exposing
  bitrate, sample-rate, channels, volume, and time-range controls.
- **Recovery/device/disc:** `repair` applies timestamp/corruption-tolerant
  remuxing or an explicit re-encode; `presets` feeds device-aware conversion;
  `disc` is a best-effort DVD/CD/ISO bridge with host capability warnings.

The supported format lists are declarations of routing intent, not a promise
that every FFmpeg build contains every encoder. Plans probe the active build,
and execution maps missing encoders to `ENCODER_UNAVAILABLE`.

Target-size compression adds a two-pass software execution phase when no
hardware encoder is selected. The pass log is temporary and cleaned after the
final output; hardware encoding remains single-pass for portability.

## Invariants

1. A source path is never selected as an output path.
2. Existing outputs are not overwritten unless the caller explicitly requests
   `overwrite`.
3. Every planned operation can be inspected before execution with `dry_run`.
4. Successful media transformations are verified unless verification is
   disabled by the request/configuration.
5. Errors are structured and actionable; raw process text is retained as
   details rather than becoming the API itself.
6. Subtitle and metadata decisions are represented in plans; incompatible
   subtitle conversions produce warnings instead of silent stream loss.
7. Resize verification checks the planned effective dimension and even output
   geometry; target-size compression checks the actual output byte size.
8. Device presets may add a scale filter but never relax output safety or
   verification.
9. Optical-disc operations never claim DRM bypass or disc-authoring support;
   platform permissions and optional utilities remain visible in warnings and
   capabilities.

The single-file implementation is deliberate for this first milestone: the
behavioral contract is still moving. As operations grow, the next natural
split is `model`, `probe`, `plan`, `execute`, and `verify` modules without
changing the Tool API.
