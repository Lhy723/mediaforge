use super::prelude::*;
use super::{error::*, format::*, model::*, parse::*, state::*};

pub(crate) fn probe_media(input: &Path, verbose: bool) -> Result<Probe, AppError> {
    ensure_input(input)?;
    let result = run_program(
        "ffprobe",
        &[
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &input.to_string_lossy(),
        ],
        verbose,
    )?;
    let raw: Value = serde_json::from_str(&result.stdout).map_err(|error| {
        AppError::new("INVALID_MEDIA", format!("ffprobe returned invalid JSON: {error}"))
    })?;
    let duration_seconds =
        raw.get("format").and_then(|format| number_field(format, "duration")).or_else(|| {
            raw.get("streams").and_then(Value::as_array).and_then(|streams| {
                streams
                    .iter()
                    .filter_map(|stream| number_field(stream, "duration"))
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            })
        });
    Ok(Probe { raw, duration_seconds })
}

#[derive(Debug)]
pub(crate) struct ProcessResult {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) fn run_program(
    program: &str,
    args: &[&str],
    verbose: bool,
) -> Result<ProcessResult, AppError> {
    let output = ProcessCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| process_start_error(program, error))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if verbose && !stderr.trim().is_empty() {
        eprintln!("[{program}] {}", stderr.trim());
    }
    if !output.status.success() {
        return Err(process_failure_error(program, args, output.status, &stderr));
    }
    Ok(ProcessResult { stdout, stderr })
}

pub(crate) fn run_ffmpeg(context: &Context, args: &[String]) -> Result<(), AppError> {
    if context.progress {
        return run_ffmpeg_with_progress(context, args);
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let mut command = ProcessCommand::new("ffmpeg");
    command.args(&refs).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| process_start_error("ffmpeg", error))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::new("FFMPEG_FAILED", "Could not capture FFmpeg stderr."))?;
    let mut reader = BufReader::new(stderr_pipe);
    let mut line = String::new();
    let mut stderr = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            AppError::from_io("FFMPEG_FAILED", "Could not read FFmpeg stderr.", error)
        })?;
        if bytes == 0 {
            break;
        }
        append_stderr_tail(&mut stderr, &line);
        if context.verbose && !line.trim().is_empty() {
            eprint!("[ffmpeg] {line}");
        }
    }
    let status = child
        .wait()
        .map_err(|error| AppError::from_io("FFMPEG_FAILED", "Could not wait for FFmpeg.", error))?;
    if !status.success() {
        return Err(process_failure_error("ffmpeg", &refs, status, &stderr));
    }
    Ok(())
}

pub(crate) fn process_start_error(program: &str, error: io::Error) -> AppError {
    if error.kind() == io::ErrorKind::NotFound {
        AppError::new("FFMPEG_NOT_FOUND", format!("{program} was not found on PATH."))
            .with_suggestions(&[
                "Install FFmpeg and FFprobe.",
                "Run media capabilities to inspect the current environment.",
            ])
    } else {
        AppError::from_io("FFMPEG_FAILED", format!("Could not start {program}."), error)
    }
}

pub(crate) fn process_failure_error(
    program: &str,
    args: &[&str],
    status: std::process::ExitStatus,
    stderr: &str,
) -> AppError {
    let stderr_lower = stderr.to_lowercase();
    let code = if stderr_lower.contains("unknown encoder")
        || (stderr_lower.contains("encoder") && stderr_lower.contains("not found"))
        || stderr_lower.contains("experimental feature")
        || stderr_lower.contains("only supports")
        || stderr_lower.contains("does not support")
    {
        "ENCODER_UNAVAILABLE"
    } else if stderr_lower.contains("unknown decoder") {
        "DECODER_UNAVAILABLE"
    } else if stderr_lower.contains("cannot create compression session")
        || stderr_lower.contains("no capable devices found")
        || stderr_lower.contains("hardware encoder")
    {
        "HARDWARE_UNAVAILABLE"
    } else {
        "FFMPEG_FAILED"
    };
    let error = AppError::new(code, format!("{program} exited with status {status}."))
        .with_details(json!({"command":program,"arguments":args,"stderr":stderr}));
    if code == "HARDWARE_UNAVAILABLE" {
        error.with_suggestions(&[
            "Retry with --hardware cpu.",
            "Run media capabilities to inspect available hardware encoders.",
        ])
    } else {
        error
    }
}

