use super::prelude::*;
use super::{
    audio::*, basic::*, disc::*, edit::*, error::*, execution::*, format::*, hardware::*, image::*,
    merge::*, model::*, paths::*, presets::*, process::*, state::*, tool::*,
};

pub(crate) fn plan_command(context: &Context, args: &PlanArgs) -> Result<Value, AppError> {
    let planning_context = Context { dry_run: true, ..context.clone() };
    let operation = args.operation.as_deref().map(normalize_operation).unwrap_or_else(|| {
        if args.target_size.is_some()
            || (args.quality.is_some()
                && args.to.is_none()
                && args.video_codec.is_none()
                && args.audio_codec.is_none())
        {
            "compress".to_string()
        } else if args.width.is_some() || args.resolution.is_some() {
            "resize".to_string()
        } else if args.start.is_some() {
            "clip".to_string()
        } else if args.format.is_some() {
            "extract_audio".to_string()
        } else if args.at.is_some() {
            "thumbnail".to_string()
        } else {
            "convert".to_string()
        }
    });
    match operation.as_str() {
        "compress" => {
            let quality = args.quality.unwrap_or(planning_context.default_quality);
            return compress_command(
                &planning_context,
                &CompressArgs {
                    input: args.input.clone(),
                    quality: Some(quality),
                    target_size: args.target_size.clone(),
                    output: args.output.clone(),
                    hardware: args.hardware,
                },
            );
        }
        "resize" => {
            return resize_command(
                &planning_context,
                &ResizeArgs {
                    input: args.input.clone(),
                    width: args.width,
                    resolution: args.resolution.clone(),
                    output: args.output.clone(),
                },
            );
        }
        "clip" => {
            return clip_command(
                &planning_context,
                &ClipArgs {
                    input: args.input.clone(),
                    start: args.start.clone().unwrap_or_else(|| "0".to_string()),
                    duration: args.duration.clone(),
                    end: args.end.clone(),
                    output: args.output.clone(),
                },
            );
        }
        "extract_audio" => {
            return extract_audio_command(
                &planning_context,
                &ExtractAudioArgs {
                    input: args.input.clone(),
                    format: args.format.clone().unwrap_or_else(|| "m4a".to_string()),
                    output: args.output.clone(),
                },
            );
        }
        "thumbnail" => {
            return thumbnail_command(
                &planning_context,
                &ThumbnailArgs {
                    input: args.input.clone(),
                    at: args.at.clone().unwrap_or_else(|| "0".to_string()),
                    output: args.output.clone(),
                },
            );
        }
        "image" | "image_convert" | "image_compress" | "compress_image" => {
            return image_command(
                &planning_context,
                &ImageArgs {
                    input: args.input.clone(),
                    to: args.to.clone().or(args.format.clone()),
                    output: args.output.clone(),
                    width: args.width,
                    height: args.height,
                    rotate: args.rotate,
                    watermark: args.watermark.clone(),
                    image_quality: args.image_quality,
                },
            );
        }
        "gif" | "video_to_gif" | "gif_convert" => {
            return gif_command(
                &planning_context,
                &GifArgs {
                    input: args.input.clone(),
                    start: args.start.clone().unwrap_or_else(|| "0".to_string()),
                    duration: args.duration.clone().unwrap_or_else(|| "3".to_string()),
                    fps: args.fps.unwrap_or(12),
                    width: args.width,
                    output: args.output.clone(),
                },
            );
        }
        "edit" | "edit_media" => {
            return edit_command(
                &planning_context,
                &EditArgs {
                    input: args.input.clone(),
                    output: args.output.clone(),
                    crop: args.crop.clone(),
                    rotate: args.rotate,
                    speed: args.speed,
                    volume: args.volume,
                    filter: args.filter.clone(),
                    subtitle: args.subtitle.clone(),
                    subtitle_style: args.subtitle_style.clone(),
                    start: args.start.clone(),
                    duration: args.duration.clone(),
                },
            );
        }
        "merge" | "concat" | "mux" | "mix" => {
            let mut inputs = vec![args.input.clone()];
            inputs.extend(args.inputs.clone().unwrap_or_default());
            let merge_mode = match operation.as_str() {
                "mux" | "mix" | "concat" => operation.clone(),
                _ => args.mode.clone(),
            };
            return merge_command(
                &planning_context,
                &MergeArgs { inputs, mode: merge_mode, output: args.output.clone() },
            );
        }
        "audio" | "audio_convert" | "compress_audio" => {
            return audio_command(
                &planning_context,
                &AudioArgs {
                    input: args.input.clone(),
                    format: args
                        .format
                        .clone()
                        .or(args.to.clone())
                        .unwrap_or_else(|| "m4a".to_string()),
                    output: args.output.clone(),
                    bitrate: args.bitrate.clone(),
                    sample_rate: args.sample_rate,
                    channels: args.channels,
                    volume: args.volume,
                    start: args.start.clone(),
                    duration: args.duration.clone(),
                },
            );
        }
        "repair" | "repair_media" => {
            return repair_command(
                &planning_context,
                &RepairArgs {
                    input: args.input.clone(),
                    output: args.output.clone(),
                    reencode: args.reencode,
                },
            );
        }
        "disc" | "dvd" | "cd" | "iso" => {
            return disc_command(
                &planning_context,
                &DiscArgs {
                    input: args.input.clone(),
                    kind: args.kind.clone().unwrap_or_else(|| default_disc_kind(&operation)),
                    action: args.action.clone(),
                    volume_label: args.volume_label.clone(),
                    to: args.to.clone().or(args.format.clone()),
                    output: args.output.clone(),
                },
            );
        }
        "convert" => {}
        _ => {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                format!("Planning operation {operation} is not supported."),
            )
            .with_suggestions(&[
                "Use convert, compress, resize, clip, extract_audio, thumbnail, image, edit, merge, audio, repair, or disc.",
            ]));
        }
    }
    let profile = args.device.as_deref().map(device_profile).transpose()?;
    let target_container = args.to.as_deref().or(profile.map(|profile| profile.container));
    let target_video_codec = args
        .video_codec
        .as_deref()
        .or(profile.map(|profile| profile.video_codec))
        .unwrap_or(&planning_context.default_video_codec);
    let target_audio_codec = args
        .audio_codec
        .as_deref()
        .or(profile.map(|profile| profile.audio_codec))
        .unwrap_or(&planning_context.default_audio_codec);
    let mut plan = build_convert_plan(
        &planning_context,
        &args.input,
        target_container,
        args.output.as_deref(),
        target_video_codec,
        target_audio_codec,
        args.hardware.unwrap_or(planning_context.default_hardware),
        args.quality.unwrap_or(planning_context.default_quality),
    )?;
    if let Some(profile) = profile {
        apply_device_profile(&mut plan, profile);
    }
    let mut value = plan.value;
    if let Some(object) = value.as_object_mut() {
        object.insert("status".to_string(), json!("planned"));
        object.insert("will_execute".to_string(), json!(false));
    }
    Ok(value)
}

