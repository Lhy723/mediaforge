use super::error::*;
use super::prelude::*;

pub(crate) fn parse_resolution(value: &str) -> Result<u32, AppError> {
    let value = value.to_lowercase();
    let value = value.strip_suffix('p').unwrap_or(&value);
    let resolution = value
        .parse::<u32>()
        .map_err(|_| AppError::new("INVALID_ARGUMENT", format!("Invalid resolution: {value}")))?;
    if resolution == 0 {
        return Err(AppError::new("INVALID_ARGUMENT", "Resolution must be greater than zero."));
    }
    Ok(resolution)
}

pub(crate) fn even_dimension(value: u32) -> Result<u32, AppError> {
    if value.is_multiple_of(2) {
        return Ok(value);
    }
    value.checked_add(1).ok_or_else(|| {
        AppError::new(
            "INVALID_ARGUMENT",
            "Resize dimension is too large to round to an even value.",
        )
    })
}
pub(crate) fn parse_thumbnail_time(value: &str, duration: Option<f64>) -> Result<String, AppError> {
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent
            .parse::<f64>()
            .map_err(|_| AppError::new("INVALID_ARGUMENT", "Invalid percentage for --at."))?;
        if !(0.0..=100.0).contains(&percent) {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Thumbnail percentage must be between 0% and 100%.",
            ));
        }
        let duration = duration.ok_or_else(|| {
            AppError::new(
                "INVALID_MEDIA",
                "Percentage thumbnail position requires a known duration.",
            )
        })?;
        // A timestamp exactly at the container duration is often past the last
        // decoded frame. Keep the final percentage inside a conservative half
        // second guard band so short files still yield a thumbnail.
        let last_decodable = (duration - 0.5).max(0.0);
        let position = (duration * percent / 100.0).min(last_decodable);
        return Ok(format!("{position:.3}"));
    }
    let seconds = parse_time_seconds(value)?;
    if seconds < 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "Thumbnail position must not be negative."));
    }
    Ok(value.to_string())
}
pub(crate) fn parse_time_seconds(value: &str) -> Result<f64, AppError> {
    if let Ok(seconds) = value.parse::<f64>() {
        return Ok(seconds);
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 && parts.len() != 3 {
        return Err(AppError::new("INVALID_ARGUMENT", format!("Invalid time value: {value}")));
    }
    let numbers =
        parts.iter().map(|part| part.parse::<f64>()).collect::<Result<Vec<_>, _>>().map_err(
            |_| AppError::new("INVALID_ARGUMENT", format!("Invalid time value: {value}")),
        )?;
    Ok(if numbers.len() == 2 {
        numbers[0] * 60.0 + numbers[1]
    } else {
        numbers[0] * 3600.0 + numbers[1] * 60.0 + numbers[2]
    })
}
pub(crate) fn parse_size(value: &str) -> Result<u64, AppError> {
    let value = value.trim().to_uppercase();
    let (number, multiplier) = if let Some(value) = value.strip_suffix("GB") {
        (value, 1024_f64.powi(3))
    } else if let Some(value) = value.strip_suffix("MB") {
        (value, 1024_f64.powi(2))
    } else if let Some(value) = value.strip_suffix("KB") {
        (value, 1024_f64)
    } else if let Some(value) = value.strip_suffix('B') {
        (value, 1.0)
    } else {
        return Err(AppError::new("INVALID_ARGUMENT", format!("Invalid size: {value}")));
    };
    let number = number
        .trim()
        .parse::<f64>()
        .map_err(|_| AppError::new("INVALID_ARGUMENT", format!("Invalid size: {value}")))?;
    if !number.is_finite() || number <= 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "Size must be greater than zero."));
    }
    Ok((number * multiplier) as u64)
}

pub(crate) fn collect_inputs(input: &str, recursive: bool) -> Result<Vec<PathBuf>, AppError> {
    let path = Path::new(input);
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if path.is_dir() {
        return Ok(walk_files(path, recursive));
    }
    if input.contains('*') || input.contains('?') {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let pattern = path.file_name().and_then(OsStr::to_str).unwrap_or("");
        let mut files = fs::read_dir(parent)
            .map_err(|error| {
                AppError::from_io(
                    "FILE_NOT_FOUND",
                    format!("Cannot scan {}", parent.display()),
                    error,
                )
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate.is_file()
                    && wildcard_match(
                        candidate.file_name().and_then(OsStr::to_str).unwrap_or(""),
                        pattern,
                    )
            })
            .collect::<Vec<_>>();
        files.sort();
        return Ok(files);
    }
    Err(AppError::new("FILE_NOT_FOUND", format!("No files matched: {input}")))
}
pub(crate) fn walk_files(root: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_media_extension(&path) {
                files.push(path);
            } else if recursive && path.is_dir() {
                files.extend(walk_files(&path, true));
            }
        }
    }
    files.sort();
    files
}
pub(crate) fn is_media_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            [
                "mp4", "mkv", "mov", "webm", "avi", "wmv", "asf", "flv", "ogv", "3gp", "mpg",
                "mpeg", "vob", "swf", "m4v", "mts", "m2ts", "mp3", "wav", "flac", "m4a", "aac",
                "opus", "ogg", "wma", "aiff", "aif", "alac", "amr", "ac3", "mp2", "png", "jpg",
                "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "tga", "avif",
            ]
            .contains(&ext.to_lowercase().as_str())
        })
        .unwrap_or(false)
}
pub(crate) fn wildcard_match(value: &str, pattern: &str) -> bool {
    wildcard_match_bytes(value.as_bytes(), pattern.as_bytes())
}
pub(crate) fn wildcard_match_bytes(value: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == b'*' {
        wildcard_match_bytes(value, &pattern[1..])
            || (!value.is_empty() && wildcard_match_bytes(&value[1..], pattern))
    } else if pattern[0] == b'?' {
        !value.is_empty() && wildcard_match_bytes(&value[1..], &pattern[1..])
    } else {
        !value.is_empty()
            && value[0].eq_ignore_ascii_case(&pattern[0])
            && wildcard_match_bytes(&value[1..], &pattern[1..])
    }
}
