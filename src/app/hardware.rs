use super::prelude::*;
use super::{error::*, format::*, model::*, output::*, process::*, state::*};

pub(crate) fn select_video_hardware(
    context: &Context,
    requested: HardwareMode,
    codec: &str,
    needs_video_encode: bool,
) -> Result<HardwareSelection, AppError> {
    let requested_name = format_hardware(requested).to_string();
    if !needs_video_encode {
        return Ok(HardwareSelection {
            requested: requested_name,
            selected: "not_applicable".to_string(),
            encoder: None,
            reason: "Video is copied, so no encoder hardware is required.".to_string(),
        });
    }

    match requested {
        HardwareMode::Cpu => Ok(HardwareSelection {
            requested: requested_name,
            selected: "cpu".to_string(),
            encoder: None,
            reason: "Software encoding was explicitly requested.".to_string(),
        }),
        HardwareMode::Auto => Ok(HardwareSelection {
            requested: requested_name,
            selected: "cpu".to_string(),
            encoder: None,
            reason: "Auto uses deterministic software encoding; request gpu to opt in to a hardware encoder.".to_string(),
        }),
        HardwareMode::Gpu => {
            let normalized_codec = preferred_codec(codec, "h264");
            let candidates = hardware_encoder_candidates(&normalized_codec);
            if candidates.is_empty() {
                return Err(AppError::new(
                    "UNSUPPORTED_HARDWARE",
                    format!("No hardware encoder mapping exists for video codec {normalized_codec}."),
                )
                .with_details(json!({
                    "requested_hardware": "gpu",
                    "requested_codec": normalized_codec,
                }))
                .with_suggestions(&[
                    "Use --hardware cpu or choose h264, h265, or av1.",
                    "Run media capabilities to inspect available encoders.",
                ]));
            }
            let encoder_text = run_program("ffmpeg", &["-hide_banner", "-encoders"], context.verbose)?
                .stdout;
            let selected_encoder = candidates.iter().find(|candidate| {
                encoder_text
                    .lines()
                    .any(|line| line.split_whitespace().any(|token| token == **candidate))
            });
            let Some(selected_encoder) = selected_encoder else {
                return Err(AppError::new(
                    "ENCODER_UNAVAILABLE",
                    format!("No available GPU encoder was found for {normalized_codec}."),
                )
                .with_details(json!({
                    "requested_hardware": "gpu",
                    "requested_codec": normalized_codec,
                    "candidates": candidates,
                }))
                .with_suggestions(&[
                    "Use --hardware cpu for a software encode.",
                    "Run media capabilities to inspect available encoders.",
                ]));
            };
            Ok(HardwareSelection {
                requested: requested_name,
                selected: "gpu".to_string(),
                encoder: Some((*selected_encoder).to_string()),
                reason: format!("Using the available {} hardware encoder.", selected_encoder),
            })
        }
    }
}

pub(crate) fn hardware_encoder_candidates(codec: &str) -> &'static [&'static str] {
    match codec {
        "h264" => H264_HARDWARE_ENCODERS,
        "h265" | "hevc" => HEVC_HARDWARE_ENCODERS,
        "av1" => AV1_HARDWARE_ENCODERS,
        _ => &[],
    }
}

pub(crate) fn software_encoder_candidates(codec: &str) -> &'static [&'static str] {
    match codec {
        "h264" => &["libx264"],
        "h265" | "hevc" => &["libx265"],
        "vp9" => &["libvpx-vp9"],
        "av1" => &["libsvtav1", "libaom-av1"],
        "mpeg2video" => &["mpeg2video"],
        "flv1" => &["flv"],
        "wmv2" => &["wmv2"],
        "theora" => &["libtheora", "theora"],
        "mpeg4" => &["mpeg4", "libxvid"],
        _ => &[],
    }
}

pub(crate) fn select_software_video_encoder(
    context: &Context,
    codec: &str,
) -> Result<String, AppError> {
    let candidates = software_encoder_candidates(codec);
    if candidates.is_empty() {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            format!("Unsupported video codec: {codec}"),
        ));
    }
    let encoder_text =
        run_program("ffmpeg", &["-hide_banner", "-encoders"], context.verbose)?.stdout;
    candidates
        .iter()
        .find(|candidate| {
            encoder_text
                .lines()
                .any(|line| line.split_whitespace().any(|token| token == **candidate))
        })
        .map(|encoder| (*encoder).to_string())
        .ok_or_else(|| {
            AppError::new(
                "ENCODER_UNAVAILABLE",
                format!("No software encoder is available for video codec {codec}."),
            )
            .with_details(json!({"requested_codec":codec,"candidates":candidates}))
            .with_suggestions(&[
                "Run media capabilities to inspect available encoders.",
                "Install an FFmpeg build with the requested software encoder.",
            ])
        })
}
