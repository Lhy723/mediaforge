use super::prelude::*;
use super::{error::*, hardware::*, metadata::*, model::*};

pub(crate) fn default_video_codec_for_container(container: &str) -> &'static str {
    match container {
        "webm" => "vp9",
        "ogv" => "theora",
        "flv" | "swf" => "flv1",
        "wmv" => "wmv2",
        "mpeg" | "vob" => "mpeg2video",
        _ => "h264",
    }
}

pub(crate) fn default_audio_codec_for_container(container: &str) -> &'static str {
    match container {
        "webm" | "ogv" => "opus",
        "wmv" => "wmav2",
        "flv" | "swf" => "mp3",
        "mpeg" | "vob" => "mp2",
        _ => "aac",
    }
}

pub(crate) fn validate_transcode_compatibility(
    stream: &str,
    action: &str,
    codec: &str,
    container: &str,
) -> Result<(), AppError> {
    let compatible = match stream {
        "video" => is_video_compatible(container, codec),
        "audio" => is_audio_compatible(container, codec),
        _ => false,
    };
    if action != "transcode" || compatible {
        return Ok(());
    }
    let suggestions: &[&str] = if stream == "video" {
        &[
            "Use --video-codec auto to select a compatible codec.",
            "Choose a compatible target container.",
        ]
    } else {
        &[
            "Use --audio-codec auto to select a compatible codec.",
            "Choose a compatible target container.",
        ]
    };
    Err(AppError::new(
        "UNSUPPORTED_CODEC",
        format!(
            "Cannot encode {} {stream} into {}.",
            display_codec(codec),
            container.to_uppercase()
        ),
    )
    .with_details(json!({"stream":stream,"codec":codec,"container":container}))
    .with_suggestions(suggestions))
}

pub(crate) fn preferred_codec(requested: &str, fallback: &str) -> String {
    if requested == "auto" || requested == "copy" {
        fallback.to_string()
    } else {
        requested.to_string()
    }
}

pub(crate) fn is_hardware_encoder(encoder: &str) -> bool {
    encoder.ends_with("_videotoolbox")
        || encoder.ends_with("_nvenc")
        || encoder.ends_with("_qsv")
        || encoder.ends_with("_amf")
}

pub(crate) fn remove_option(args: &mut Vec<String>, option: &str) {
    let mut index = 0;
    while index + 1 < args.len() {
        if args[index] == option {
            args.drain(index..=index + 1);
        } else {
            index += 1;
        }
    }
}

pub(crate) fn video_encode_args(
    codec: &str,
    quality: &str,
    encoder_override: Option<&str>,
) -> Result<Vec<String>, AppError> {
    if codec == "copy" {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            "Video codec `copy` cannot be used for a transcode plan.",
        ));
    }
    let codec = preferred_codec(codec, "h264");
    let default_encoder =
        software_encoder_candidates(&codec).first().copied().ok_or_else(|| {
            AppError::new("UNSUPPORTED_CODEC", format!("Unsupported video codec: {codec}"))
        })?;
    if let Some(encoder) = encoder_override.filter(|encoder| is_hardware_encoder(encoder)) {
        return Ok(vec!["-c:v".into(), encoder.to_string()]);
    }
    let encoder = encoder_override.unwrap_or(default_encoder);
    match codec.as_str() {
        "h264" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "18",
                "high" => "20",
                "balanced" => "23",
                "small" => "28",
                "tiny" => "32",
                _ => "23",
            }
            .into(),
        ]),
        "h265" | "hevc" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "20",
                "high" => "23",
                "balanced" => "26",
                "small" => "30",
                "tiny" => "34",
                _ => "26",
            }
            .into(),
        ]),
        "vp9" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-deadline".into(),
            "good".into(),
            "-cpu-used".into(),
            "2".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "18",
                "high" => "24",
                "balanced" => "30",
                "small" => "36",
                "tiny" => "42",
                _ => "30",
            }
            .into(),
            "-b:v".into(),
            "0".into(),
        ]),
        "av1" => {
            let mut args = vec![
                "-c:v".into(),
                encoder.to_string(),
                "-crf".into(),
                match quality {
                    "lossless" => "0",
                    "very-high" => "24",
                    "high" => "28",
                    "balanced" => "30",
                    "small" => "34",
                    "tiny" => "38",
                    _ => "30",
                }
                .into(),
            ];
            if encoder == "libaom-av1" {
                args.extend(["-b:v".into(), "0".into()]);
            }
            Ok(args)
        }
        "mpeg2video" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-q:v".into(),
            match quality {
                "lossless" => "1",
                "very-high" => "2",
                "high" => "3",
                "balanced" => "5",
                "small" => "7",
                "tiny" => "9",
                _ => "5",
            }
            .into(),
        ]),
        "flv1" | "wmv2" | "theora" | "mpeg4" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-q:v".into(),
            match quality {
                "lossless" => "1",
                "very-high" => "3",
                "high" => "5",
                "balanced" => "7",
                "small" => "9",
                "tiny" => "12",
                _ => "7",
            }
            .into(),
        ]),
        _ => unreachable!("software encoder candidates validate the codec"),
    }
}

pub(crate) fn quality_name(quality: Quality) -> &'static str {
    match quality {
        Quality::Lossless => "lossless",
        Quality::VeryHigh => "very-high",
        Quality::High => "high",
        Quality::Balanced => "balanced",
        Quality::Small => "small",
        Quality::Tiny => "tiny",
    }
}

