use super::prelude::*;
use super::{error::*, execution::*, format::*, model::*, paths::*, process::*, state::*};

pub(crate) fn repair_command(context: &Context, args: &RepairArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let source_streams =
        probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let extension = args.input.extension().and_then(OsStr::to_str).unwrap_or("mp4");
    let output = if let Some(path) = &args.output {
        resolve_output(context, &args.input, Some(path), extension)?
    } else {
        let stem = args.input.file_stem().and_then(OsStr::to_str).unwrap_or("media");
        let requested = args.input.with_file_name(format!("{stem}_repaired.{extension}"));
        resolve_output(context, &args.input, Some(&requested), extension)?
    };
    let mut ffmpeg_args = vec![
        "-fflags".to_string(),
        "+genpts+discardcorrupt".to_string(),
        "-err_detect".to_string(),
        "ignore_err".to_string(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
    ];
    if args.reencode {
        ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    } else {
        ffmpeg_args.extend(["-c".to_string(), "copy".to_string()]);
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "repair",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "strategy": if args.reencode { "repair_transcode" } else { "repair_copy" },
            "reencode": args.reencode,
            "audio_present": first_stream(&source_streams, "audio").is_some(),
            "quality_loss": if args.reencode { "possible" } else { "none" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: if args.reencode {
            "repair_transcode".to_string()
        } else {
            "repair_copy".to_string()
        },
    };
    finish_custom_plan(context, &args.input, plan)
}

pub(crate) fn disc_command(context: &Context, args: &DiscArgs) -> Result<Value, AppError> {
    if !args.input.exists() {
        return Err(AppError::new(
            "FILE_NOT_FOUND",
            format!("Disc source does not exist: {}", args.input.display()),
        ));
    }
    let kind = args.kind.to_lowercase();
    if !["dvd", "cd", "iso"].contains(&kind.as_str()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Disc kind must be dvd, cd, or iso."));
    }
    let action = normalize_disc_action(&args.action)?;
    if action == "create_iso" {
        return create_iso_command(context, args, &kind);
    }
    let default_format = if kind == "cd" { "flac" } else { "mp4" };
    let target = args.to.clone().unwrap_or_else(|| default_format.to_string());
    let (format, extension) = if kind == "cd" {
        let format = normalize_audio_format(&target)?;
        let extension = audio_output_extension(&format).to_string();
        (format, extension)
    } else {
        let format = normalize_container(&target)?;
        (format.clone(), format)
    };
    let output = resolve_output(context, &args.input, args.output.as_deref(), &extension)?;
    let mut ffmpeg_args = vec!["-i".to_string(), args.input.to_string_lossy().to_string()];
    if kind == "cd" {
        ffmpeg_args.extend(["-map".to_string(), "0:a?".to_string(), "-vn".to_string()]);
        ffmpeg_args
            .extend(audio_encode_args(audio_codec_for_format(&format), DEFAULT_AUDIO_BITRATE)?);
        ffmpeg_args.extend(audio_container_args(&format));
    } else if kind == "dvd" {
        ffmpeg_args.extend(["-map".to_string(), "0".to_string()]);
        ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    } else {
        ffmpeg_args.extend([
            "-map".to_string(),
            "0".to_string(),
            "-c".to_string(),
            "copy".to_string(),
        ]);
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "disc",
            "kind": kind,
            "action": action,
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "strategy": if kind == "iso" { "disc_copy" } else { "disc_transcode" },
            "ffmpeg_args": ffmpeg_args,
            "warnings": ["Disc devices and protected media depend on platform permissions and FFmpeg build support."],
        }),
        output,
        args: ffmpeg_args,
        strategy: if kind == "iso" {
            "disc_copy".to_string()
        } else {
            "disc_transcode".to_string()
        },
    };
    finish_custom_plan(context, &args.input, plan)
}

pub(crate) fn normalize_disc_action(value: &str) -> Result<String, AppError> {
    match value.to_lowercase().replace('-', "_").as_str() {
        "extract" | "convert" | "remux" => Ok("extract".to_string()),
        "create_iso" | "author" | "write_iso" => Ok("create_iso".to_string()),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported disc action: {other}"))
                .with_suggestions(&["Use --action extract or --action create-iso."]))
        }
    }
}

pub(crate) fn disc_authoring_tool() -> Option<&'static str> {
    ["xorriso", "genisoimage", "mkisofs", "hdiutil"]
        .into_iter()
        .find(|tool| program_available(tool))
}

