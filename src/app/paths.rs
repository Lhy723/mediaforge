use super::prelude::*;
use super::{error::*, state::*};

pub(crate) fn resolve_output(
    context: &Context,
    input: &Path,
    output: Option<&Path>,
    extension: &str,
) -> Result<PathBuf, AppError> {
    let requested = output.map(PathBuf::from).unwrap_or_else(|| input.with_extension(extension));
    if same_path(input, &requested) {
        if output.is_some() {
            return Err(AppError::new(
                "OUTPUT_CONFLICT",
                "Input and output paths must be different.",
            )
            .with_suggestions(&[
                "Choose a different --output path.",
                "MediaForge never replaces the input in place.",
            ]));
        }
        return Ok(next_available_path(&requested));
    }
    if requested.exists() && !context.overwrite {
        return Ok(next_available_path(&requested));
    }
    if !context.dry_run {
        if let Some(parent) = requested.parent() {
            if !parent.as_os_str().is_empty() {
                if parent.exists() && !parent.is_dir() {
                    return Err(AppError::new(
                        "OUTPUT_UNWRITABLE",
                        format!("Output parent is not a directory: {}", parent.display()),
                    ));
                }
                if !parent.exists() {
                    fs::create_dir_all(parent).map_err(|error| {
                        AppError::from_io(
                            "OUTPUT_UNWRITABLE",
                            format!("Cannot create output directory {}", parent.display()),
                            error,
                        )
                    })?;
                }
            }
        }
    }
    Ok(requested)
}

pub(crate) fn next_available_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or("output");
    let ext = path.extension().and_then(OsStr::to_str);
    for index in 1..10000 {
        let candidate = match ext {
            Some(ext) => path.with_file_name(format!("{stem}_{index}.{ext}")),
            None => path.with_file_name(format!("{stem}_{index}")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!("{stem}_{}", timestamp_suffix()))
}

pub(crate) fn timestamp_suffix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a).ok() == fs::canonicalize(b).ok() || a == b
}
pub(crate) fn absolute_display(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            }
        })
        .display()
        .to_string()
}