pub(crate) fn hardware_quality_bitrate(quality: Quality) -> &'static str {
    match quality {
        Quality::Lossless => "12M",
        Quality::VeryHigh => "8M",
        Quality::High => "5M",
        Quality::Balanced => "3M",
        Quality::Small => "2M",
        Quality::Tiny => "1M",
    }
}
pub(crate) fn audio_encode_args(codec: &str, bitrate: &str) -> Result<Vec<String>, AppError> {
    if codec == "copy" {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            "Audio codec `copy` cannot be used for a transcode plan.",
        ));
    }
    match preferred_codec(codec, "aac").as_str() {
        "aac" => Ok(vec!["-c:a".into(), "aac".into(), "-b:a".into(), bitrate.into()]),
        "opus" => Ok(vec!["-c:a".into(), "libopus".into(), "-b:a".into(), bitrate.into()]),
        "mp3" => Ok(vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), bitrate.into()]),
        "flac" => Ok(vec!["-c:a".into(), "flac".into()]),
        "wav" => Ok(vec!["-c:a".into(), "pcm_s16le".into()]),
        "vorbis" => Ok(vec![
            "-strict".into(),
            "-2".into(),
            "-c:a".into(),
            "vorbis".into(),
            "-b:a".into(),
            bitrate.into(),
        ]),
        "wmav2" => Ok(vec!["-c:a".into(), "wmav2".into(), "-b:a".into(), bitrate.into()]),
        "aiff" => Ok(vec!["-c:a".into(), "pcm_s16be".into()]),
        "alac" => Ok(vec!["-c:a".into(), "alac".into()]),
        "amr_nb" => Ok(vec![
            "-c:a".into(),
            "libopencore_amrnb".into(),
            "-ar".into(),
            "8000".into(),
            "-ac".into(),
            "1".into(),
            "-b:a".into(),
            "12.2k".into(),
        ]),
        "ac3" => Ok(vec!["-c:a".into(), "ac3".into(), "-b:a".into(), bitrate.into()]),
        "mp2" => Ok(vec!["-c:a".into(), "mp2".into(), "-b:a".into(), bitrate.into()]),
        other => {
            Err(AppError::new("UNSUPPORTED_CODEC", format!("Unsupported audio codec: {other}")))
        }
    }
}

pub(crate) fn audio_container_args(format: &str) -> Vec<String> {
    match format {
        // ALAC is an audio codec carried by the ISO BMFF/M4A container; an
        // explicit format keeps custom `.alac` output paths deterministic.
        "alac" => vec!["-f".to_string(), "ipod".to_string()],
        _ => Vec::new(),
    }
}
pub(crate) fn subtitle_warnings(streams: &[Value], container: &str) -> Vec<String> {
    let subtitle_count = stream_count(streams, "subtitle");
    if subtitle_count == 0 {
        return Vec::new();
    }
    if matches!(container, "mp4" | "webm")
        && streams
            .iter()
            .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("subtitle"))
            .any(|stream| !subtitle_conversion_supported(container, stream))
    {
        let target = if container == "mp4" { "mov_text" } else { "WebVTT" };
        return vec![format!(
            "Some subtitle streams cannot be safely converted to {target}; review the plan before execution."
        )];
    }
    match container {
        "mp4" => {
            vec!["Subtitle streams will be converted to mov_text for MP4 compatibility.".to_string()]
        }
        "webm" => {
            vec!["Subtitle streams will be converted to WebVTT for WebM compatibility.".to_string()]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn subtitle_strategy(container: &str, streams: &[Value]) -> &'static str {
    if stream_count(streams, "subtitle") == 0 {
        "none"
    } else if matches!(container, "mp4" | "webm")
        && streams
            .iter()
            .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("subtitle"))
            .any(|stream| !subtitle_conversion_supported(container, stream))
    {
        "warning"
    } else {
        match container {
            "mp4" => "convert_to_mov_text",
            "webm" => "convert_to_webvtt",
            _ => "copy",
        }
    }
}

pub(crate) fn subtitle_conversion_supported(container: &str, stream: &Value) -> bool {
    let codec = stream.get("codec_name").and_then(Value::as_str).unwrap_or("").to_lowercase();
    match container {
        "mp4" => {
            ["subrip", "srt", "ass", "ssa", "webvtt", "mov_text", "text"].contains(&codec.as_str())
        }
        "webm" => ["subrip", "srt", "ass", "ssa", "webvtt", "text"].contains(&codec.as_str()),
        _ => true,
    }
}

pub(crate) fn subtitle_ffmpeg_args(container: &str, streams: &[Value]) -> Vec<String> {
    if stream_count(streams, "subtitle") == 0 {
        return Vec::new();
    }
    let codec = match container {
        "mp4" => "mov_text",
        "webm" => "webvtt",
        _ => "copy",
    };
    vec!["-map".to_string(), "0:s?".to_string(), "-c:s".to_string(), codec.to_string()]
}

pub(crate) fn subtitle_codec_args(container: &str, streams: &[Value]) -> Vec<String> {
    if stream_count(streams, "subtitle") == 0 {
        return Vec::new();
    }
    let codec = match container {
        "mp4" => "mov_text",
        "webm" => "webvtt",
        _ => "copy",
    };
    vec!["-c:s".to_string(), codec.to_string()]
}