pub(crate) fn convert_command(context: &Context, args: &ConvertArgs) -> Result<Value, AppError> {
    let profile = args.device.as_deref().map(device_profile).transpose()?;
    let target_container = args.to.as_deref().or(profile.map(|profile| profile.container));
    let target_video_codec = args
        .video_codec
        .as_deref()
        .or(profile.map(|profile| profile.video_codec))
        .unwrap_or(&context.default_video_codec);
    let target_audio_codec = args
        .audio_codec
        .as_deref()
        .or(profile.map(|profile| profile.audio_codec))
        .unwrap_or(&context.default_audio_codec);
    let mut plan = build_convert_plan(
        context,
        &args.input,
        target_container,
        args.output.as_deref(),
        target_video_codec,
        target_audio_codec,
        args.hardware.unwrap_or(context.default_hardware),
        args.quality.unwrap_or(context.default_quality),
    )?;
    if let Some(profile) = profile {
        apply_device_profile(&mut plan, profile);
    }
    if context.dry_run {
        let mut value = plan.value;
        if let Some(object) = value.as_object_mut() {
            object.insert("status".to_string(), json!("planned"));
            object.insert("will_execute".to_string(), json!(false));
        }
        return Ok(value);
    }
    execute_plan(context, &args.input, &plan)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_convert_plan(
    context: &Context,
    input: &Path,
    to: Option<&str>,
    output: Option<&Path>,
    video_codec: &str,
    audio_codec: &str,
    hardware: HardwareMode,
    quality: Quality,
) -> Result<OperationPlan, AppError> {
    let video_codec = video_codec.to_lowercase();
    let audio_codec = audio_codec.to_lowercase();
    ensure_input(input)?;
    let probe = probe_media(input, context.verbose)?;
    let format = probe.raw.get("format").cloned().unwrap_or_else(|| json!({}));
    let source_container = format
        .get("format_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_lowercase();
    let source_container = internal_container(input, &source_container);
    let target_container = normalize_container(to.unwrap_or_else(|| {
        output
            .and_then(|path| path.extension().and_then(OsStr::to_str))
            .unwrap_or(&source_container)
    }))?;
    let target_video_codec =
        preferred_codec(&video_codec, default_video_codec_for_container(&target_container));
    let target_audio_codec =
        preferred_codec(&audio_codec, default_audio_codec_for_container(&target_container));
    let target_path = resolve_output(context, input, output, &target_container)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source_video = first_stream(&streams, "video");
    let source_audio = first_stream(&streams, "audio");
    let source_video_codec = source_video
        .as_ref()
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_lowercase();
    let source_audio_codec = source_audio
        .as_ref()
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_lowercase();
    let source_video_compatible = is_video_compatible(&target_container, &source_video_codec);
    let source_audio_compatible = is_audio_compatible(&target_container, &source_audio_codec);
    if video_codec == "copy" && !source_video_compatible {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            format!(
                "Cannot copy {} video into {}.",
                display_codec(&source_video_codec),
                target_container.to_uppercase()
            ),
        )
        .with_details(json!({
            "stream": "video",
            "codec": source_video_codec,
            "container": target_container,
        }))
        .with_suggestions(&[
            "Use --video-codec auto to let MediaForge choose copy or transcode.",
            "Choose a compatible target container.",
        ]));
    }
    if audio_codec == "copy"
        && first_stream(&streams, "audio").is_some()
        && !source_audio_compatible
    {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            format!(
                "Cannot copy {} audio into {}.",
                display_codec(&source_audio_codec),
                target_container.to_uppercase()
            ),
        )
        .with_details(json!({
            "stream": "audio",
            "codec": source_audio_codec,
            "container": target_container,
        }))
        .with_suggestions(&[
            "Use --audio-codec auto to let MediaForge choose copy or transcode.",
            "Choose a compatible target container.",
        ]));
    }
    let video_compatible =
        (video_codec == "auto" || video_codec == "copy") && source_video_compatible;
    let audio_compatible =
        (audio_codec == "auto" || audio_codec == "copy") && source_audio_compatible;
    let video_action = if video_compatible { "copy" } else { "transcode" };
    let audio_action = if audio_compatible { "copy" } else { "transcode" };
    validate_transcode_compatibility(
        "video",
        video_action,
        &target_video_codec,
        &target_container,
    )?;
    validate_transcode_compatibility(
        "audio",
        audio_action,
        &target_audio_codec,
        &target_container,
    )?;
    let hardware_selection =
        select_video_hardware(context, hardware, &target_video_codec, video_action == "transcode")?;
    let software_encoder = if video_action == "transcode" && hardware_selection.encoder.is_none() {
        Some(select_software_video_encoder(context, &target_video_codec)?)
    } else {
        None
    };
    let selected_encoder = hardware_selection.encoder.as_deref().or(software_encoder.as_deref());
    let strategy = match (video_action, audio_action) {
        ("copy", "copy") if source_container == target_container => "copy",
        ("copy", "copy") => "remux",
        ("copy", "transcode") | ("transcode", "copy") => "partial_transcode",
        _ => "transcode",
    };
    let mut reasons = Vec::new();
    if video_action == "copy" {
        reasons.push(format!(
            "{} video is compatible with {}.",
            display_codec(&source_video_codec),
            target_container.to_uppercase()
        ));
    } else if video_codec != "auto" {
        reasons.push(format!("Video codec was explicitly requested as {}.", video_codec));
    } else {
        reasons.push(format!(
            "{} video is not compatible with {}.",
            display_codec(&source_video_codec),
            target_container.to_uppercase()
        ));
    }
    if audio_action == "copy" {
        reasons.push(format!(
            "{} audio is compatible with {}.",
            display_codec(&source_audio_codec),
            target_container.to_uppercase()
        ));
    } else if audio_codec != "auto" {
        reasons.push(format!("Audio codec was explicitly requested as {}.", audio_codec));
    } else {
        reasons.push(format!(
            "{} audio is not compatible with {}.",
            display_codec(&source_audio_codec),
            target_container.to_uppercase()
        ));
    }
    let quality_loss = match (video_action, audio_action) {
        ("copy", "copy") => "none",
        ("copy", "transcode") => "audio_only",
        ("transcode", "copy") => "video_only",
        _ => "video_and_audio",
    };
    let mut ffmpeg_args = vec!["-map".to_string(), "0".to_string()];
    if video_action == "copy" {
        ffmpeg_args.extend(["-c:v".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(video_encode_args(
            &target_video_codec,
            quality_name(quality),
            selected_encoder,
        )?);
    }
    if audio_action == "copy" {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(&target_audio_codec, DEFAULT_AUDIO_BITRATE)?);
    }
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    ffmpeg_args.extend(subtitle_codec_args(&target_container, &streams));
    let plan = json!({
        "status": "success",
        "operation": "convert",
        "input": absolute_display(input),
        "output": absolute_display(&target_path),
        "strategy": strategy,
        "quality": quality,
        "video": {"action": video_action, "codec": if video_action == "copy" { source_video_codec.clone() } else { target_video_codec }, "encoder": if video_action == "transcode" { selected_encoder } else { None::<&str> }},
        "audio": {"action": audio_action, "from": source_audio_codec, "to": if audio_action == "copy" { Value::Null } else { json!(target_audio_codec) }},
        "subtitle": {"action": subtitle_strategy(&target_container, &streams)},
        "metadata": {"action": "preserve"},
        "hardware": {"requested": hardware_selection.requested, "selected": hardware_selection.selected, "encoder": hardware_selection.encoder, "reason": hardware_selection.reason},
        "quality_loss": quality_loss,
        "reason": reasons,
        "warnings": subtitle_warnings(&streams, &target_container),
        "ffmpeg_args": ffmpeg_args,
    });
    Ok(OperationPlan {
        value: plan,
        output: target_path,
        args: ffmpeg_args,
        strategy: strategy.to_string(),
    })
}
