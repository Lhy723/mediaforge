use super::prelude::*;
use super::{convert::*, error::*, model::*, parse::*, paths::*, state::*};

pub(crate) fn batch_command(context: &Context, args: &BatchArgs) -> Result<Value, AppError> {
    let format = args.convert.clone().ok_or_else(|| {
        AppError::new("INVALID_ARGUMENT", "Batch currently requires --convert FORMAT.")
    })?;
    let files = collect_inputs(&args.input, args.recursive)?;
    if files.is_empty() {
        return Err(AppError::new("FILE_NOT_FOUND", "No media files matched the batch input."));
    }
    let mut results = Vec::new();
    let mut success = 0usize;
    for file in files {
        let output = args
            .output_dir
            .as_ref()
            .map(|dir| dir.join(file.file_stem().unwrap_or_default()).with_extension(&format));
        let convert = ConvertArgs {
            input: file.clone(),
            to: Some(format.clone()),
            output,
            video_codec: None,
            audio_codec: None,
            hardware: None,
            quality: None,
            device: None,
        };
        match convert_command(context, &convert) { Ok(value) => { success += 1; results.push(value); }, Err(error) => results.push(json!({"status":"error","input":absolute_display(&file),"code":error.code,"message":error.message,"details":error.details})) }
    }
    let failed = results.len().saturating_sub(success);
    Ok(
        json!({"status": if failed == 0 { "success" } else { "partial_success" }, "total": results.len(), "success": success, "failed": failed, "results": results}),
    )
}
