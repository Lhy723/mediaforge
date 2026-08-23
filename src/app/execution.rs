use super::prelude::*;
use super::{error::*, paths::*, process::*, state::*, verify::*};

pub(crate) fn execute_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    if plan.output == input {
        return Err(AppError::new("OUTPUT_CONFLICT", "Input and output paths must be different.")
            .with_details(
                json!({"input": absolute_display(input), "output": absolute_display(&plan.output)}),
            ));
    }
    let args = build_ffmpeg_args(input, &plan.output, &plan.args, context.overwrite);
    run_ffmpeg(context, &args)?;
    finish_plan_execution(context, input, plan)
}

pub(crate) fn execute_two_pass_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    if plan.output == input {
        return Err(AppError::new("OUTPUT_CONFLICT", "Input and output paths must be different.")
            .with_details(
                json!({"input": absolute_display(input), "output": absolute_display(&plan.output)}),
            ));
    }
    let passlog = temporary_passlog_path();
    let mut first_pass = plan.args.clone();
    first_pass.extend([
        "-pass".to_string(),
        "1".to_string(),
        "-passlogfile".to_string(),
        passlog.to_string_lossy().to_string(),
        "-an".to_string(),
        "-sn".to_string(),
        "-f".to_string(),
        "null".to_string(),
    ]);
    let first_args =
        build_ffmpeg_args(input, Path::new(null_device()), &first_pass, context.overwrite);
    let first_result = run_ffmpeg(context, &first_args);
    if let Err(error) = first_result {
        cleanup_passlog(&passlog);
        return Err(error);
    }

    let mut second_pass = plan.args.clone();
    second_pass.extend([
        "-pass".to_string(),
        "2".to_string(),
        "-passlogfile".to_string(),
        passlog.to_string_lossy().to_string(),
    ]);
    let second_args = build_ffmpeg_args(input, &plan.output, &second_pass, context.overwrite);
    let second_result = run_ffmpeg(context, &second_args);
    cleanup_passlog(&passlog);
    second_result?;
    finish_plan_execution(context, input, plan)
}

pub(crate) fn build_ffmpeg_args(
    input: &Path,
    output: &Path,
    operation_args: &[String],
    overwrite: bool,
) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        if overwrite { "-y" } else { "-n" }.to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
    ];
    args.extend(operation_args.iter().cloned());
    args.push(output.to_string_lossy().to_string());
    args
}

pub(crate) fn finish_plan_execution(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let verification = if context.verify_after_execute {
        verify_operation(context, input, plan)
            .map_err(|error| verification_failed(&plan.output, error))?
    } else {
        json!({
            "status": "skipped",
            "valid": Value::Null,
            "reason": "disabled_by_configuration",
            "input": absolute_display(input),
            "output": absolute_display(&plan.output),
        })
    };
    if verification.get("valid").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "VERIFY_FAILED",
            "FFmpeg completed but the output did not pass verification.",
        )
        .with_details(
            json!({"output": absolute_display(&plan.output), "verification": verification}),
        ));
    }
    let operation = plan.value.get("operation").cloned().unwrap_or_else(|| json!("convert"));
    Ok(json!({
        "status": "success",
        "operation": operation,
        "input": absolute_display(input),
        "output": absolute_display(&plan.output),
        "strategy": plan.strategy,
        "video": plan.value.get("video").cloned().unwrap_or(Value::Null),
        "audio": plan.value.get("audio").cloned().unwrap_or(Value::Null),
        "quality": plan.value.get("quality").cloned().unwrap_or(Value::Null),
        "quality_loss": plan.value.get("quality_loss").cloned().unwrap_or(Value::Null),
        "reason": plan.value.get("reason").cloned().unwrap_or_else(|| json!([])),
        "warnings": plan.value.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "hardware": plan.value.get("hardware").cloned().unwrap_or(Value::Null),
        "subtitle": plan.value.get("subtitle").cloned().unwrap_or(Value::Null),
        "metadata": plan.value.get("metadata").cloned().unwrap_or(Value::Null),
        "target_size_bytes": plan.value.get("target_size_bytes").cloned().unwrap_or(Value::Null),
        "target_dimension": plan.value.get("target_dimension").cloned().unwrap_or(Value::Null),
        "passes": plan.value.get("passes").cloned().unwrap_or_else(|| json!(1)),
        "pass_strategy": plan
            .value
            .get("pass_strategy")
            .cloned()
            .unwrap_or_else(|| json!("single_pass")),
        "verification": verification,
    }))
}

pub(crate) fn temporary_passlog_path() -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    std::env::temp_dir().join(format!("mediaforge-pass-{}-{nanos}", std::process::id()))
}

pub(crate) fn cleanup_passlog(passlog: &Path) {
    let Some(name) = passlog.file_name().and_then(OsStr::to_str) else {
        return;
    };
    let Some(parent) = passlog.parent() else {
        return;
    };
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let matches = path
                .file_name()
                .and_then(OsStr::to_str)
                .map(|value| value.starts_with(name))
                .unwrap_or(false);
            if matches {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub(crate) fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

pub(crate) fn finish_custom_plan(
    context: &Context,
    input: &Path,
    plan: OperationPlan,
) -> Result<Value, AppError> {
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, input, &plan)
}

pub(crate) fn execute_simple_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        if context.overwrite { "-y" } else { "-n" }.to_string(),
    ];
    if !plan.args.iter().any(|argument| argument == "-i") {
        args.extend(["-i".to_string(), input.to_string_lossy().to_string()]);
    }
    args.extend(plan.args.clone());
    args.push(plan.output.to_string_lossy().to_string());
    run_ffmpeg(context, &args)?;
    let verification = if context.verify_after_execute {
        verify_operation(context, input, plan)
            .map_err(|error| verification_failed(&plan.output, error))?
    } else {
        json!({
            "status": "skipped",
            "valid": Value::Null,
            "reason": "disabled_by_configuration",
            "input": absolute_display(input),
            "output": absolute_display(&plan.output),
        })
    };
    if verification.get("valid").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "VERIFY_FAILED",
            "Operation completed but the output did not pass verification.",
        )
        .with_details(verification));
    }
    Ok(json!({
        "status":"success",
        "operation":plan.value.get("operation").cloned().unwrap_or_else(|| json!("media")),
        "input":absolute_display(input),
        "output":absolute_display(&plan.output),
        "strategy":plan.strategy,
        "video":plan.value.get("video").cloned().unwrap_or(Value::Null),
        "audio":plan.value.get("audio").cloned().unwrap_or(Value::Null),
        "quality":plan.value.get("quality").cloned().unwrap_or(Value::Null),
        "quality_loss":plan.value.get("quality_loss").cloned().unwrap_or(Value::Null),
        "reason":plan.value.get("reason").cloned().unwrap_or_else(|| json!([])),
        "warnings":plan.value.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "hardware":plan.value.get("hardware").cloned().unwrap_or(Value::Null),
        "subtitle":plan.value.get("subtitle").cloned().unwrap_or(Value::Null),
        "metadata":plan.value.get("metadata").cloned().unwrap_or(Value::Null),
        "verification":verification
    }))
}
