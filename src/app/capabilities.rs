use super::prelude::*;
use super::{disc::*, edit::*, error::*, presets::*, process::*, state::*};

pub(crate) fn capabilities_command(context: &Context) -> Result<Value, AppError> {
    let version = run_program("ffmpeg", &["-version"], context.verbose)
        .map(|result| result.stdout.lines().next().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not installed".to_string());
    let hwaccels = run_program("ffmpeg", &["-hide_banner", "-hwaccels"], context.verbose)
        .map(|result| {
            result
                .stdout
                .lines()
                .skip(1)
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let encoder_text = run_program("ffmpeg", &["-hide_banner", "-encoders"], context.verbose)
        .map(|result| result.stdout)
        .unwrap_or_default();
    let mut encoders: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (codec, needles) in [
        ("h264", vec!["libx264", "h264_videotoolbox", "h264_nvenc", "h264_qsv", "h264_amf"]),
        ("hevc", vec!["libx265", "hevc_videotoolbox", "hevc_nvenc", "hevc_qsv", "hevc_amf"]),
        ("vp9", vec!["libvpx-vp9"]),
        ("av1", vec!["libaom-av1", "libsvtav1", "av1_nvenc", "av1_qsv", "av1_amf"]),
        ("mpeg4", vec!["mpeg4", "libxvid"]),
        ("mpeg2video", vec!["mpeg2video"]),
        ("flv1", vec!["flv1"]),
        ("wmv2", vec!["wmv2"]),
        ("theora", vec!["libtheora", "theora"]),
        ("mjpeg", vec!["mjpeg"]),
        ("png", vec!["png"]),
        ("webp", vec!["libwebp"]),
        ("gif", vec!["gif"]),
        ("bmp", vec!["bmp"]),
        ("tiff", vec!["tiff"]),
        ("targa", vec!["targa"]),
        ("libaom-av1-image", vec!["libaom-av1"]),
        ("aac", vec!["aac", "libfdk_aac"]),
        ("mp3", vec!["libmp3lame", "mp3"]),
        ("opus", vec!["libopus", "opus"]),
        ("vorbis", vec!["libvorbis", "vorbis"]),
        ("flac", vec!["flac"]),
        ("pcm_s16le", vec!["pcm_s16le"]),
        ("wmav2", vec!["wmav2"]),
        ("alac", vec!["alac"]),
        ("amr_nb", vec!["libopencore_amrnb", "amr_nb"]),
        ("ac3", vec!["ac3"]),
        ("mp2", vec!["mp2"]),
    ] {
        let found = needles
            .into_iter()
            .filter(|needle| {
                encoder_text
                    .lines()
                    .any(|line| line.split_whitespace().any(|token| token == *needle))
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        encoders.insert(codec, found);
    }
    let has_accel = |name: &str| hwaccels.iter().any(|value| value == name);
    let has_encoder =
        |needle: &str| encoders.values().any(|values| values.iter().any(|value| value == needle));
    let hardware_acceleration = json!({
        "videotoolbox": has_accel("videotoolbox") || has_encoder("h264_videotoolbox"),
        "nvenc": has_accel("cuda") || has_encoder("h264_nvenc"),
        "qsv": has_accel("qsv") || has_encoder("h264_qsv"),
        "vaapi": has_accel("vaapi"),
        "amf": has_encoder("h264_amf"),
    });
    let external_tools = [
        "ffmpeg",
        "ffprobe",
        "drutil",
        "diskutil",
        "mount",
        "dvdbackup",
        "abcde",
        "xorriso",
        "genisoimage",
        "mkisofs",
        "hdiutil",
    ]
    .into_iter()
    .map(|tool| (tool, program_available(tool)))
    .collect::<BTreeMap<_, _>>();
    Ok(json!({
        "status":"success",
        "ffmpeg":{"installed":version != "not installed","version":version},
        "platform":std::env::consts::OS,
        "architecture":std::env::consts::ARCH,
        "hardware_acceleration":hardware_acceleration,
        "hardware_acceleration_list":hwaccels,
        "encoders":encoders,
            "supported_containers":["mp4","mkv","mov","webm","avi","wmv","asf","flv","ogv","3gp","mpg","mpeg","vob","swf"],
            "supported_image_formats":["png","jpg","webp","gif","bmp","tiff","ico","tga","avif"],
            "supported_audio_formats":["mp3","aac","m4a","flac","wav","opus","ogg","wma","aiff","alac","amr","ac3","mp2"],
            "formats":{
                "containers":["mp4","mkv","mov","webm","avi","wmv","asf","flv","ogv","3gp","mpg","mpeg","vob","swf"],
                "image":["png","jpg","webp","gif","bmp","tiff","ico","tga","avif"],
                "audio":["mp3","aac","m4a","flac","wav","opus","ogg","wma","aiff","alac","amr","ac3","mp2"]
            },
            "device_presets":device_presets(),
            "external_tools":external_tools,
            "disc":{
                "iso_authoring_tools":["xorriso","genisoimage","mkisofs","hdiutil"],
                "iso_authoring_available":disc_authoring_tool().is_some(),
                "note":"DVD/CD extraction and ISO authoring depend on OS permissions and optional utilities."
            },
            "filters":{
                "subtitles":ffmpeg_filter_available(context, "subtitles"),
                "named_video_filters":["grayscale","blur","sharpen","vintage"],
                "note":"Subtitle burn-in requires the FFmpeg subtitles/libass filter."
            },
            "operations":["inspect","plan","convert","compress","resize","clip","extract_audio","thumbnail","image","gif","edit","merge","audio","repair","disc","batch","verify","capabilities","presets"],
            "notes":[
            "Encoder availability is build-specific; an advertised format can still return ENCODER_UNAVAILABLE.",
            "DVD/CD device access and protected-media support depend on OS permissions and optional tools."
        ]
    }))
}
