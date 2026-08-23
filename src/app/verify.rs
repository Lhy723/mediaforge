use super::prelude::*;
use super::{error::*, format::*, parse::*, paths::*, process::*, state::*};

pub(crate) fn verify_operation(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    match plan.value.get("operation").and_then(Value::as_str) {
        Some("compress") => verify_compress_output(context, input, plan),
        Some("resize") => verify_resize_output(context, input, plan),
        Some("extract_audio") => verify_audio_output(context, input, &plan.output),
        Some("audio") => verify_audio_output(context, input, &plan.output),
        Some("thumbnail") => verify_thumbnail_output(context, &plan.output),
        Some("clip") => verify_clip_output(context, input, plan),
        Some("image") | Some("gif") | Some("edit") | Some("merge") | Some("repair")
        | Some("disc") => verify_transformed_output(context, input, plan),
        _ => verify_value(context, input, &plan.output),
    }
}

pub(crate) fn verify_transformed_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let probe = probe_media(&plan.output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let operation = plan.value.get("operation").and_then(Value::as_str).unwrap_or("media");
    let video_present = first_stream(&streams, "video").is_some();
    let audio_present = first_stream(&streams, "audio").is_some();
    let required_video = match operation {
        "audio" => false,
        "disc" => plan.value.get("kind").and_then(Value::as_str) != Some("cd"),
        "merge" => plan.value.get("video_present").and_then(Value::as_bool).unwrap_or(false),
        _ => true,
    };
    let required_audio = match operation {
        "image" | "gif" => false,
        "edit" | "repair" => {
            plan.value.get("audio_present").and_then(Value::as_bool).unwrap_or(true)
        }
        "merge" => plan.value.get("audio_present").and_then(Value::as_bool).unwrap_or(false),
        "disc" => plan.value.get("kind").and_then(Value::as_str) == Some("cd"),
        _ => false,
    };
    let decode_errors = decode_check(context, &plan.output).is_err();
    let size_bytes = fs::metadata(&plan.output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    let video_match = !required_video || video_present;
    let audio_match = !required_audio || audio_present;
    Ok(json!({
        "status": "success",
        "valid": size_positive && video_match && audio_match && !decode_errors,
        "input": absolute_display(input),
        "output": absolute_display(&plan.output),
        "checks": {
            "readable": true,
            "size_bytes": size_bytes,
            "size_positive": size_positive,
            "video_present": video_present,
            "video_match": video_match,
            "audio_present": audio_present,
            "audio_match": audio_match,
            "decode_errors": decode_errors,
        },
        "warnings": []
    }))
}

pub(crate) fn verify_compress_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut verification = verify_value(context, input, &plan.output)?;
    let Some(target_size_bytes) = plan.value.get("target_size_bytes").and_then(Value::as_u64)
    else {
        return Ok(verification);
    };
    let actual_size_bytes = verification
        .get("checks")
        .and_then(|checks| checks.get("size_bytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let target_size_match = actual_size_bytes <= target_size_bytes;
    verification["checks"]["target_size_bytes"] = json!(target_size_bytes);
    verification["checks"]["target_size_match"] = json!(target_size_match);
    if !target_size_match {
        if let Some(warnings) = verification.get_mut("warnings").and_then(Value::as_array_mut) {
            warnings.push(json!(format!(
                "Output size {actual_size_bytes} exceeds target size {target_size_bytes}."
            )));
        }
    }
    let base_valid = verification.get("valid").and_then(Value::as_bool).unwrap_or(false);
    verification["valid"] = json!(base_valid && target_size_match);
    Ok(verification)
}

pub(crate) fn verify_resize_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut verification = verify_value(context, input, &plan.output)?;
    let rendered = probe_media(&plan.output, context.verbose)?;
    let streams =
        rendered.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output_video = first_stream(&streams, "video");
    let width = output_video.as_ref().and_then(|video| video.get("width")).and_then(Value::as_u64);
    let height =
        output_video.as_ref().and_then(|video| video.get("height")).and_then(Value::as_u64);
    let target = plan.value.get("target_dimension").cloned().unwrap_or(Value::Null);
    let axis = target.get("axis").and_then(Value::as_str).unwrap_or("");
    let expected = target.get("effective").and_then(Value::as_u64);
    let actual = match axis {
        "width" => width,
        "height" => height,
        _ => None,
    };
    let target_dimension_match = expected.is_some() && actual == expected;
    let even_dimensions_match = width
        .zip(height)
        .is_some_and(|(width, height)| width.is_multiple_of(2) && height.is_multiple_of(2));
    verification["checks"]["resolution_match"] = json!(target_dimension_match);
    verification["checks"]["target_dimension_match"] = json!(target_dimension_match);
    verification["checks"]["even_dimensions_match"] = json!(even_dimensions_match);
    verification["checks"]["width"] = json!(width);
    verification["checks"]["height"] = json!(height);
    if let Some(warnings) = verification.get_mut("warnings").and_then(Value::as_array_mut) {
        warnings.retain(|warning| {
            warning.as_str() != Some("Output resolution differs from the input.")
        });
        if !target_dimension_match {
            warnings.push(json!("Output resolution does not match the resize plan."));
        }
        if !even_dimensions_match {
            warnings.push(json!("Output dimensions are not both even."));
        }
    }
    let base_valid = verification.get("valid").and_then(Value::as_bool).unwrap_or(false);
    verification["valid"] = json!(base_valid && target_dimension_match && even_dimensions_match);
    Ok(verification)
}

pub(crate) fn verify_audio_output(
    context: &Context,
    input: &Path,
    output: &Path,
) -> Result<Value, AppError> {
    let probe = probe_media(output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let audio_present = first_stream(&streams, "audio").is_some();
    let decode_errors = decode_check(context, output).is_err();
    let size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    Ok(
        json!({"status":"success","valid":audio_present && !decode_errors && size_positive,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"audio_present":audio_present,"decode_errors":decode_errors},"warnings":[]}),
    )
}

pub(crate) fn verify_thumbnail_output(context: &Context, output: &Path) -> Result<Value, AppError> {
    let probe = probe_media(output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video_present = first_stream(&streams, "video").is_some();
    let decode_errors = decode_check(context, output).is_err();
    let size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    Ok(
        json!({"status":"success","valid":video_present && !decode_errors && size_positive,"output":absolute_display(output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"frame_present":video_present,"decode_errors":decode_errors},"warnings":[]}),
    )
}

pub(crate) fn verify_clip_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let probe = probe_media(&plan.output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video_present = first_stream(&streams, "video").is_some();
    let decode_errors = decode_check(context, &plan.output).is_err();
    let size_bytes = fs::metadata(&plan.output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    let expected_duration =
        if let Some(duration) = plan.value.get("duration").and_then(Value::as_str) {
            parse_time_seconds(duration).ok()
        } else if let (Some(start), Some(end)) = (
            plan.value.get("start").and_then(Value::as_str),
            plan.value.get("end").and_then(Value::as_str),
        ) {
            Some(
                (parse_time_seconds(end).unwrap_or(0.0) - parse_time_seconds(start).unwrap_or(0.0))
                    .max(0.0),
            )
        } else {
            None
        };
    let duration_match = match (probe.duration_seconds, expected_duration) {
        (Some(actual), Some(expected)) => (actual - expected).abs() <= 0.35,
        _ => true,
    };
    Ok(
        json!({"status":"success","valid":video_present && !decode_errors && duration_match && size_positive,"input":absolute_display(input),"output":absolute_display(&plan.output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"video_present":video_present,"duration_match":duration_match,"decode_errors":decode_errors},"warnings":[]}),
    )
}

pub(crate) fn verify_command(
    context: &Context,
    input: &Path,
    output: &Path,
) -> Result<Value, AppError> {
    ensure_input(input)?;
    ensure_input(output)?;
    let value = verify_value(context, input, output).map_err(|error| {
        AppError::new("VERIFY_FAILED", "Could not complete output verification.").with_details(
            json!({
                "cause": error.code,
                "message": error.message,
                "details": error.details,
            }),
        )
    })?;
    if !value.get("valid").and_then(Value::as_bool).unwrap_or(false) {
        return Err(
            AppError::new("VERIFY_FAILED", "One or more output checks failed.").with_details(value)
        );
    }
    Ok(value)
}

pub(crate) fn verification_failed(output: &Path, error: AppError) -> AppError {
    AppError::new("VERIFY_FAILED", "Could not complete output verification.").with_details(json!({
        "output": absolute_display(output),
        "cause": error.code,
        "message": error.message,
        "details": error.details,
    }))
}

pub(crate) fn verify_value(
    context: &Context,
    input: &Path,
    output: &Path,
) -> Result<Value, AppError> {
    let source = probe_media(input, context.verbose)?;
    let rendered = probe_media(output, context.verbose)?;
    let source_streams =
        source.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output_streams =
        rendered.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let duration_match = match (source.duration_seconds, rendered.duration_seconds) {
        (Some(a), Some(b)) => {
            let tolerance = (a * 0.02).max(1.5);
            (a - b).abs() <= tolerance
        }
        _ => true,
    };
    let video_present = first_stream(&output_streams, "video").is_some();
    let audio_present = first_stream(&output_streams, "audio").is_some();
    let source_video = first_stream(&source_streams, "video");
    let audio_expected = first_stream(&source_streams, "audio").is_some();
    let output_video = first_stream(&output_streams, "video");
    let source_audio = first_stream(&source_streams, "audio");
    let output_audio = first_stream(&output_streams, "audio");
    let resolution_match = match (source_video, output_video) {
        (Some(a), Some(b)) => {
            a.get("width") == b.get("width") && a.get("height") == b.get("height")
        }
        _ => true,
    };
    let video_codec_match =
        match (first_stream(&source_streams, "video"), first_stream(&output_streams, "video")) {
            (Some(a), Some(b)) => a.get("codec_name") == b.get("codec_name"),
            _ => true,
        };
    let audio_codec_match = match (source_audio, output_audio) {
        (Some(a), Some(b)) => a.get("codec_name") == b.get("codec_name"),
        _ => true,
    };
    let stream_counts_match = ["video", "audio", "subtitle"]
        .iter()
        .all(|kind| stream_count(&source_streams, kind) == stream_count(&output_streams, kind));
    let decode_errors = decode_check(context, output).is_err();
    let audio_match = !audio_expected || audio_present;
    let output_size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let output_size_positive = output_size_bytes > 0;
    let mut warnings = Vec::new();
    if !audio_match {
        warnings.push("Output is missing the input audio stream.".to_string());
    }
    if !resolution_match {
        warnings.push("Output resolution differs from the input.".to_string());
    }
    if !video_codec_match || !audio_codec_match {
        warnings.push("Output codec differs from the input.".to_string());
    }
    if !stream_counts_match {
        warnings.push("The output stream counts differ from the input.".to_string());
    }
    if !duration_match {
        warnings.push("Output duration differs materially from the input.".to_string());
    }
    let valid =
        output_size_positive && duration_match && video_present && audio_match && !decode_errors;
    Ok(
        json!({"status":"success","valid":valid,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"size_bytes":output_size_bytes,"size_positive":output_size_positive,"duration_match":duration_match,"video_present":video_present,"audio_present":audio_present,"audio_expected":audio_expected,"audio_match":audio_match,"resolution_match":resolution_match,"video_codec_match":video_codec_match,"audio_codec_match":audio_codec_match,"stream_counts_match":stream_counts_match,"decode_errors":decode_errors},"warnings":warnings}),
    )
}
