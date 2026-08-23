use super::prelude::*;
use super::{
    error::*, execution::*, format::*, hardware::*, model::*, parse::*, paths::*, process::*,
    state::*,
};

pub(crate) fn compress_command(context: &Context, args: &CompressArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let quality = args.quality.unwrap_or(context.default_quality);
    let hardware = args.hardware.unwrap_or(context.default_hardware);
    let probe = probe_media(&args.input, context.verbose)?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let video = first_stream(
        &probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default(),
        "video",
    )
    .ok_or_else(|| AppError::new("INVALID_MEDIA", "No video stream was found."))?;
    let duration = probe.duration_seconds.unwrap_or(0.0);
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let hardware_selection = select_video_hardware(context, hardware, "h264", true)?;
    let software_encoder = if hardware_selection.encoder.is_none() {
        Some(select_software_video_encoder(context, "h264")?)
    } else {
        None
    };
    let selected_encoder = hardware_selection.encoder.as_deref().or(software_encoder.as_deref());
    let mut ffmpeg_args =
        vec!["-map".to_string(), "0:v:0".to_string(), "-map".to_string(), "0:a?".to_string()];
    ffmpeg_args.extend(video_encode_args("h264", quality_name(quality), selected_encoder)?);
    let mut notes = vec![format!(
        "Compressing {} video with the {:?} quality preset.",
        video.get("codec_name").and_then(Value::as_str).unwrap_or("unknown"),
        quality
    )];
    let mut two_pass = false;
    let mut target_size_bytes = None;
    match args.target_size.as_deref().map(parse_size) {
        Some(Ok(target_bytes)) => {
            target_size_bytes = Some(target_bytes);
            if duration <= 0.0 {
                return Err(AppError::new(
                    "INVALID_MEDIA",
                    "Target-size compression requires a known duration.",
                ));
            }
            let audio_bits = 256_000.0 * duration;
            let total_bits = target_bytes as f64 * 8.0 * 0.96;
            let video_bitrate = ((total_bits - audio_bits).max(250_000.0) / duration) as u64;
            ffmpeg_args.extend(["-b:v".to_string(), format!("{video_bitrate}")]);
            ffmpeg_args.extend(["-maxrate".to_string(), format!("{video_bitrate}")]);
            ffmpeg_args
                .extend(["-bufsize".to_string(), format!("{}", video_bitrate.saturating_mul(2))]);
            if software_encoder.is_some() {
                remove_option(&mut ffmpeg_args, "-crf");
            }
            two_pass = hardware_selection.encoder.is_none();
            if two_pass {
                notes.push(
                    "Using two-pass software encoding to improve target-size accuracy.".to_string(),
                );
            }
            notes.push(format!("Target size is approximately {} bytes.", target_bytes));
        }
        Some(Err(error)) => return Err(error),
        None => {
            if hardware_selection.encoder.is_some() {
                ffmpeg_args
                    .extend(["-b:v".to_string(), hardware_quality_bitrate(quality).to_string()]);
            }
        }
    }
    ffmpeg_args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        DEFAULT_AUDIO_BITRATE.to_string(),
    ]);
    ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"compress","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"transcode","quality":quality,"target_size_bytes":target_size_bytes,"passes":if two_pass { 2 } else { 1 },"pass_strategy":if two_pass { "two_pass" } else { "single_pass" },"quality_loss":"video_and_audio","reason":notes,"hardware":{"requested":hardware_selection.requested,"selected":hardware_selection.selected,"encoder":hardware_selection.encoder,"reason":hardware_selection.reason},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "transcode".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    if two_pass {
        execute_two_pass_plan(context, &args.input, &plan)
    } else {
        execute_plan(context, &args.input, &plan)
    }
}

pub(crate) fn resize_command(context: &Context, args: &ResizeArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    if args.width.is_none() && args.resolution.is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", "Provide --width or --resolution."));
    }
    if args.width.is_some() && args.resolution.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Use only one of --width or --resolution."));
    }
    if args.width == Some(0) {
        return Err(AppError::new("INVALID_ARGUMENT", "Resize width must be greater than zero."));
    }
    let height = args.resolution.as_deref().map(parse_resolution).transpose()?;
    let (target_axis, requested_dimension) = if let Some(width) = args.width {
        ("width", width)
    } else {
        (
            "height",
            height.ok_or_else(|| {
                AppError::new("INVALID_ARGUMENT", "Provide --width or --resolution.")
            })?,
        )
    };
    let effective_dimension = even_dimension(requested_dimension)?;
    let filter = if target_axis == "width" {
        format!("scale={effective_dimension}:-2")
    } else {
        format!("scale=-2:{effective_dimension}")
    };
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let software_encoder = select_software_video_encoder(context, "h264")?;
    let mut warnings = subtitle_warnings(&streams, "mp4");
    if requested_dimension != effective_dimension {
        warnings.push(format!(
            "Requested {target_axis} {requested_dimension} was rounded to {effective_dimension} for an even encoder-compatible dimension."
        ));
    }
    let mut ffmpeg_args = vec![
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
        "-vf".to_string(),
        filter.clone(),
    ];
    ffmpeg_args.extend(video_encode_args("h264", "high", Some(&software_encoder))?);
    ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"resize","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"video_transcode","filter":filter,"target_dimension":{"axis":target_axis,"requested":requested_dimension,"effective":effective_dimension},"preserve_aspect_ratio":true,"even_dimensions":true,"quality_loss":"video_only","hardware":{"requested":"cpu","selected":"cpu","encoder":null,"reason":"Resize uses deterministic software filtering."},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":warnings,"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "video_transcode".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_plan(context, &args.input, &plan)
}

