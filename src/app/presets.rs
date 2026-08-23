use super::prelude::*;
use super::{error::*, state::*};

pub(crate) fn device_presets() -> Vec<Value> {
    vec![
        json!({"id":"iphone","label":"iPhone","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1080}),
        json!({"id":"ipad","label":"iPad","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1440}),
        json!({"id":"android","label":"Android","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1080}),
        json!({"id":"psp","label":"PSP","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":480}),
        json!({"id":"car","label":"车载通用","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":720}),
    ]
}

pub(crate) fn device_profile(value: &str) -> Result<DeviceProfile, AppError> {
    match value.to_lowercase().replace('_', "-").as_str() {
        "iphone" => Ok(DeviceProfile {
            id: "iphone",
            label: "iPhone",
            container: "mp4",
            video_codec: "h264",
            audio_codec: "aac",
            max_height: 1080,
        }),
        "ipad" => Ok(DeviceProfile {
            id: "ipad",
            label: "iPad",
            container: "mp4",
            video_codec: "h264",
            audio_codec: "aac",
            max_height: 1440,
        }),
        "android" => Ok(DeviceProfile {
            id: "android",
            label: "Android",
            container: "mp4",
            video_codec: "h264",
            audio_codec: "aac",
            max_height: 1080,
        }),
        "psp" => Ok(DeviceProfile {
            id: "psp",
            label: "PSP",
            container: "mp4",
            video_codec: "h264",
            audio_codec: "aac",
            max_height: 480,
        }),
        "car" | "car-player" => Ok(DeviceProfile {
            id: "car",
            label: "车载通用",
            container: "mp4",
            video_codec: "h264",
            audio_codec: "aac",
            max_height: 720,
        }),
        other => Err(AppError::new("INVALID_ARGUMENT", format!("Unknown device preset: {other}"))
            .with_suggestions(&[
                "Use iphone, ipad, android, psp, or car.",
                "Run media presets --json to list profiles.",
            ])),
    }
}

pub(crate) fn apply_device_profile(plan: &mut OperationPlan, profile: DeviceProfile) {
    plan.args.extend(["-vf".to_string(), format!("scale=-2:{}", profile.max_height)]);
    if let Some(value) = plan.value.as_object_mut() {
        value.insert(
            "device".to_string(),
            json!({
                "id": profile.id,
                "label": profile.label,
                "container": profile.container,
                "video_codec": profile.video_codec,
                "audio_codec": profile.audio_codec,
                "max_height": profile.max_height,
            }),
        );
        value.insert("ffmpeg_args".to_string(), json!(plan.args));
        value.insert("quality_loss".to_string(), json!("video_and_audio"));
        value.insert(
            "reason".to_string(),
            json!([format!("Applied {} device profile.", profile.label)]),
        );
    }
}

pub(crate) fn presets_command(_context: &Context) -> Result<Value, AppError> {
    Ok(json!({"status":"success","operation":"presets","presets":device_presets()}))
}
