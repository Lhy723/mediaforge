use super::prelude::*;
use super::{config::*, dispatch::*, error::*, model::*, process::*, state::*};

pub(crate) fn tool_command(context: &Context, args: &ToolArgs) -> Result<Value, AppError> {
    let request_text = if let Some(request) = &args.request {
        request.clone()
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).map_err(|error| {
            AppError::from_io(
                "INVALID_ARGUMENT",
                "Could not read the Tool request from stdin.",
                error,
            )
        })?;
        input
    };
    if request_text.trim().is_empty() {
        return Err(AppError::new("INVALID_ARGUMENT", "Tool request must be a JSON object."));
    }
    let request: ToolRequest = serde_json::from_str(&request_text).map_err(|error| {
        AppError::new("INVALID_ARGUMENT", format!("Tool request is not valid JSON: {error}"))
            .with_suggestions(&["Send one JSON object with an operation and input field."])
    })?;
    let mut tool_context = context.clone();
    tool_context.json = true;
    if let Some(dry_run) = request.dry_run {
        tool_context.dry_run = dry_run;
    }
    // Config cannot enable overwrite implicitly: only the request or an explicit
    // CLI flag may relax the core safety policy.
    tool_context.overwrite = context.overwrite || request.overwrite.unwrap_or(false);
    tool_context.verify_after_execute =
        request.verify_after_execute.unwrap_or(context.verify_after_execute);
    tool_context.progress = request.progress.unwrap_or(context.progress);
    let video_codec = request.video_codec.clone();
    let audio_codec = request.audio_codec.clone();
    let requested_quality = request.quality.clone();
    let quality =
        requested_quality.as_ref().map(|value| parse_quality_name(value.clone())).transpose()?;
    let hardware =
        request.hardware.as_ref().map(|value| parse_hardware_name(value.clone())).transpose()?;
    let operation = normalize_operation(&request.operation);
    let input = || required_input(request.input.clone());
    match operation.as_str() {
        "inspect" => dispatch(&tool_context, Command::Inspect(InputArgs { input: input()? })),
        "plan" => dispatch(
            &tool_context,
            Command::Plan(PlanArgs {
                input: input()?,
                inputs: request
                    .inputs
                    .clone()
                    .map(|values| values.into_iter().map(PathBuf::from).collect()),
                operation: request.target_operation.clone(),
                to: request.output_format.clone(),
                output: request.output.clone().map(PathBuf::from),
                video_codec: video_codec.clone(),
                audio_codec: audio_codec.clone(),
                hardware,
                quality,
                target_size: request.target_size.clone(),
                width: request.width,
                resolution: request.resolution.clone(),
                start: request.start.clone(),
                duration: request.duration.clone(),
                end: request.end.clone(),
                format: request.format.clone(),
                at: request.at.clone(),
                fps: request.fps,
                device: request.device.clone(),
                height: request.height,
                crop: request.crop.clone(),
                rotate: request.rotate,
                speed: request.speed,
                volume: request.volume,
                filter: request.filter.clone(),
                subtitle: request.subtitle.clone().map(PathBuf::from),
                subtitle_style: request.subtitle_style.clone(),
                watermark: request.watermark.clone().map(PathBuf::from),
                image_quality: request.image_quality,
                bitrate: request.bitrate.clone(),
                sample_rate: request.sample_rate,
                channels: request.channels,
                reencode: request.reencode.unwrap_or(false),
                kind: request.kind.clone(),
                action: request.action.clone().unwrap_or_else(|| "extract".to_string()),
                volume_label: request.volume_label.clone(),
                mode: request.mode.clone().unwrap_or_else(|| "concat".to_string()),
            }),
        ),
        "convert" => dispatch(
            &tool_context,
            Command::Convert(ConvertArgs {
                input: input()?,
                to: request.output_format.clone(),
                output: request.output.clone().map(PathBuf::from),
                video_codec,
                audio_codec,
                hardware,
                quality,
                device: request.device.clone(),
            }),
        ),
        "compress" => dispatch(
            &tool_context,
            Command::Compress(CompressArgs {
                input: input()?,
                quality,
                target_size: request.target_size.clone(),
                output: request.output.clone().map(PathBuf::from),
                hardware,
            }),
        ),
        "resize" => dispatch(
            &tool_context,
            Command::Resize(ResizeArgs {
                input: input()?,
                width: request.width,
                resolution: request.resolution.clone(),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "clip" => dispatch(
            &tool_context,
            Command::Clip(ClipArgs {
                input: input()?,
                start: request.start.clone().ok_or_else(|| AppError::new("INVALID_ARGUMENT", "Tool clip requests require start."))?,
                duration: request.duration.clone(),
                end: request.end.clone(),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "extract_audio" => dispatch(
            &tool_context,
            Command::ExtractAudio(ExtractAudioArgs {
                input: input()?,
                format: request.format.clone().unwrap_or_else(|| "m4a".to_string()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "thumbnail" => dispatch(
            &tool_context,
            Command::Thumbnail(ThumbnailArgs {
                input: input()?,
                at: request.at.clone().unwrap_or_else(|| "0".to_string()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "image" | "image_convert" | "image_compress" | "compress_image" => dispatch(
            &tool_context,
            Command::Image(ImageArgs {
                input: input()?,
                to: request.output_format.clone().or(request.format.clone()),
                output: request.output.clone().map(PathBuf::from),
                width: request.width,
                height: request.height,
                rotate: request.rotate,
                watermark: request.watermark.clone().map(PathBuf::from),
                image_quality: request.image_quality,
            }),
        ),
        "gif" | "video_to_gif" | "gif_convert" => dispatch(
            &tool_context,
            Command::Gif(GifArgs {
                input: input()?,
                start: request.start.clone().unwrap_or_else(|| "0".to_string()),
                duration: request.duration.clone().unwrap_or_else(|| "3".to_string()),
                fps: request.fps.unwrap_or(12),
                width: request.width,
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "edit" | "edit_media" => dispatch(
            &tool_context,
            Command::Edit(EditArgs {
                input: input()?,
                output: request.output.clone().map(PathBuf::from),
                crop: request.crop.clone(),
                rotate: request.rotate,
                speed: request.speed,
                volume: request.volume,
                filter: request.filter.clone(),
                subtitle: request.subtitle.clone().map(PathBuf::from),
                subtitle_style: request.subtitle_style.clone(),
                start: request.start.clone(),
                duration: request.duration.clone(),
            }),
        ),
        "merge" | "concat" | "mux" | "mix" => {
            let mut inputs = Vec::new();
            if let Some(value) = request.input.clone() {
                inputs.push(PathBuf::from(value));
            }
            if let Some(value) = request.inputs.clone() {
                inputs.extend(value.into_iter().map(PathBuf::from));
            }
            dispatch(
                &tool_context,
                Command::Merge(MergeArgs {
                    inputs,
                    mode: request.mode.clone().unwrap_or_else(|| operation.clone()),
                    output: request.output.clone().map(PathBuf::from),
                }),
            )
        }
        "audio" | "audio_convert" | "compress_audio" => dispatch(
            &tool_context,
            Command::Audio(AudioArgs {
                input: input()?,
                format: request.format.clone().unwrap_or_else(|| "m4a".to_string()),
                output: request.output.clone().map(PathBuf::from),
                bitrate: request.bitrate.clone(),
                sample_rate: request.sample_rate,
                channels: request.channels,
                volume: request.volume,
                start: request.start.clone(),
                duration: request.duration.clone(),
            }),
        ),
        "repair" | "repair_media" => dispatch(
            &tool_context,
            Command::Repair(RepairArgs {
                input: input()?,
                output: request.output.clone().map(PathBuf::from),
                reencode: request.reencode.unwrap_or(false),
            }),
        ),
        "disc" | "dvd" | "cd" | "iso" => dispatch(
            &tool_context,
            Command::Disc(DiscArgs {
                input: input()?,
                kind: request.kind.clone().unwrap_or_else(|| default_disc_kind(&operation)),
                action: request.action.clone().unwrap_or_else(|| "extract".to_string()),
                volume_label: request.volume_label.clone(),
                to: request.output_format.clone().or(request.format.clone()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "batch" => dispatch(
            &tool_context,
            Command::Batch(BatchArgs {
                input: input()?.to_string_lossy().to_string(),
                convert: request.output_format.clone(),
                recursive: request.recursive.unwrap_or(false),
                output_dir: request.output_dir.clone().map(PathBuf::from),
            }),
        ),
        "verify" => dispatch(
            &tool_context,
            Command::Verify(VerifyArgs {
                input: input()?,
                output: PathBuf::from(required_string(request.output.clone(), "output")?),
            }),
        ),
        "capabilities" => dispatch(&tool_context, Command::Capabilities),
        "presets" | "device_presets" => dispatch(&tool_context, Command::Presets),
        "ffmpeg" => raw_ffmpeg_command(&tool_context, &request.args.unwrap_or_default()),
        _ => Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("Unsupported Tool operation: {}", request.operation),
        )
        .with_suggestions(&[
            "Use a semantic operation such as inspect_media, plan_media_operation, convert_media, compress_media, resize_media, clip_media, extract_audio, create_thumbnail, image_convert, image_compress, edit_media, merge, audio_convert, repair_media, disc, presets, batch, verify_media, capabilities, or ffmpeg.",
        ])),
    }
}

pub(crate) fn normalize_operation(value: &str) -> String {
    match value.to_lowercase().replace('-', "_").as_str() {
        "inspect_media" => "inspect",
        "plan_media_operation" => "plan",
        "convert_media" => "convert",
        "compress_media" => "compress",
        "resize_media" => "resize",
        "clip_media" => "clip",
        "create_thumbnail" => "thumbnail",
        "verify_media" => "verify",
        operation => operation,
    }
    .to_string()
}

pub(crate) fn raw_ffmpeg_command(context: &Context, args: &[String]) -> Result<Value, AppError> {
    if args.is_empty() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "Raw FFmpeg requires arguments after `media ffmpeg --`.",
        ));
    }
    if context.dry_run {
        return Ok(json!({
            "status": "planned",
            "operation": "ffmpeg",
            "will_execute": false,
            "command": "ffmpeg",
            "args": args,
        }));
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = run_program("ffmpeg", &refs, context.verbose)?;
    Ok(json!({
        "status": "success",
        "operation": "ffmpeg",
        "command": "ffmpeg",
        "args": args,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }))
}

pub(crate) fn required_input(input: Option<String>) -> Result<PathBuf, AppError> {
    Ok(PathBuf::from(required_string(input, "input")?))
}

pub(crate) fn required_string(value: Option<String>, field: &str) -> Result<String, AppError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("INVALID_ARGUMENT", format!("Tool requests require {field}.")))
}

pub(crate) fn default_disc_kind(operation: &str) -> String {
    match operation {
        "dvd" | "cd" | "iso" => operation.to_string(),
        _ => "dvd".to_string(),
    }
}
