use super::prelude::{Args, Deserialize, Parser, PathBuf, Serialize, Subcommand, ValueEnum};

pub(crate) const DEFAULT_AUDIO_BITRATE: &str = "256k";
pub(crate) const MAX_CAPTURED_STDERR_BYTES: usize = 256 * 1024;
pub(crate) const H264_HARDWARE_ENCODERS: &[&str] =
    &["h264_videotoolbox", "h264_nvenc", "h264_qsv", "h264_amf"];
pub(crate) const HEVC_HARDWARE_ENCODERS: &[&str] =
    &["hevc_videotoolbox", "hevc_nvenc", "hevc_qsv", "hevc_amf"];
pub(crate) const AV1_HARDWARE_ENCODERS: &[&str] = &["av1_nvenc", "av1_qsv", "av1_amf"];

#[derive(Parser, Debug)]
#[command(name = "media", version, about = "Deterministic media tooling for AI agents")]
pub(crate) struct Cli {
    #[arg(long, global = true, help = "Emit one stable JSON object on stdout")]
    pub(crate) json: bool,
    #[arg(long, global = true, help = "Print the planned command without executing it")]
    pub(crate) dry_run: bool,
    #[arg(long, global = true, help = "Allow replacing an existing output path")]
    pub(crate) overwrite: bool,
    #[arg(long, global = true, help = "Write diagnostic process output to stderr")]
    pub(crate) verbose: bool,
    #[arg(
        long,
        global = true,
        help = "Emit progress on stderr (human text, or NDJSON with --json)"
    )]
    pub(crate) progress: bool,
    #[arg(
        long,
        global = true,
        help = "Enable verbose diagnostics (same stderr channel as --verbose)"
    )]
    pub(crate) debug: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    Inspect(InputArgs),
    Plan(PlanArgs),
    Convert(ConvertArgs),
    Compress(CompressArgs),
    Resize(ResizeArgs),
    Clip(ClipArgs),
    ExtractAudio(ExtractAudioArgs),
    Thumbnail(ThumbnailArgs),
    Image(ImageArgs),
    Gif(GifArgs),
    Edit(EditArgs),
    Merge(MergeArgs),
    Audio(AudioArgs),
    Repair(RepairArgs),
    Disc(DiscArgs),
    Batch(BatchArgs),
    Verify(VerifyArgs),
    Capabilities,
    Presets,
    Tool(ToolArgs),
    Ffmpeg(FfmpegArgs),
}

