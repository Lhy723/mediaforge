use super::error::*;
use super::prelude::*;

pub(crate) fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| {
        item.as_f64().or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}
pub(crate) fn first_stream(streams: &[Value], kind: &str) -> Option<Value> {
    streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some(kind))
        .cloned()
}
pub(crate) fn stream_count(streams: &[Value], kind: &str) -> usize {
    streams
        .iter()
        .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some(kind))
        .count()
}

pub(crate) fn normalize_video(stream: &Value) -> Value {
    let codec = stream.get("codec_name").and_then(Value::as_str).unwrap_or("unknown");
    json!({"index":stream.get("index"),"codec":codec,"profile":stream.get("profile"),"width":stream.get("width"),"height":stream.get("height"),"fps":parse_ratio(stream.get("avg_frame_rate").and_then(Value::as_str).or_else(|| stream.get("r_frame_rate").and_then(Value::as_str))),"pixel_format":stream.get("pix_fmt"),"bit_depth":bit_depth(stream),"hdr":hdr_name(stream),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default": disposition_flag(stream, "default")})
}
pub(crate) fn normalize_audio(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"sample_rate":number_field(stream,"sample_rate").map(|v| v as u64),"channels":stream.get("channels"),"channel_layout":stream.get("channel_layout"),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default":disposition_flag(stream,"default")})
}
pub(crate) fn normalize_subtitle(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"language":stream.get("tags").and_then(|tags| tags.get("language")),"forced":disposition_flag(stream,"forced"),"default":disposition_flag(stream,"default")})
}
pub(crate) fn bit_depth(stream: &Value) -> Option<u8> {
    let explicit = stream
        .get("bits_per_raw_sample")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u8>().ok())
        .or_else(|| {
            stream
                .get("bits_per_raw_sample")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
        });
    if explicit.is_some() {
        return explicit;
    }
    let pixel_format = stream.get("pix_fmt").and_then(Value::as_str).unwrap_or("");
    [16, 14, 12, 10, 9, 8]
        .into_iter()
        .find(|depth| pixel_format.contains(&depth.to_string()))
        .or_else(|| (!pixel_format.is_empty()).then_some(8))
}
pub(crate) fn disposition_flag(stream: &Value, key: &str) -> bool {
    stream.get("disposition").and_then(|value| value.get(key)).and_then(Value::as_u64).unwrap_or(0)
        == 1
}
pub(crate) fn parse_ratio(value: Option<&str>) -> Option<f64> {
    let value = value?;
    let mut parts = value.split('/');
    let numerator = parts.next()?.parse::<f64>().ok()?;
    let denominator = parts.next().unwrap_or("1").parse::<f64>().ok()?;
    if denominator == 0.0 {
        None
    } else {
        Some((numerator / denominator * 1000.0).round() / 1000.0)
    }
}
pub(crate) fn hdr_name(stream: &Value) -> Value {
    let transfer = stream.get("color_transfer").and_then(Value::as_str).unwrap_or("");
    let primaries = stream.get("color_primaries").and_then(Value::as_str).unwrap_or("");
    if transfer.contains("smpte2084") {
        json!("HDR10")
    } else if transfer.contains("arib-std-b67") {
        json!("HLG")
    } else if primaries.contains("bt2020") {
        json!("HDR")
    } else {
        Value::Null
    }
}
pub(crate) fn display_codec(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" | "h265" => "HEVC".into(),
        "aac" => "AAC".into(),
        "truehd" => "TrueHD".into(),
        "opus" => "Opus".into(),
        "flac" => "FLAC".into(),
        _ => codec.to_uppercase(),
    }
}

pub(crate) fn inspect_container_label(input: &Path, format_name: &str) -> String {
    match input.extension().and_then(OsStr::to_str).map(|value| value.to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "mp4".to_string(),
        Some("mkv") => "matroska".to_string(),
        Some("mov") => "mov".to_string(),
        Some("webm") => "webm".to_string(),
        Some("avi") => "avi".to_string(),
        Some("wmv") | Some("asf") => "wmv".to_string(),
        Some("flv") => "flv".to_string(),
        Some("ogv") | Some("ogg") => "ogv".to_string(),
        Some("3gp") | Some("3g2") => "3gp".to_string(),
        Some("mpg") | Some("mpeg") => "mpeg".to_string(),
        Some("vob") => "vob".to_string(),
        Some("swf") => "swf".to_string(),
        _ => format_name.to_string(),
    }
}

