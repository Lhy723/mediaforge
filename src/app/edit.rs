use super::prelude::*;
use super::{
    error::*, execution::*, format::*, image::*, model::*, parse::*, paths::*, process::*, state::*,
};

pub(crate) fn parse_crop(value: &str) -> Result<String, AppError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 4 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Crop must use WIDTH:HEIGHT:X:Y."));
    }
    for part in &parts {
        if part.parse::<u32>().is_err() {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Crop dimensions and offsets must be non-negative integers.",
            ));
        }
    }
    Ok(format!("crop={}", parts.join(":")))
}

pub(crate) fn named_video_filter(value: &str) -> Result<&'static str, AppError> {
    match value.to_lowercase().as_str() {
        "grayscale" | "gray" => Ok("hue=s=0"),
        "blur" => Ok("boxblur=2:1"),
        "sharpen" => Ok("unsharp=5:5:1.0:5:5:0.0"),
        "vintage" => Ok("curves=vintage"),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported named filter: {other}")))
        }
    }
}

pub(crate) fn atempo_filter(speed: f64) -> String {
    let mut value = speed;
    let mut filters = Vec::new();
    while value < 0.5 {
        filters.push("atempo=0.5".to_string());
        value /= 0.5;
    }
    while value > 2.0 {
        filters.push("atempo=2.0".to_string());
        value /= 2.0;
    }
    filters.push(format!("atempo={value:.6}"));
    filters.join(",")
}

pub(crate) fn escape_filter_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace(':', "\\:").replace('\'', "\\'")
}

pub(crate) fn subtitle_filter(path: &Path, style: Option<&str>) -> Result<String, AppError> {
    let mut filter = format!("subtitles={}", escape_filter_path(path));
    if let Some(style) = style {
        let style = style.trim();
        if style.is_empty() || !style.contains('=') {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Subtitle style must contain comma-separated key=value pairs.",
            ));
        }
        if style.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || " =,.:_&#%+-/".contains(character))
        }) {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Subtitle style contains unsupported filter characters.",
            )
            .with_suggestions(&[
                "Use values such as FontName=Arial,FontSize=24,PrimaryColour=&H00FFFFFF.",
            ]));
        }
        filter.push_str(":force_style='");
        filter.push_str(style);
        filter.push('\'');
    }
    Ok(filter)
}

pub(crate) fn ffmpeg_filter_available(context: &Context, name: &str) -> bool {
    run_program("ffmpeg", &["-hide_banner", "-filters"], context.verbose).ok().is_some_and(
        |result| {
            result.stdout.lines().any(|line| line.split_whitespace().any(|token| token == name))
        },
    )
}

pub(crate) fn edit_command(context: &Context, args: &EditArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "video").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "Edit requires a video stream."));
    }
    if let Some(speed) = args.speed {
        if !(0.25..=4.0).contains(&speed) {
            return Err(AppError::new("INVALID_ARGUMENT", "Speed must be between 0.25 and 4.0."));
        }
    }
    if let Some(volume) = args.volume {
        if !(0.0..=10.0).contains(&volume) {
            return Err(AppError::new("INVALID_ARGUMENT", "Volume must be between 0 and 10."));
        }
    }
    if let Some(subtitle) = &args.subtitle {
        ensure_input(subtitle)?;
    }
    if args.subtitle.is_none() && args.subtitle_style.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Subtitle style requires --subtitle."));
    }
    let subtitle_filter_available =
        args.subtitle.as_ref().is_none_or(|_| ffmpeg_filter_available(context, "subtitles"));
    if args.subtitle.is_some() && !subtitle_filter_available && !context.dry_run {
        return Err(AppError::new(
            "FILTER_UNAVAILABLE",
            "The installed FFmpeg build does not include the subtitles/libass filter.",
        )
        .with_details(json!({"filter":"subtitles","subtitle":args.subtitle.as_ref().map(|path| absolute_display(path))}))
        .with_suggestions(&[
            "Install an FFmpeg build compiled with libass, then retry.",
            "Use a subtitle stream conversion operation when burn-in is not required.",
        ]));
    }
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let mut video_filters = Vec::new();
    if let Some(crop) = &args.crop {
        video_filters.push(parse_crop(crop)?);
    }
    if let Some(rotate) = args.rotate {
        video_filters.push(rotate_filter(rotate)?.to_string());
    }
    if let Some(filter) = &args.filter {
        video_filters.push(named_video_filter(filter)?.to_string());
    }
    if let Some(speed) = args.speed {
        video_filters.push(format!("setpts=PTS/{speed:.6}"));
    }
    if let Some(subtitle) = &args.subtitle {
        video_filters.push(subtitle_filter(subtitle, args.subtitle_style.as_deref())?);
    }
    let mut ffmpeg_args = Vec::new();
    if let Some(start) = &args.start {
        parse_time_seconds(start)?;
        ffmpeg_args.extend(["-ss".to_string(), start.clone()]);
    }
    ffmpeg_args.extend(["-i".to_string(), args.input.to_string_lossy().to_string()]);
    ffmpeg_args.extend([
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
    ]);
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Edit duration must be greater than zero.",
            ));
        }
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    }
    if !video_filters.is_empty() {
        ffmpeg_args.extend(["-vf".to_string(), video_filters.join(",")]);
    }
    ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
    if args.speed.is_some() || args.volume.is_some() {
        let mut audio_filters = Vec::new();
        if let Some(speed) = args.speed {
            audio_filters.push(atempo_filter(speed));
        }
        if let Some(volume) = args.volume {
            audio_filters.push(format!("volume={volume:.6}"));
        }
        ffmpeg_args.extend(["-af".to_string(), audio_filters.join(",")]);
    }
    ffmpeg_args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        DEFAULT_AUDIO_BITRATE.to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "edit",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "strategy": "filter_transcode",
            "crop": args.crop,
            "rotate": args.rotate,
            "speed": args.speed,
            "volume": args.volume,
            "filter": args.filter,
            "subtitle": args.subtitle.as_ref().map(|path| absolute_display(path)),
            "subtitle_style": args.subtitle_style,
            "warnings": if args.subtitle.is_some() && !subtitle_filter_available {
                vec!["The current FFmpeg build lacks the subtitles/libass filter; execution is unavailable.".to_string()]
            } else {
                Vec::new()
            },
            "audio_present": first_stream(&streams, "audio").is_some(),
            "quality_loss": "video_and_audio",
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "filter_transcode".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}
