use super::model::*;
use super::prelude::*;

#[derive(Debug, Clone)]
pub(crate) struct Context {
    pub(crate) json: bool,
    pub(crate) dry_run: bool,
    pub(crate) overwrite: bool,
    pub(crate) verbose: bool,
    pub(crate) verify_after_execute: bool,
    pub(crate) progress: bool,
    pub(crate) default_quality: Quality,
    pub(crate) default_hardware: HardwareMode,
    pub(crate) default_video_codec: String,
    pub(crate) default_audio_codec: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Probe {
    pub(crate) raw: Value,
    pub(crate) duration_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct OperationPlan {
    pub(crate) value: Value,
    pub(crate) output: PathBuf,
    pub(crate) args: Vec<String>,
    pub(crate) strategy: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HardwareSelection {
    pub(crate) requested: String,
    pub(crate) selected: String,
    pub(crate) encoder: Option<String>,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeviceProfile {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) container: &'static str,
    pub(crate) video_codec: &'static str,
    pub(crate) audio_codec: &'static str,
    pub(crate) max_height: u32,
}
