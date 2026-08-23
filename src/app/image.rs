use super::prelude::*;
use super::{
    error::*, execution::*, format::*, model::*, parse::*, paths::*, process::*, state::*,
};

pub(crate) fn normalize_image_format(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    let normalized = match value.as_str() {
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "webp" => "webp",
        "gif" => "gif",
        "bmp" => "bmp",
        "tif" | "tiff" => "tiff",
        "ico" => "ico",
        "tga" => "tga",
        "avif" => "avif",
        _ => {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                format!("Unsupported image format: {value}"),
            ))
        }
    };
    Ok(normalized.to_string())
}

pub(crate) fn image_output_extension(format: &str) -> String {
    match format {
        "jpg" => "jpg".to_string(),
        "tiff" => "tiff".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn validate_positive_dimension(value: Option<u32>, field: &str) -> Result<(), AppError> {
    if value == Some(0) {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("{field} must be greater than zero."),
        ));
    }
    Ok(())
}

pub(crate) fn rotate_filter(value: u16) -> Result<&'static str, AppError> {
    match value {
        90 => Ok("transpose=1"),
        180 => Ok("hflip,vflip"),
        270 => Ok("transpose=2"),
        _ => Err(AppError::new("INVALID_ARGUMENT", "Rotation must be 90, 180, or 270 degrees.")),
    }
}

pub(crate) fn image_quality_args(format: &str, quality: u8) -> Vec<String> {
    match format {
        "jpg" => {
            let quantizer = 31_u16.saturating_sub((quality as u16 * 30) / 100).max(1);
            vec!["-q:v".to_string(), quantizer.to_string()]
        }
        "webp" | "avif" => vec!["-q:v".to_string(), quality.to_string()],
        "png" => {
            let compression = ((100_u16.saturating_sub(quality as u16)) / 12).min(9);
            vec!["-compression_level".to_string(), compression.to_string()]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn image_codec_args(format: &str) -> Vec<String> {
    let codec = match format {
        "jpg" => "mjpeg",
        "png" => "png",
        "webp" => "libwebp",
        "gif" => "gif",
        "bmp" => "bmp",
        "tiff" => "tiff",
        "ico" => "bmp",
        "tga" => "targa",
        "avif" => "libaom-av1",
        _ => return Vec::new(),
    };
    vec!["-c:v".to_string(), codec.to_string()]
}

pub(crate) fn image_command(context: &Context, args: &ImageArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    validate_positive_dimension(args.width, "Image width")?;
    validate_positive_dimension(args.height, "Image height")?;
    if let Some(quality) = args.image_quality {
        if !(1..=100).contains(&quality) {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Image quality must be between 1 and 100.",
            ));
        }
    }
    if let Some(watermark) = &args.watermark {
        ensure_input(watermark)?;
    }
    let format = normalize_image_format(
        args.to
            .as_deref()
            .or_else(|| {
                args.output.as_deref().and_then(|path| path.extension().and_then(OsStr::to_str))
            })
            .unwrap_or_else(|| args.input.extension().and_then(OsStr::to_str).unwrap_or("png")),
    )?;
    let image_extension = image_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &image_extension)?;
    let mut filters = Vec::new();
    match (args.width, args.height) {
        (Some(width), Some(height)) => filters.push(format!("scale={width}:{height}")),
        (Some(width), None) => filters.push(format!("scale={width}:-1")),
        (None, Some(height)) => filters.push(format!("scale=-1:{height}")),
        (None, None) => {}
    }
    if let Some(rotate) = args.rotate {
        filters.push(rotate_filter(rotate)?.to_string());
    }
    let mut ffmpeg_args = vec!["-i".to_string(), args.input.to_string_lossy().to_string()];
    if let Some(watermark) = &args.watermark {
        ffmpeg_args.extend(["-i".to_string(), watermark.to_string_lossy().to_string()]);
        let base = if filters.is_empty() {
            "[0:v]".to_string()
        } else {
            format!("[0:v]{}[base]", filters.join(","))
        };
        let overlay_input = if filters.is_empty() { "[0:v]" } else { "[base]" };
        let filter_complex = if filters.is_empty() {
            "[0:v][1:v]overlay=W-w-16:H-h-16[v]".to_string()
        } else {
            format!("{base};{overlay_input}[1:v]overlay=W-w-16:H-h-16[v]")
        };
        ffmpeg_args.extend([
            "-filter_complex".to_string(),
            filter_complex,
            "-map".to_string(),
            "[v]".to_string(),
        ]);
    } else if !filters.is_empty() {
        ffmpeg_args.extend(["-vf".to_string(), filters.join(",")]);
    }
    ffmpeg_args.push("-frames:v".to_string());
    ffmpeg_args.push("1".to_string());
    ffmpeg_args.extend(image_codec_args(&format));
    ffmpeg_args.extend(image_quality_args(&format, args.image_quality.unwrap_or(90)));
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "image",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "resize": {"width": args.width, "height": args.height},
            "rotate": args.rotate,
            "watermark": args.watermark.as_ref().map(|path| absolute_display(path)),
            "quality": args.image_quality.unwrap_or(90),
            "strategy": "image_transcode",
            "quality_loss": if args.image_quality.is_some() { "possible" } else { "none" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "image_transcode".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}

pub(crate) fn gif_command(context: &Context, args: &GifArgs) -> Result<Value, AppError> {
    const MAX_GIF_DURATION_SECONDS: f64 = 600.0;
    const MAX_GIF_WIDTH: u32 = 16_384;
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "video").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "GIF conversion requires a video stream."));
    }
    if !(1..=60).contains(&args.fps) {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF FPS must be between 1 and 60."));
    }
    let start_seconds = parse_time_seconds(&args.start)?;
    if !start_seconds.is_finite() {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF start must be a finite timestamp."));
    }
    if start_seconds < 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF start must not be negative."));
    }
    let duration_seconds = parse_time_seconds(&args.duration)?;
    if !duration_seconds.is_finite() {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF duration must be a finite value."));
    }
    if duration_seconds <= 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF duration must be greater than zero."));
    }
    if duration_seconds > MAX_GIF_DURATION_SECONDS {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("GIF duration must not exceed {MAX_GIF_DURATION_SECONDS:.0} seconds."),
        )
        .with_suggestions(&["Use a shorter clip or split a long animation into multiple GIFs."]));
    }
    if args.width.is_some_and(|width| width > MAX_GIF_WIDTH) {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("GIF width must not exceed {MAX_GIF_WIDTH} pixels."),
        ));
    }
    validate_positive_dimension(args.width, "GIF width")?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "gif")?;
    let mut filters = vec![format!("fps={}", args.fps)];
    if let Some(width) = args.width {
        filters.push(format!("scale={width}:-1:flags=lanczos"));
    }
    let filter = format!(
        "{},split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=sierra2_4a",
        filters.join(",")
    );
    let ffmpeg_args = vec![
        "-ss".to_string(),
        args.start.clone(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-t".to_string(),
        args.duration.clone(),
        "-an".to_string(),
        "-vf".to_string(),
        filter.clone(),
        "-loop".to_string(),
        "0".to_string(),
        "-f".to_string(),
        "gif".to_string(),
    ];
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "gif",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "start": args.start,
            "duration": args.duration,
            "fps": args.fps,
            "width": args.width,
            "strategy": "palette_gif",
            "quality_loss": "video_only",
            "filter": filter,
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "palette_gif".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}