pub(crate) fn create_iso_command(
    context: &Context,
    args: &DiscArgs,
    kind: &str,
) -> Result<Value, AppError> {
    if !args.input.is_dir() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "ISO creation requires a directory containing the disc files.",
        )
        .with_suggestions(&[
            "Mount or stage the DVD/CD contents in a directory, then pass that directory as input.",
            "Use --action extract when the input is an existing ISO or media file.",
        ]));
    }
    let output = resolve_output(context, &args.input, args.output.as_deref(), "iso")?;
    let label =
        args.volume_label.clone().unwrap_or_else(|| format!("MEDIAFORGE-{}", kind.to_uppercase()));
    if label.is_empty() || label.len() > 32 || label.chars().any(|character| character.is_control())
    {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "ISO volume label must be 1-32 printable characters.",
        ));
    }
    let tool = disc_authoring_tool();
    let (program, tool_args) = match tool {
        Some("xorriso") => (
            "xorriso".to_string(),
            vec![
                "-as".to_string(),
                "mkisofs".to_string(),
                "-quiet".to_string(),
                "-J".to_string(),
                "-R".to_string(),
                "-V".to_string(),
                label.clone(),
                "-o".to_string(),
                output.to_string_lossy().to_string(),
                args.input.to_string_lossy().to_string(),
            ],
        ),
        Some("genisoimage") | Some("mkisofs") => {
            let program = tool.unwrap().to_string();
            (
                program,
                vec![
                    "-quiet".to_string(),
                    "-J".to_string(),
                    "-R".to_string(),
                    "-V".to_string(),
                    label.clone(),
                    "-o".to_string(),
                    output.to_string_lossy().to_string(),
                    args.input.to_string_lossy().to_string(),
                ],
            )
        }
        Some("hdiutil") => (
            "hdiutil".to_string(),
            vec![
                "makehybrid".to_string(),
                "-ov".to_string(),
                "-o".to_string(),
                output.to_string_lossy().to_string(),
                "-hfs".to_string(),
                "-joliet".to_string(),
                "-iso".to_string(),
                "-default-volume-name".to_string(),
                label.clone(),
                args.input.to_string_lossy().to_string(),
            ],
        ),
        _ => (String::new(), Vec::new()),
    };
    let mut value = json!({
        "status": "success",
        "operation": "disc",
        "action": "create_iso",
        "kind": kind,
        "input": absolute_display(&args.input),
        "output": absolute_display(&output),
        "format": "iso",
        "strategy": "disc_authoring",
        "tool": if program.is_empty() { Value::Null } else { json!(program) },
        "tool_available": !program.is_empty(),
        "tool_args": tool_args,
        "volume_label": label,
        "warnings": [
            "ISO creation depends on an installed authoring utility; FFmpeg alone does not author ISO images.",
            "This operation creates a filesystem image and does not bypass optical-media DRM."
        ],
    });
    if context.dry_run {
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    if program.is_empty() {
        return Err(AppError::new(
            "DISC_TOOL_UNAVAILABLE",
            "No ISO authoring utility was found on PATH.",
        )
        .with_details(json!({
            "candidates": ["xorriso", "genisoimage", "mkisofs", "hdiutil"],
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
        }))
        .with_suggestions(&[
            "Install xorriso, genisoimage, or mkisofs, then retry.",
            "On macOS, hdiutil is normally available when the system permits it.",
        ]));
    }
    let refs = tool_args.iter().map(String::as_str).collect::<Vec<_>>();
    if let Err(error) = run_program(&program, &refs, context.verbose) {
        return Err(AppError::new(
            "DISC_TOOL_FAILED",
            format!("{program} could not create the ISO image."),
        )
        .with_details(json!({
            "tool": program,
            "arguments": refs,
            "cause": error.code,
            "message": error.message,
            "details": error.details,
        }))
        .with_suggestions(&[
            "Check source-directory permissions and available disk space.",
            "Run the same operation with --dry-run to inspect the authoring command.",
        ]));
    }
    let size_bytes = fs::metadata(&output).map(|metadata| metadata.len()).unwrap_or(0);
    if size_bytes == 0 {
        return Err(AppError::new(
            "DISC_TOOL_FAILED",
            "ISO authoring completed without producing a non-empty output.",
        )
        .with_details(json!({"output": absolute_display(&output), "tool": program})));
    }
    value["status"] = json!("success");
    value["output"] = json!(absolute_display(&output));
    value["verification"] = json!({
        "status": "success",
        "valid": true,
        "checks": {"size_bytes": size_bytes, "size_positive": true, "readable": true}
    });
    Ok(value)
}
