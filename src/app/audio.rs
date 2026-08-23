use super::prelude::*;
use super::{
    error::*, execution::*, format::*, model::*, parse::*, paths::*, process::*, state::*,
};

pub(crate) fn validate_bitrate(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("INVALID_ARGUMENT", "Audio bitrate cannot be empty."));
    }
    let (number, suffix) = trimmed
        .strip_suffix('k')
        .map(|value| (value, "k"))
        .or_else(|| trimmed.strip_suffix('K').map(|value| (value, "k")))
        .or_else(|| trimmed.strip_suffix('M').map(|value| (value, "M")))
        .or_else(|| trimmed.strip_suffix('m').map(|value| (value, "M")))
        .unwrap_or((trimmed, ""));
    if number.parse::<f64>().ok().filter(|number| number.is_finite() && *number > 0.0).is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", format!("Invalid audio bitrate: {value}")));
    }
    Ok(format!("{number}{suffix}"))
}

pub(crate) fn audio_command(context: &Context, args: &AudioArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let format = normalize_audio_format(&args.format)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source = first_stream(&streams, "audio")
        .ok_or_else(|| AppError::new("INVALID_MEDIA", "No audio stream was found."))?;
    let source_codec =
        source.get("codec_name").and_then(Value::as_str).unwrap_or("unknown").to_lowercase();
    if let Some(rate) = args.sample_rate {
        if rate == 0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Sample rate must be greater than zero.",
            ));
        }
    }
    if let Some(channels) = args.channels {
        if channels == 0 {
            return Err(AppError::new("INVALID_ARGUMENT", "Channels must be greater than zero."));
        }
    }
    if let Some(volume) = args.volume {
        if !(0.0..=10.0).contains(&volume) {
            return Err(AppError::new("INVALID_ARGUMENT", "Volume must be between 0 and 10."));
        }
    }
    let bitrate = args
        .bitrate
        .as_deref()
        .map(validate_bitrate)
        .transpose()?
        .unwrap_or_else(|| DEFAULT_AUDIO_BITRATE.to_string());
    let audio_extension = audio_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &audio_extension)?;
    let target_codec = audio_codec_for_format(&format);
    let copy_audio = args.bitrate.is_none()
        && args.sample_rate.is_none()
        && args.channels.is_none()
        && args.volume.is_none()
        && args.start.is_none()
        && args.duration.is_none()
        && audio_copy_compatible(&source_codec, &format);
    let mut ffmpeg_args = vec![
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:a:0".to_string(),
        "-vn".to_string(),
    ];
    if let Some(start) = &args.start {
        parse_time_seconds(start)?;
        ffmpeg_args.extend(["-ss".to_string(), start.clone()]);
    }
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Audio duration must be greater than zero.",
            ));
        }
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    }
    if copy_audio {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(target_codec, &bitrate)?);
    }
    if let Some(rate) = args.sample_rate {
        ffmpeg_args.extend(["-ar".to_string(), rate.to_string()]);
    }
    if let Some(channels) = args.channels {
        ffmpeg_args.extend(["-ac".to_string(), channels.to_string()]);
    }
    if let Some(volume) = args.volume {
        ffmpeg_args.extend(["-af".to_string(), format!("volume={volume:.6}")]);
    }
    ffmpeg_args.extend(audio_container_args(&format));
    ffmpeg_args.push("-map_metadata".to_string());
    ffmpeg_args.push("0".to_string());
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "audio",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "source_codec": source_codec,
            "target_codec": target_codec,
            "strategy": if copy_audio { "copy" } else { "transcode" },
            "bitrate": bitrate,
            "sample_rate": args.sample_rate,
            "channels": args.channels,
            "volume": args.volume,
            "quality_loss": if copy_audio { "none" } else { "audio_only" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: if copy_audio { "copy".to_string() } else { "transcode".to_string() },
    };
    finish_custom_plan(context, &args.input, plan)
}
