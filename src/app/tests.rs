use super::*;

#[test]
fn parses_ratios_and_sizes() {
    assert_eq!(parse_ratio(Some("24000/1001")), Some(23.976));
    assert_eq!(parse_size("500MB").unwrap(), 524_288_000);
}

#[test]
fn safe_path_increments_without_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("video.mp4");
    fs::write(&path, b"existing").unwrap();
    assert_eq!(next_available_path(&path), temp.path().join("video_1.mp4"));
}

#[test]
fn ffmpeg_args_require_explicit_overwrite() {
    let input = Path::new("input.mp4");
    let output = Path::new("output.mp4");
    let operation_args = vec!["-c".to_string(), "copy".to_string()];

    let safe_args = build_ffmpeg_args(input, output, &operation_args, false);
    assert_eq!(safe_args.get(2).map(String::as_str), Some("-n"));

    let overwrite_args = build_ffmpeg_args(input, output, &operation_args, true);
    assert_eq!(overwrite_args.get(2).map(String::as_str), Some("-y"));
}

#[test]
fn wildcard_match_supports_globs() {
    assert!(wildcard_match("clip-01.mov", "*.mov"));
    assert!(wildcard_match("a1.mp4", "a?.mp4"));
    assert!(!wildcard_match("clip.wav", "*.mov"));
}

#[test]
fn parses_agent_tool_request_and_aliases() {
    let request: ToolRequest = serde_json::from_str(
            r#"{"operation":"extract-audio","input":"in.mp4","format":"flac","quality":"tiny","dry_run":true,"verify_after_execute":false}"#,
        )
        .unwrap();
    assert_eq!(request.operation, "extract-audio");
    assert_eq!(normalize_operation("convert_media"), "convert");
    assert_eq!(normalize_operation("plan-media-operation"), "plan");
    assert_eq!(normalize_operation("create_thumbnail"), "thumbnail");
    assert_eq!(request.verify_after_execute, Some(false));
    assert_eq!(request.quality.as_deref(), Some("tiny"));
    assert!(hardware_encoder_candidates("h264").contains(&"h264_videotoolbox"));
    assert_eq!(hardware_quality_bitrate(Quality::Balanced), "3M");
    assert_eq!(parse_quality_name("very_high".to_string()).unwrap() as u8, Quality::VeryHigh as u8);
    assert!(matches!(parse_hardware_name("gpu".to_string()), Ok(HardwareMode::Gpu)));
}

#[test]
fn audio_extraction_prefers_copy_for_compatible_codecs() {
    assert_eq!(audio_codec_for_format("m4a"), "aac");
    assert!(audio_copy_compatible("aac", "m4a"));
    assert!(audio_copy_compatible("pcm_s16le", "wav"));
    assert!(!audio_copy_compatible("aac", "flac"));
}

#[test]
fn progress_duration_and_stream_counts_are_deterministic() {
    let args = vec!["-t".to_string(), "00:01:30".to_string()];
    assert_eq!(progress_duration_seconds(&args), Some(90.0));
    assert_eq!(estimated_remaining_seconds(Some(0.25), 10.0), Some(30.0));
    assert_eq!(format_progress_time(65.0), "01:05");
    assert_eq!(format_progress_time(3661.0), "01:01:01");
    let streams = vec![
        json!({"codec_type":"video"}),
        json!({"codec_type":"audio"}),
        json!({"codec_type":"subtitle","codec_name":"subrip"}),
        json!({"codec_type":"subtitle","codec_name":"subrip"}),
    ];
    assert_eq!(stream_count(&streams, "video"), 1);
    assert_eq!(stream_count(&streams, "subtitle"), 2);
    assert_eq!(subtitle_strategy("mp4", &streams), "convert_to_mov_text");
    let mut stderr = String::new();
    append_stderr_tail(&mut stderr, &"x".repeat(MAX_CAPTURED_STDERR_BYTES + 10));
    assert_eq!(stderr.len(), MAX_CAPTURED_STDERR_BYTES);
}

#[test]
fn validates_numeric_ranges_and_clamps_thumbnail_end() {
    assert_eq!(parse_thumbnail_time("100%", Some(3.0)).unwrap(), "2.500");
    assert!(parse_thumbnail_time("101%", Some(3.0)).is_err());
    assert!(parse_resolution("0p").is_err());
    assert!(parse_size("0MB").is_err());
}

#[test]
fn convert_quality_changes_transcode_crf() {
    for (quality, expected_crf) in [
        ("lossless", "0"),
        ("very-high", "18"),
        ("high", "20"),
        ("balanced", "23"),
        ("small", "28"),
        ("tiny", "32"),
    ] {
        let args = video_encode_args("h264", quality, None).unwrap();
        assert!(args.windows(2).any(|pair| pair == ["-crf", expected_crf]));
    }
}