pub(crate) fn internal_container(input: &Path, format_name: &str) -> String {
    match input.extension().and_then(OsStr::to_str).map(|value| value.to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "mp4".to_string(),
        Some("mkv") => "mkv".to_string(),
        Some("mov") => "mov".to_string(),
        Some("webm") => "webm".to_string(),
        Some("avi") => "avi".to_string(),
        Some("wmv") | Some("asf") => "wmv".to_string(),
        Some("flv") => "flv".to_string(),
        Some("ogv") | Some("ogg") => "ogv".to_string(),
        Some("3gp") | Some("3g2") => "3gp".to_string(),
        Some("mpg") | Some("mpeg") => "mpeg".to_string(),
        Some("vob") => "vob".to_string(),
        Some("swf") => "swf".to_string(),
        _ if format_name.contains("matroska") => "mkv".to_string(),
        _ => format_name.to_string(),
    }
}

pub(crate) fn normalize_container(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    let normalized = match value.as_str() {
        "mp4" | "m4v" => "mp4",
        "mkv" | "matroska" => "mkv",
        "mov" | "quicktime" => "mov",
        "webm" => "webm",
        "avi" => "avi",
        "wmv" | "asf" => "wmv",
        "flv" => "flv",
        "ogv" | "ogg" => "ogv",
        "3gp" | "3g2" => "3gp",
        "mpg" | "mpeg" | "mpeg1" | "mpeg2" => "mpeg",
        "vob" | "dvd" => "vob",
        "swf" => "swf",
        _ => {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                format!("Unsupported target container: {value}"),
            ))
        }
    };
    Ok(normalized.to_string())
}
pub(crate) fn normalize_audio_format(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    if [
        "mp3", "aac", "m4a", "flac", "wav", "opus", "ogg", "wma", "aiff", "aif", "alac", "amr",
        "ac3", "mp2",
    ]
    .contains(&value.as_str())
    {
        Ok(value)
    } else {
        Err(AppError::new("UNSUPPORTED_FORMAT", format!("Unsupported audio format: {value}")))
    }
}

pub(crate) fn audio_codec_for_format(format: &str) -> &'static str {
    match format {
        "m4a" | "aac" => "aac",
        "mp3" => "mp3",
        "flac" => "flac",
        "wav" => "wav",
        "opus" => "opus",
        "ogg" => "vorbis",
        "wma" => "wmav2",
        "aiff" | "aif" => "aiff",
        "alac" => "alac",
        "amr" => "amr_nb",
        "ac3" => "ac3",
        "mp2" => "mp2",
        _ => "aac",
    }
}

pub(crate) fn audio_output_extension(format: &str) -> String {
    match format {
        "aif" | "aiff" => "aiff".to_string(),
        "alac" => "m4a".to_string(),
        _ => format.to_string(),
    }
}

pub(crate) fn audio_copy_compatible(codec: &str, format: &str) -> bool {
    match format {
        "m4a" | "aac" => codec == "aac",
        "mp3" => codec == "mp3",
        "flac" => codec == "flac",
        "wav" => codec.starts_with("pcm_") || codec == "pcm_s16le",
        "opus" => codec == "opus",
        "ogg" => codec == "vorbis",
        "wma" => codec == "wmav1" || codec == "wmav2",
        "aiff" | "aif" => codec.starts_with("pcm_") || codec == "alac",
        "alac" => codec == "alac",
        "amr" => codec == "amr_nb" || codec == "amr_wb",
        "ac3" => codec == "ac3",
        "mp2" => codec == "mp2",
        _ => false,
    }
}

pub(crate) fn is_video_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["h264", "h265", "hevc", "mpeg4", "av1", "vp9"].contains(&codec),
        "webm" => ["vp8", "vp9", "av1"].contains(&codec),
        "mkv" | "avi" => true,
        "wmv" => ["wmv1", "wmv2", "msmpeg4", "msmpeg4v2", "h264"].contains(&codec),
        "flv" => ["flv1", "h263", "h264"].contains(&codec),
        "ogv" => ["theora"].contains(&codec),
        "3gp" => ["h264", "mpeg4", "h263"].contains(&codec),
        "mpeg" | "vob" => ["mpeg1video", "mpeg2video"].contains(&codec),
        "swf" => ["flv1", "h263"].contains(&codec),
        _ => false,
    }
}
pub(crate) fn is_audio_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["aac", "mp3", "ac3", "eac3"].contains(&codec),
        "webm" => ["opus", "vorbis"].contains(&codec),
        "mkv" | "avi" => true,
        "wmv" => ["wmav1", "wmav2", "wma", "aac", "mp3"].contains(&codec),
        "flv" => ["mp3", "aac"].contains(&codec),
        "ogv" => ["vorbis", "opus"].contains(&codec),
        "3gp" => ["aac", "amr_nb", "amr_wb", "mp3"].contains(&codec),
        "mpeg" | "vob" => ["mp1", "mp2", "mp3", "ac3", "dts", "pcm_s16be"].contains(&codec),
        "swf" => ["mp3"].contains(&codec),
        _ => false,
    }
}
