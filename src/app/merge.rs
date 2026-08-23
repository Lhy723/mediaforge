use super::prelude::*;
use super::{error::*, execution::*, format::*, model::*, paths::*, process::*, state::*};

pub(crate) fn merge_command(context: &Context, args: &MergeArgs) -> Result<Value, AppError> {
    if args.inputs.len() < 2 {
        return Err(AppError::new("INVALID_ARGUMENT", "Merge requires at least two input files."));
    }
    let mode = args.mode.to_lowercase();
    if !["concat", "mux", "mix"].contains(&mode.as_str()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Merge mode must be concat, mux, or mix."));
    }
    for input in &args.inputs {
        ensure_input(input)?;
    }
    if mode != "concat" && args.inputs.len() != 2 {
        return Err(AppError::new("INVALID_ARGUMENT", "Mux and mix require exactly two inputs."));
    }
    let probes = args
        .inputs
        .iter()
        .map(|input| probe_media(input, context.verbose))
        .collect::<Result<Vec<_>, _>>()?;
    let streams = probes
        .iter()
        .map(|probe| {
            probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let has_video = streams.iter().any(|value| first_stream(value, "video").is_some());
    let has_audio = streams.iter().any(|value| first_stream(value, "audio").is_some());
    if mode == "mux"
        && (first_stream(&streams[0], "video").is_none()
            || first_stream(&streams[1], "audio").is_none())
    {
        return Err(AppError::new(
            "INVALID_MEDIA",
            "Mux expects a video input followed by an audio input.",
        ));
    }
    if mode == "mix" && streams.iter().any(|value| first_stream(value, "audio").is_none()) {
        return Err(AppError::new("INVALID_MEDIA", "Mix expects audio streams in both inputs."));
    }
    let output_extension = if args.output.is_some() {
        args.output
            .as_deref()
            .and_then(|path| path.extension().and_then(OsStr::to_str))
            .unwrap_or("mp4")
            .to_string()
    } else if mode == "mix" && !has_video {
        "m4a".to_string()
    } else {
        "mp4".to_string()
    };
    let output =
        resolve_output(context, &args.inputs[0], args.output.as_deref(), &output_extension)?;
    let mut ffmpeg_args = Vec::new();
    for input in &args.inputs {
        ffmpeg_args.extend(["-i".to_string(), input.to_string_lossy().to_string()]);
    }
    match mode.as_str() {
        "concat" => {
            let all_video = streams.iter().all(|value| first_stream(value, "video").is_some());
            let all_audio = streams.iter().all(|value| first_stream(value, "audio").is_some());
            let mut filter = String::new();
            for index in 0..args.inputs.len() {
                if all_video {
                    filter.push_str(&format!("[{index}:v:0]"));
                }
                if all_audio {
                    filter.push_str(&format!("[{index}:a:0]"));
                }
            }
            filter.push_str(&format!(
                "concat=n={}:v={}:a={}",
                args.inputs.len(),
                all_video as u8,
                all_audio as u8
            ));
            if all_video {
                filter.push_str("[v]");
            }
            if all_audio {
                filter.push_str("[a]");
            }
            ffmpeg_args.extend(["-filter_complex".to_string(), filter]);
            if all_video {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "[v]".to_string(),
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                    "-crf".to_string(),
                    "23".to_string(),
                ]);
            }
            if all_audio {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "[a]".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    DEFAULT_AUDIO_BITRATE.to_string(),
                ]);
            }
        }
        "mux" => ffmpeg_args.extend([
            "-map".to_string(),
            "0:v:0".to_string(),
            "-map".to_string(),
            "1:a:0".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
        ]),
        "mix" => {
            ffmpeg_args.extend([
                "-filter_complex".to_string(),
                "[0:a:0][1:a:0]amix=inputs=2:duration=longest[a]".to_string(),
            ]);
            if has_video {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "0:v:0".to_string(),
                    "-c:v".to_string(),
                    "copy".to_string(),
                ]);
            }
            ffmpeg_args.extend([
                "-map".to_string(),
                "[a]".to_string(),
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                DEFAULT_AUDIO_BITRATE.to_string(),
            ]);
        }
        _ => unreachable!(),
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "merge",
            "mode": mode,
            "inputs": args.inputs.iter().map(|path| absolute_display(path)).collect::<Vec<_>>(),
            "input_count": args.inputs.len(),
            "output": absolute_display(&output),
            "strategy": mode,
            "quality_loss": if args.mode.eq_ignore_ascii_case("mux") { "none" } else { "possible" },
            "video_present": has_video,
            "audio_present": if mode == "concat" {
                streams.iter().all(|value| first_stream(value, "audio").is_some())
            } else {
                has_audio
            },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: mode.to_string(),
    };
    finish_custom_plan(context, &args.inputs[0], plan)
}
