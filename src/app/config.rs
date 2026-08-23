use super::prelude::*;
use super::{error::*, model::*};

pub(crate) fn parse_quality_name(value: String) -> Result<Quality, AppError> {
    match value.to_lowercase().replace('_', "-").as_str() {
        "lossless" => Ok(Quality::Lossless),
        "very-high" => Ok(Quality::VeryHigh),
        "high" => Ok(Quality::High),
        "balanced" => Ok(Quality::Balanced),
        "small" => Ok(Quality::Small),
        "tiny" => Ok(Quality::Tiny),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported quality preset: {other}")))
        }
    }
}

pub(crate) fn parse_hardware_name(value: String) -> Result<HardwareMode, AppError> {
    match value.to_lowercase().as_str() {
        "auto" => Ok(HardwareMode::Auto),
        "cpu" => Ok(HardwareMode::Cpu),
        "gpu" => Ok(HardwareMode::Gpu),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported hardware mode: {other}")))
        }
    }
}

pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MEDIAFORGE_CONFIG") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path).join("mediaforge/config.toml"));
    }
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(path).join("mediaforge/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/mediaforge/config.toml"))
}

pub(crate) fn load_config() -> Result<ConfigFile, AppError> {
    let Some(path) = config_path() else {
        return Ok(ConfigFile::default());
    };
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let contents = fs::read_to_string(&path).map_err(|error| {
        AppError::from_io(
            "INVALID_ARGUMENT",
            format!("Cannot read config {}", path.display()),
            error,
        )
    })?;
    toml::from_str(&contents).map_err(|error| {
        AppError::new("INVALID_ARGUMENT", format!("Invalid MediaForge config: {error}"))
            .with_details(json!({"path": path.display().to_string()}))
    })
}