pub(crate) fn clip_command(context: &Context, args: &ClipArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    if args.duration.is_none() && args.end.is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", "Provide --duration or --end."));
    }
    if args.duration.is_some() && args.end.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Use only one of --duration or --end."));
    }
    let start_seconds = parse_time_seconds(&args.start)?;
    if start_seconds < 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "Clip start must not be negative."));
    }
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Clip duration must be greater than zero.",
            ));
        }
    }
    if let Some(end) = &args.end {
        if parse_time_seconds(end)? <= start_seconds {
            return Err(AppError::new("INVALID_ARGUMENT", "Clip end must be after start."));
        }
    }
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source_container = probe
        .raw
        .get("format")
        .and_then(|format| format.get("format_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown");
    let source_container = internal_container(&args.input, source_container);
    let source_video_codec = first_stream(&streams, "video")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let source_audio_codec = first_stream(&streams, "audio")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let copy_compatible = start_seconds == 0.0
        && source_container == "mp4"
        && is_video_compatible("mp4", &source_video_codec)
        && (source_audio_codec == "unknown" || is_audio_compatible("mp4", &source_audio_codec));
    let software_encoder =
        if copy_compatible { None } else { Some(select_software_video_encoder(context, "h264")?) };
    let mut ffmpeg_args = Vec::new();
    if !copy_compatible {
        ffmpeg_args.extend([
            "-ss".to_string(),
            args.start.clone(),
            "-i".to_string(),
            args.input.to_string_lossy().to_string(),
            "-map".to_string(),
            "0:v:0".to_string(),
            "-map".to_string(),
            "0:a?".to_string(),
        ]);
    } else {
        ffmpeg_args.extend(["-map".to_string(), "0".to_string()]);
    }
    if let Some(duration) = &args.duration {
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    } else if let Some(end) = &args.end {
        let duration = parse_time_seconds(end)? - start_seconds;
        ffmpeg_args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    if copy_compatible {
        ffmpeg_args.extend(["-c".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(video_encode_args("h264", "high", software_encoder.as_deref())?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    }
    if copy_compatible {
        ffmpeg_args.extend(subtitle_codec_args("mp4", &streams));
    } else {
        ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    }
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let strategy = if copy_compatible { "stream_copy" } else { "precise_transcode" };
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"clip","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":strategy,"start":args.start,"duration":args.duration,"end":args.end,"quality_loss":if copy_compatible { "none" } else { "video_and_audio" },"reason":if copy_compatible { "Start is at zero and source streams are compatible with MP4; stream copy avoids re-encoding." } else { "Precise clipping re-encodes to honor the requested boundary." },"hardware":{"requested":"cpu","selected":if copy_compatible { "not_applicable" } else { "cpu" },"encoder":null,"reason":if copy_compatible { "Stream copy avoids video encoding." } else { "Precise clipping uses deterministic software encoding." }},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: strategy.to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}

pub(crate) fn extract_audio_command(
    context: &Context,
    args: &ExtractAudioArgs,
) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let format = normalize_audio_format(&args.format)?;
    let audio_extension = audio_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &audio_extension)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "audio").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "No audio stream was found."));
    }
    let source_audio_codec = first_stream(&streams, "audio")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let target_audio_codec = audio_codec_for_format(&format);
    let copy_audio = audio_copy_compatible(&source_audio_codec, &format);
    let mut ffmpeg_args = vec!["-map".to_string(), "0:a:0".to_string(), "-vn".to_string()];
    if copy_audio {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(target_audio_codec, DEFAULT_AUDIO_BITRATE)?);
    }
    ffmpeg_args.extend(audio_container_args(&format));
    ffmpeg_args.extend(["-map_metadata".to_string(), "0".to_string()]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"extract_audio","input":absolute_display(&args.input),"output":absolute_display(&output),"format":format,"source_codec":source_audio_codec,"target_codec":target_audio_codec,"strategy":if copy_audio { "copy" } else { "transcode" },"quality_loss":if copy_audio { "none" } else { "audio_only" },"hardware":{"requested":"cpu","selected":"not_applicable","encoder":null,"reason":"Audio extraction does not use video hardware."},"metadata":{"action":"preserve"},"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: if copy_audio { "copy".to_string() } else { "transcode".to_string() },
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}

pub(crate) fn thumbnail_command(
    context: &Context,
    args: &ThumbnailArgs,
) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let at = parse_thumbnail_time(&args.at, probe.duration_seconds)?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "jpg")?;
    let ffmpeg_args = vec![
        "-ss".to_string(),
        at.clone(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-q:v".to_string(),
        "2".to_string(),
    ];
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"thumbnail","input":absolute_display(&args.input),"output":absolute_display(&output),"at":at,"format":"jpg","hardware":{"requested":"cpu","selected":"not_applicable","encoder":null,"reason":"Thumbnail extraction uses the software image pipeline."},"metadata":{"action":"not_applicable"},"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "frame_extract".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}