pub(crate) fn run_ffmpeg_with_progress(context: &Context, args: &[String]) -> Result<(), AppError> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let expected_duration = progress_duration_seconds(args)
        .or_else(|| progress_input_duration_seconds(args, context.verbose));
    let started_at = Instant::now();
    let mut command = ProcessCommand::new("ffmpeg");
    command
        .args(&refs)
        .stdin(Stdio::null())
        .args(["-progress", "pipe:2", "-nostats"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| process_start_error("ffmpeg", error))?;
    emit_progress_event(context, "start", Some(0.0), None, None, Some(0.0), None);
    let stderr_pipe = child.stderr.take().ok_or_else(|| {
        AppError::new("FFMPEG_FAILED", "Could not capture FFmpeg progress output.")
    })?;
    let mut reader = BufReader::new(stderr_pipe);
    let mut line = String::new();
    let mut stderr = String::new();
    let mut out_time_ms: Option<f64> = None;
    let mut speed: Option<String> = None;
    let mut ended = false;
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            AppError::from_io("FFMPEG_FAILED", "Could not read FFmpeg progress output.", error)
        })?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim_end();
        append_stderr_tail(&mut stderr, trimmed);
        append_stderr_tail(&mut stderr, "\n");
        if let Some((key, value)) = trimmed.split_once('=') {
            match key {
                "out_time_ms" => out_time_ms = value.parse::<f64>().ok(),
                "speed" => speed = Some(value.to_string()),
                "progress" => {
                    let is_end = value == "end";
                    let normalized =
                        expected_duration.zip(out_time_ms).map(|(duration, milliseconds)| {
                            (milliseconds / 1_000_000.0 / duration).clamp(0.0, 1.0)
                        });
                    let elapsed_seconds = started_at.elapsed().as_secs_f64();
                    let remaining_seconds =
                        estimated_remaining_seconds(normalized, elapsed_seconds);
                    emit_progress_event(
                        context,
                        if is_end { "complete" } else { "progress" },
                        normalized,
                        out_time_ms,
                        speed.as_deref(),
                        Some(elapsed_seconds),
                        remaining_seconds,
                    );
                    ended = is_end;
                }
                _ => {}
            }
        } else if context.verbose && !trimmed.is_empty() {
            eprintln!("[ffmpeg] {trimmed}");
        }
    }
    let status = child
        .wait()
        .map_err(|error| AppError::from_io("FFMPEG_FAILED", "Could not wait for FFmpeg.", error))?;
    if !status.success() {
        return Err(process_failure_error("ffmpeg", &refs, status, &stderr));
    }
    if !ended {
        emit_progress_event(
            context,
            "complete",
            Some(1.0),
            out_time_ms,
            speed.as_deref(),
            Some(started_at.elapsed().as_secs_f64()),
            Some(0.0),
        );
    }
    Ok(())
}

pub(crate) fn emit_progress_event(
    context: &Context,
    event: &str,
    value: Option<f64>,
    out_time_ms: Option<f64>,
    speed: Option<&str>,
    elapsed_seconds: Option<f64>,
    remaining_seconds: Option<f64>,
) {
    if context.json {
        eprintln!(
            "{}",
            json!({"event":event,"value":value,"out_time_ms":out_time_ms,"speed":speed,"elapsed_seconds":elapsed_seconds,"remaining_seconds":remaining_seconds})
        );
        return;
    }
    match event {
        "start" => eprintln!("Converting media..."),
        "complete" => {
            let elapsed = elapsed_seconds
                .map(|seconds| format!(" Elapsed: {}.", format_progress_time(seconds)))
                .unwrap_or_default();
            eprintln!("Complete.{elapsed}");
        }
        _ => {
            let percent = value
                .map(|progress| format!("{:.0}%", progress * 100.0))
                .unwrap_or_else(|| "unknown".to_string());
            let elapsed =
                elapsed_seconds.map(|seconds| format!("elapsed {}", format_progress_time(seconds)));
            let remaining = remaining_seconds
                .map(|seconds| format!("remaining ~{}", format_progress_time(seconds)));
            let speed = speed.map(|value| format!("speed {value}"));
            let details =
                [elapsed, remaining, speed].into_iter().flatten().collect::<Vec<_>>().join(" | ");
            if details.is_empty() {
                eprintln!("Progress: {percent}");
            } else {
                eprintln!("Progress: {percent} | {details}");
            }
        }
    }
}

pub(crate) fn append_stderr_tail(buffer: &mut String, text: &str) {
    buffer.push_str(text);
    if buffer.len() <= MAX_CAPTURED_STDERR_BYTES {
        return;
    }
    let mut start = buffer.len() - MAX_CAPTURED_STDERR_BYTES;
    while !buffer.is_char_boundary(start) {
        start += 1;
    }
    buffer.drain(..start);
}

pub(crate) fn progress_duration_seconds(args: &[String]) -> Option<f64> {
    args.windows(2).find(|pair| pair[0] == "-t").and_then(|pair| parse_time_seconds(&pair[1]).ok())
}

pub(crate) fn progress_input_duration_seconds(args: &[String], verbose: bool) -> Option<f64> {
    let input = args.windows(2).find(|pair| pair[0] == "-i").map(|pair| Path::new(&pair[1]))?;
    probe_media(input, verbose).ok()?.duration_seconds
}

pub(crate) fn estimated_remaining_seconds(
    progress: Option<f64>,
    elapsed_seconds: f64,
) -> Option<f64> {
    let progress = progress?;
    if progress <= f64::EPSILON {
        return None;
    }
    Some((elapsed_seconds * (1.0 - progress) / progress).max(0.0))
}

pub(crate) fn format_progress_time(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub(crate) fn decode_check(context: &Context, input: &Path) -> Result<(), AppError> {
    // Decode only a bounded sample. This catches malformed headers/frames while
    // avoiding an infinite read for intentionally looping animated GIFs.
    let refs = ["-v", "error", "-t", "1", "-i", &input.to_string_lossy(), "-f", "null", "-"];
    run_program("ffmpeg", &refs, context.verbose).map(|_| ())
}

pub(crate) fn ensure_input(input: &Path) -> Result<(), AppError> {
    if !input.exists() {
        return Err(AppError::new(
            "FILE_NOT_FOUND",
            format!("Input file does not exist: {}", input.display()),
        )
        .with_details(json!({"path":input.display().to_string()})));
    }
    if !input.is_file() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("Input is not a file: {}", input.display()),
        ));
    }
    Ok(())
}

pub(crate) fn program_available(program: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|directory| {
        let candidate = directory.join(program);
        if candidate.is_file() {
            return true;
        }
        #[cfg(windows)]
        {
            if Path::new(program).extension().is_some() {
                false
            } else {
                let path_ext =
                    std::env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
                path_ext
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .any(|extension| directory.join(format!("{program}{extension}")).is_file())
            }
        }
        #[cfg(not(windows))]
        {
            false
        }
    })
}