#[test]
fn software_encoder_overrides_are_reflected_in_codec_args() {
    assert_eq!(software_encoder_candidates("av1"), ["libsvtav1", "libaom-av1"]);
    let args = video_encode_args("av1", "tiny", Some("libsvtav1")).unwrap();
    assert_eq!(args.get(1).map(String::as_str), Some("libsvtav1"));
    let vp9 = video_encode_args("vp9", "balanced", Some("libvpx-vp9")).unwrap();
    assert_eq!(vp9.get(1).map(String::as_str), Some("libvpx-vp9"));
    assert_eq!(default_video_codec_for_container("webm"), "vp9");
    assert_eq!(default_audio_codec_for_container("webm"), "opus");
    assert!(validate_transcode_compatibility("video", "transcode", "h264", "webm").is_err());
    assert!(!is_hardware_encoder("libsvtav1"));
    assert!(is_hardware_encoder("h264_videotoolbox"));
}

#[test]
fn subtitle_mapping_does_not_duplicate_existing_full_map() {
    let streams = vec![json!({"codec_type":"subtitle"})];
    assert_eq!(subtitle_ffmpeg_args("mp4", &streams), ["-map", "0:s?", "-c:s", "mov_text"]);
    assert_eq!(subtitle_codec_args("mp4", &streams), ["-c:s", "mov_text"]);
}

#[test]
fn unsupported_subtitle_codecs_are_explicitly_warned() {
    let streams = vec![json!({"codec_type":"subtitle","codec_name":"hdmv_pgs_subtitle"})];
    assert_eq!(subtitle_strategy("mp4", &streams), "warning");
    assert_eq!(subtitle_warnings(&streams, "mp4").len(), 1);
}

#[test]
fn derives_video_bit_depth_from_pixel_format() {
    assert_eq!(bit_depth(&json!({"pix_fmt":"yuv420p10le"})), Some(10));
    assert_eq!(bit_depth(&json!({"pix_fmt":"yuv420p"})), Some(8));
    assert_eq!(bit_depth(&json!({})), None);
}

#[test]
fn expanded_format_matrix_routes_audio_and_video_codecs() {
    assert_eq!(normalize_container("wmv").unwrap(), "wmv");
    assert_eq!(normalize_container("3gp").unwrap(), "3gp");
    assert_eq!(normalize_audio_format("wma").unwrap(), "wma");
    assert_eq!(audio_codec_for_format("ogg"), "vorbis");
    assert_eq!(audio_output_extension("alac"), "m4a");
    assert!(is_video_compatible("flv", "h264"));
    assert!(is_audio_compatible("wmv", "wmav2"));
    assert!(!is_audio_compatible("mpeg", "aac"));
    assert!(is_audio_compatible("vob", "ac3"));
    assert!(software_encoder_candidates("mpeg2video").contains(&"mpeg2video"));
    assert!(audio_encode_args("vorbis", "96k").unwrap().contains(&"-strict".to_string()));
}

#[test]
fn image_and_edit_helpers_validate_safe_operations() {
    assert_eq!(normalize_image_format(".jpeg").unwrap(), "jpg");
    assert_eq!(rotate_filter(90).unwrap(), "transpose=1");
    assert!(rotate_filter(45).is_err());
    assert_eq!(parse_crop("320:240:0:0").unwrap(), "crop=320:240:0:0");
    assert!(parse_crop("320x240").is_err());
    assert_eq!(named_video_filter("grayscale").unwrap(), "hue=s=0");
    assert_eq!(atempo_filter(4.0), "atempo=2.0,atempo=2.000000");
}

#[test]
fn subtitle_styles_and_disc_actions_are_bounded() {
    let subtitle =
        subtitle_filter(Path::new("captions.srt"), Some("FontSize=24,PrimaryColour=&H00FFFFFF"))
            .unwrap();
    assert!(subtitle.contains("force_style='FontSize=24,PrimaryColour=&H00FFFFFF'"));
    assert!(subtitle_filter(Path::new("captions.srt"), Some("bad;graph")).is_err());
    assert!(normalize_disc_action("create-iso").is_ok());
    assert_eq!(default_disc_kind("disc"), "dvd");
}

#[test]
fn gif_alias_is_normalized() {
    assert!(normalize_operation("video-to-gif") == "video_to_gif");
    assert_eq!(normalize_operation("gif-convert"), "gif_convert");
}

#[test]
fn device_presets_are_explicit_and_deterministic() {
    let profile = device_profile("psp").unwrap();
    assert_eq!(profile.container, "mp4");
    assert_eq!(profile.max_height, 480);
    assert!(device_profile("unknown").is_err());
    assert_eq!(device_presets().len(), 5);
}
