use super::prelude::*;
use super::{error::*, format::*, paths::*, process::*, state::*};

pub(crate) fn inspect_command(context: &Context, input: &Path) -> Result<Value, AppError> {
    let probe = probe_media(input, context.verbose)?;
    let file = fs::metadata(input).map_err(|error| {
        AppError::from_io("FILE_NOT_FOUND", format!("Cannot read {}", input.display()), error)
    })?;
    let format = probe.raw.get("format").cloned().unwrap_or_else(|| json!({}));
    let format_name = format
        .get("format_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_string();
    let format_name = inspect_container_label(input, &format_name);
    let duration = probe.duration_seconds.or_else(|| number_field(&format, "duration"));
    let bitrate = number_field(&format, "bit_rate").map(|value| value as u64);
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitle = Vec::new();
    for stream in streams {
        match stream.get("codec_type").and_then(Value::as_str).unwrap_or("") {
            "video" => video.push(normalize_video(&stream)),
            "audio" => audio.push(normalize_audio(&stream)),
            "subtitle" => subtitle.push(normalize_subtitle(&stream)),
            _ => {}
        }
    }

    Ok(json!({
        "status": "success",
        "operation": "inspect",
        "file": {
            "path": absolute_display(input),
            "size_bytes": file.len(),
            "container": format_name,
            "duration_seconds": duration,
            "bitrate": bitrate,
            "metadata": format.get("tags").cloned().unwrap_or_else(|| json!({})),
        },
        "video": video,
        "audio": audio,
        "subtitle": subtitle,
    }))
}