#[derive(Args, Debug, Clone)]
pub(crate) struct InputArgs {
    pub(crate) input: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct PlanArgs {
    pub(crate) input: PathBuf,
    #[arg(long = "input-extra", value_name = "PATH", num_args = 1..)]
    pub(crate) inputs: Option<Vec<PathBuf>>,
    #[arg(
        long,
        help = "Semantic operation to plan; inferred from operation-specific flags when omitted"
    )]
    pub(crate) operation: Option<String>,
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC")]
    pub(crate) video_codec: Option<String>,
    #[arg(long, value_name = "CODEC")]
    pub(crate) audio_codec: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) hardware: Option<HardwareMode>,
    #[arg(long, value_enum)]
    pub(crate) quality: Option<Quality>,
    #[arg(long)]
    pub(crate) target_size: Option<String>,
    #[arg(long)]
    pub(crate) width: Option<u32>,
    #[arg(long)]
    pub(crate) resolution: Option<String>,
    #[arg(long)]
    pub(crate) start: Option<String>,
    #[arg(long)]
    pub(crate) duration: Option<String>,
    #[arg(long)]
    pub(crate) end: Option<String>,
    #[arg(long)]
    pub(crate) format: Option<String>,
    #[arg(long)]
    pub(crate) at: Option<String>,
    #[arg(long)]
    pub(crate) fps: Option<u32>,
    #[arg(long, help = "Device output preset, for example iphone or psp")]
    pub(crate) device: Option<String>,
    #[arg(long)]
    pub(crate) height: Option<u32>,
    #[arg(long)]
    pub(crate) crop: Option<String>,
    #[arg(long)]
    pub(crate) rotate: Option<u16>,
    #[arg(long)]
    pub(crate) speed: Option<f64>,
    #[arg(long)]
    pub(crate) volume: Option<f64>,
    #[arg(long)]
    pub(crate) filter: Option<String>,
    #[arg(long)]
    pub(crate) subtitle: Option<PathBuf>,
    #[arg(long)]
    pub(crate) subtitle_style: Option<String>,
    #[arg(long)]
    pub(crate) watermark: Option<PathBuf>,
    #[arg(long)]
    pub(crate) image_quality: Option<u8>,
    #[arg(long)]
    pub(crate) bitrate: Option<String>,
    #[arg(long)]
    pub(crate) sample_rate: Option<u32>,
    #[arg(long)]
    pub(crate) channels: Option<u8>,
    #[arg(long)]
    pub(crate) reencode: bool,
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long, default_value = "extract")]
    pub(crate) action: String,
    #[arg(long)]
    pub(crate) volume_label: Option<String>,
    #[arg(long, default_value = "concat")]
    pub(crate) mode: String,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ConvertArgs {
    pub(crate) input: PathBuf,
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC")]
    pub(crate) video_codec: Option<String>,
    #[arg(long, value_name = "CODEC")]
    pub(crate) audio_codec: Option<String>,
    #[arg(long, value_enum)]
    pub(crate) hardware: Option<HardwareMode>,
    #[arg(long, value_enum)]
    pub(crate) quality: Option<Quality>,
    #[arg(long, help = "Device output preset, for example iphone or psp")]
    pub(crate) device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CompressArgs {
    pub(crate) input: PathBuf,
    #[arg(long, value_enum)]
    pub(crate) quality: Option<Quality>,
    #[arg(long, help = "Target output size, e.g. 500MB or 1.5GB")]
    pub(crate) target_size: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub(crate) hardware: Option<HardwareMode>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ResizeArgs {
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) width: Option<u32>,
    #[arg(long)]
    pub(crate) resolution: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ClipArgs {
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) start: String,
    #[arg(long)]
    pub(crate) duration: Option<String>,
    #[arg(long)]
    pub(crate) end: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ExtractAudioArgs {
    pub(crate) input: PathBuf,
    #[arg(long, default_value = "m4a")]
    pub(crate) format: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ThumbnailArgs {
    pub(crate) input: PathBuf,
    #[arg(long, default_value = "0")]
    pub(crate) at: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ImageArgs {
    pub(crate) input: PathBuf,
    #[arg(long, help = "Target image format: png, jpg, webp, gif, bmp, tiff, ico, tga, avif")]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, help = "Target width in pixels")]
    pub(crate) width: Option<u32>,
    #[arg(long, help = "Target height in pixels")]
    pub(crate) height: Option<u32>,
    #[arg(long, help = "Rotate by 90, 180, or 270 degrees")]
    pub(crate) rotate: Option<u16>,
    #[arg(long, help = "Overlay a watermark image in the bottom-right corner")]
    pub(crate) watermark: Option<PathBuf>,
    #[arg(long, value_name = "1-100", help = "Image quality for lossy formats")]
    pub(crate) image_quality: Option<u8>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct GifArgs {
    pub(crate) input: PathBuf,
    #[arg(long, default_value = "0", help = "Start position in seconds or HH:MM:SS")]
    pub(crate) start: String,
    #[arg(long, default_value = "3", help = "Animated GIF duration in seconds")]
    pub(crate) duration: String,
    #[arg(long, default_value_t = 12, help = "GIF frame rate between 1 and 60")]
    pub(crate) fps: u32,
    #[arg(long, help = "Output width; height preserves aspect ratio")]
    pub(crate) width: Option<u32>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct EditArgs {
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, help = "Crop as WIDTH:HEIGHT:X:Y")]
    pub(crate) crop: Option<String>,
    #[arg(long, help = "Rotate by 90, 180, or 270 degrees")]
    pub(crate) rotate: Option<u16>,
    #[arg(long, help = "Playback speed between 0.25 and 4.0")]
    pub(crate) speed: Option<f64>,
    #[arg(long, help = "Audio volume multiplier between 0 and 10")]
    pub(crate) volume: Option<f64>,
    #[arg(long, help = "Named filter: grayscale, blur, sharpen, vintage")]
    pub(crate) filter: Option<String>,
    #[arg(long, help = "Burn an external subtitle file into the video")]
    pub(crate) subtitle: Option<PathBuf>,
    #[arg(long, help = "ASS/SSA force_style string, e.g. FontSize=24,PrimaryColour=&H00FFFFFF")]
    pub(crate) subtitle_style: Option<String>,
    #[arg(long)]
    pub(crate) start: Option<String>,
    #[arg(long)]
    pub(crate) duration: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct MergeArgs {
    #[arg(required = true, num_args = 2..)]
    pub(crate) inputs: Vec<PathBuf>,
    #[arg(long, default_value = "concat", help = "Merge mode: concat, mux, or mix")]
    pub(crate) mode: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct AudioArgs {
    pub(crate) input: PathBuf,
    #[arg(long, default_value = "m4a")]
    pub(crate) format: String,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, help = "Audio bitrate such as 128k or 1M")]
    pub(crate) bitrate: Option<String>,
    #[arg(long, help = "Sample rate in Hz")]
    pub(crate) sample_rate: Option<u32>,
    #[arg(long, help = "Number of output channels")]
    pub(crate) channels: Option<u8>,
    #[arg(long, help = "Volume multiplier between 0 and 10")]
    pub(crate) volume: Option<f64>,
    #[arg(long)]
    pub(crate) start: Option<String>,
    #[arg(long)]
    pub(crate) duration: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct RepairArgs {
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    #[arg(long, help = "Re-encode streams instead of attempting a lossless repair")]
    pub(crate) reencode: bool,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct DiscArgs {
    pub(crate) input: PathBuf,
    #[arg(long, default_value = "dvd", help = "Disc source kind: dvd, cd, or iso")]
    pub(crate) kind: String,
    #[arg(long, default_value = "extract", help = "Disc action: extract or create-iso")]
    pub(crate) action: String,
    #[arg(long, help = "ISO volume label when creating an image")]
    pub(crate) volume_label: Option<String>,
    #[arg(long, help = "Target output format, for example mp4 or flac")]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct BatchArgs {
    pub(crate) input: String,
    #[arg(long, value_name = "FORMAT")]
    pub(crate) convert: Option<String>,
    #[arg(long)]
    pub(crate) recursive: bool,
    #[arg(long)]
    pub(crate) output_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct VerifyArgs {
    pub(crate) input: PathBuf,
    pub(crate) output: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ToolArgs {
    #[arg(long, help = "Inline JSON request; defaults to reading one object from stdin")]
    pub(crate) request: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct FfmpegArgs {
    #[arg(last = true, allow_hyphen_values = true)]
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum HardwareMode {
    Auto,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Quality {
    Lossless,
    VeryHigh,
    High,
    Balanced,
    Small,
    Tiny,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ConfigFile {
    pub(crate) default_quality: Option<String>,
    pub(crate) hardware: Option<String>,
    pub(crate) overwrite: Option<bool>,
    pub(crate) verify_after_execute: Option<bool>,
    pub(crate) progress: Option<bool>,
    pub(crate) video: Option<ConfigCodec>,
    pub(crate) audio: Option<ConfigCodec>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ConfigCodec {
    pub(crate) preferred_codec: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ToolRequest {
    pub(crate) operation: String,
    pub(crate) target_operation: Option<String>,
    pub(crate) input: Option<String>,
    pub(crate) inputs: Option<Vec<String>>,
    pub(crate) output: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) video_codec: Option<String>,
    pub(crate) audio_codec: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) target_size: Option<String>,
    pub(crate) hardware: Option<String>,
    pub(crate) width: Option<u32>,
    pub(crate) resolution: Option<String>,
    pub(crate) start: Option<String>,
    pub(crate) duration: Option<String>,
    pub(crate) end: Option<String>,
    pub(crate) format: Option<String>,
    pub(crate) at: Option<String>,
    pub(crate) fps: Option<u32>,
    pub(crate) device: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) crop: Option<String>,
    pub(crate) rotate: Option<u16>,
    pub(crate) speed: Option<f64>,
    pub(crate) volume: Option<f64>,
    pub(crate) filter: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) subtitle_style: Option<String>,
    pub(crate) watermark: Option<String>,
    pub(crate) image_quality: Option<u8>,
    pub(crate) height: Option<u32>,
    pub(crate) bitrate: Option<String>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) channels: Option<u8>,
    pub(crate) reencode: Option<bool>,
    pub(crate) kind: Option<String>,
    pub(crate) action: Option<String>,
    pub(crate) volume_label: Option<String>,
    pub(crate) recursive: Option<bool>,
    pub(crate) output_dir: Option<String>,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) dry_run: Option<bool>,
    pub(crate) overwrite: Option<bool>,
    pub(crate) verify_after_execute: Option<bool>,
    pub(crate) progress: Option<bool>,
}
