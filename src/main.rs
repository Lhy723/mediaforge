use clap::{error::ErrorKind, Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const DEFAULT_AUDIO_BITRATE: &str = "256k";
const MAX_CAPTURED_STDERR_BYTES: usize = 256 * 1024;
const H264_HARDWARE_ENCODERS: &[&str] =
    &["h264_videotoolbox", "h264_nvenc", "h264_qsv", "h264_amf"];
const HEVC_HARDWARE_ENCODERS: &[&str] =
    &["hevc_videotoolbox", "hevc_nvenc", "hevc_qsv", "hevc_amf"];
const AV1_HARDWARE_ENCODERS: &[&str] = &["av1_nvenc", "av1_qsv", "av1_amf"];

#[derive(Parser, Debug)]
#[command(name = "media", version, about = "Deterministic media tooling for AI agents")]
struct Cli {
    #[arg(long, global = true, help = "Emit one stable JSON object on stdout")]
    json: bool,
    #[arg(long, global = true, help = "Print the planned command without executing it")]
    dry_run: bool,
    #[arg(long, global = true, help = "Allow replacing an existing output path")]
    overwrite: bool,
    #[arg(long, global = true, help = "Write diagnostic process output to stderr")]
    verbose: bool,
    #[arg(
        long,
        global = true,
        help = "Emit progress on stderr (human text, or NDJSON with --json)"
    )]
    progress: bool,
    #[arg(
        long,
        global = true,
        help = "Enable verbose diagnostics (same stderr channel as --verbose)"
    )]
    debug: bool,
    #[command(subcommand)]
    command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum Command {
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
struct InputArgs {
    input: PathBuf,
}

#[derive(Args, Debug, Clone)]
struct PlanArgs {
    input: PathBuf,
    #[arg(long = "input-extra", value_name = "PATH", num_args = 1..)]
    inputs: Option<Vec<PathBuf>>,
    #[arg(
        long,
        help = "Semantic operation to plan; inferred from operation-specific flags when omitted"
    )]
    operation: Option<String>,
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC")]
    video_codec: Option<String>,
    #[arg(long, value_name = "CODEC")]
    audio_codec: Option<String>,
    #[arg(long, value_enum)]
    hardware: Option<HardwareMode>,
    #[arg(long, value_enum)]
    quality: Option<Quality>,
    #[arg(long)]
    target_size: Option<String>,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    resolution: Option<String>,
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    duration: Option<String>,
    #[arg(long)]
    end: Option<String>,
    #[arg(long)]
    format: Option<String>,
    #[arg(long)]
    at: Option<String>,
    #[arg(long)]
    fps: Option<u32>,
    #[arg(long, help = "Device output preset, for example iphone or psp")]
    device: Option<String>,
    #[arg(long)]
    height: Option<u32>,
    #[arg(long)]
    crop: Option<String>,
    #[arg(long)]
    rotate: Option<u16>,
    #[arg(long)]
    speed: Option<f64>,
    #[arg(long)]
    volume: Option<f64>,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    subtitle: Option<PathBuf>,
    #[arg(long)]
    subtitle_style: Option<String>,
    #[arg(long)]
    watermark: Option<PathBuf>,
    #[arg(long)]
    image_quality: Option<u8>,
    #[arg(long)]
    bitrate: Option<String>,
    #[arg(long)]
    sample_rate: Option<u32>,
    #[arg(long)]
    channels: Option<u8>,
    #[arg(long)]
    reencode: bool,
    #[arg(long)]
    kind: Option<String>,
    #[arg(long, default_value = "extract")]
    action: String,
    #[arg(long)]
    volume_label: Option<String>,
    #[arg(long, default_value = "concat")]
    mode: String,
}

#[derive(Args, Debug, Clone)]
struct ConvertArgs {
    input: PathBuf,
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC")]
    video_codec: Option<String>,
    #[arg(long, value_name = "CODEC")]
    audio_codec: Option<String>,
    #[arg(long, value_enum)]
    hardware: Option<HardwareMode>,
    #[arg(long, value_enum)]
    quality: Option<Quality>,
    #[arg(long, help = "Device output preset, for example iphone or psp")]
    device: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct CompressArgs {
    input: PathBuf,
    #[arg(long, value_enum)]
    quality: Option<Quality>,
    #[arg(long, help = "Target output size, e.g. 500MB or 1.5GB")]
    target_size: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum)]
    hardware: Option<HardwareMode>,
}

#[derive(Args, Debug, Clone)]
struct ResizeArgs {
    input: PathBuf,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    resolution: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ClipArgs {
    input: PathBuf,
    #[arg(long)]
    start: String,
    #[arg(long)]
    duration: Option<String>,
    #[arg(long)]
    end: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ExtractAudioArgs {
    input: PathBuf,
    #[arg(long, default_value = "m4a")]
    format: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ThumbnailArgs {
    input: PathBuf,
    #[arg(long, default_value = "0")]
    at: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct ImageArgs {
    input: PathBuf,
    #[arg(long, help = "Target image format: png, jpg, webp, gif, bmp, tiff, ico, tga, avif")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, help = "Target width in pixels")]
    width: Option<u32>,
    #[arg(long, help = "Target height in pixels")]
    height: Option<u32>,
    #[arg(long, help = "Rotate by 90, 180, or 270 degrees")]
    rotate: Option<u16>,
    #[arg(long, help = "Overlay a watermark image in the bottom-right corner")]
    watermark: Option<PathBuf>,
    #[arg(long, value_name = "1-100", help = "Image quality for lossy formats")]
    image_quality: Option<u8>,
}

#[derive(Args, Debug, Clone)]
struct GifArgs {
    input: PathBuf,
    #[arg(long, default_value = "0", help = "Start position in seconds or HH:MM:SS")]
    start: String,
    #[arg(long, default_value = "3", help = "Animated GIF duration in seconds")]
    duration: String,
    #[arg(long, default_value_t = 12, help = "GIF frame rate between 1 and 60")]
    fps: u32,
    #[arg(long, help = "Output width; height preserves aspect ratio")]
    width: Option<u32>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct EditArgs {
    input: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, help = "Crop as WIDTH:HEIGHT:X:Y")]
    crop: Option<String>,
    #[arg(long, help = "Rotate by 90, 180, or 270 degrees")]
    rotate: Option<u16>,
    #[arg(long, help = "Playback speed between 0.25 and 4.0")]
    speed: Option<f64>,
    #[arg(long, help = "Audio volume multiplier between 0 and 10")]
    volume: Option<f64>,
    #[arg(long, help = "Named filter: grayscale, blur, sharpen, vintage")]
    filter: Option<String>,
    #[arg(long, help = "Burn an external subtitle file into the video")]
    subtitle: Option<PathBuf>,
    #[arg(long, help = "ASS/SSA force_style string, e.g. FontSize=24,PrimaryColour=&H00FFFFFF")]
    subtitle_style: Option<String>,
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    duration: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct MergeArgs {
    #[arg(required = true, num_args = 2..)]
    inputs: Vec<PathBuf>,
    #[arg(long, default_value = "concat", help = "Merge mode: concat, mux, or mix")]
    mode: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct AudioArgs {
    input: PathBuf,
    #[arg(long, default_value = "m4a")]
    format: String,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, help = "Audio bitrate such as 128k or 1M")]
    bitrate: Option<String>,
    #[arg(long, help = "Sample rate in Hz")]
    sample_rate: Option<u32>,
    #[arg(long, help = "Number of output channels")]
    channels: Option<u8>,
    #[arg(long, help = "Volume multiplier between 0 and 10")]
    volume: Option<f64>,
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    duration: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct RepairArgs {
    input: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, help = "Re-encode streams instead of attempting a lossless repair")]
    reencode: bool,
}

#[derive(Args, Debug, Clone)]
struct DiscArgs {
    input: PathBuf,
    #[arg(long, default_value = "dvd", help = "Disc source kind: dvd, cd, or iso")]
    kind: String,
    #[arg(long, default_value = "extract", help = "Disc action: extract or create-iso")]
    action: String,
    #[arg(long, help = "ISO volume label when creating an image")]
    volume_label: Option<String>,
    #[arg(long, help = "Target output format, for example mp4 or flac")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct BatchArgs {
    input: String,
    #[arg(long, value_name = "FORMAT")]
    convert: Option<String>,
    #[arg(long)]
    recursive: bool,
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
struct VerifyArgs {
    input: PathBuf,
    output: PathBuf,
}

#[derive(Args, Debug, Clone)]
struct ToolArgs {
    #[arg(long, help = "Inline JSON request; defaults to reading one object from stdin")]
    request: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct FfmpegArgs {
    #[arg(last = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HardwareMode {
    Auto,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Quality {
    Lossless,
    VeryHigh,
    High,
    Balanced,
    Small,
    Tiny,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ConfigFile {
    default_quality: Option<String>,
    hardware: Option<String>,
    overwrite: Option<bool>,
    verify_after_execute: Option<bool>,
    progress: Option<bool>,
    video: Option<ConfigCodec>,
    audio: Option<ConfigCodec>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ConfigCodec {
    preferred_codec: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolRequest {
    operation: String,
    target_operation: Option<String>,
    input: Option<String>,
    inputs: Option<Vec<String>>,
    output: Option<String>,
    output_format: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    quality: Option<String>,
    target_size: Option<String>,
    hardware: Option<String>,
    width: Option<u32>,
    resolution: Option<String>,
    start: Option<String>,
    duration: Option<String>,
    end: Option<String>,
    format: Option<String>,
    at: Option<String>,
    fps: Option<u32>,
    device: Option<String>,
    mode: Option<String>,
    crop: Option<String>,
    rotate: Option<u16>,
    speed: Option<f64>,
    volume: Option<f64>,
    filter: Option<String>,
    subtitle: Option<String>,
    subtitle_style: Option<String>,
    watermark: Option<String>,
    image_quality: Option<u8>,
    height: Option<u32>,
    bitrate: Option<String>,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    reencode: Option<bool>,
    kind: Option<String>,
    action: Option<String>,
    volume_label: Option<String>,
    recursive: Option<bool>,
    output_dir: Option<String>,
    args: Option<Vec<String>>,
    dry_run: Option<bool>,
    overwrite: Option<bool>,
    verify_after_execute: Option<bool>,
    progress: Option<bool>,
}

#[derive(Debug, Error)]
#[error("{message}")]
struct AppError {
    code: String,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    details: Value,
    suggestions: Vec<String>,
}

impl AppError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            source: None,
            details: Value::Object(Map::new()),
            suggestions: Vec::new(),
        }
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    fn with_suggestions(mut self, suggestions: &[&str]) -> Self {
        self.suggestions = suggestions.iter().map(|item| (*item).to_string()).collect();
        self
    }

    fn from_io(code: &str, message: impl Into<String>, source: io::Error) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            source: Some(Box::new(source)),
            details: Value::Object(Map::new()),
            suggestions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Context {
    json: bool,
    dry_run: bool,
    overwrite: bool,
    verbose: bool,
    verify_after_execute: bool,
    progress: bool,
    default_quality: Quality,
    default_hardware: HardwareMode,
    default_video_codec: String,
    default_audio_codec: String,
}

#[derive(Debug, Clone)]
struct Probe {
    raw: Value,
    duration_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
struct OperationPlan {
    value: Value,
    output: PathBuf,
    args: Vec<String>,
    strategy: String,
}

#[derive(Debug, Clone)]
struct HardwareSelection {
    requested: String,
    selected: String,
    encoder: Option<String>,
    reason: String,
}

#[derive(Debug, Clone, Copy)]
struct DeviceProfile {
    id: &'static str,
    label: &'static str,
    container: &'static str,
    video_codec: &'static str,
    audio_codec: &'static str,
    max_height: u32,
}

fn main() {
    let raw_args = std::env::args().skip(1).collect::<Vec<_>>();
    let command_token = raw_args.iter().find(|arg| !arg.starts_with('-'));
    let json_requested = raw_args.iter().any(|arg| arg == "--json")
        || command_token.is_some_and(|arg| arg == "tool");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) =>
        {
            error.exit()
        }
        Err(error) if json_requested => {
            print_json(&json!({
                "status": "error",
                "code": "INVALID_ARGUMENT",
                "message": "Invalid command-line arguments.",
                "details": {"usage": error.to_string()},
                "suggestions": ["Run media --help to inspect valid commands and options."]
            }));
            std::process::exit(2);
        }
        Err(error) => error.exit(),
    };
    let tool_mode = matches!(&cli.command, Command::Tool(_));
    let json_mode = cli.json || tool_mode;
    let config = match load_config() {
        Ok(config) => config,
        Err(error) => {
            let value = json!({
                "status": "error",
                "code": error.code,
                "message": error.message,
                "details": error.details,
                "suggestions": error.suggestions,
            });
            if json_mode {
                print_json(&value);
            } else {
                eprintln!(
                    "media: {}",
                    value["message"].as_str().unwrap_or("Invalid configuration.")
                );
            }
            std::process::exit(1);
        }
    };
    let default_quality = match parse_quality_name(
        config.default_quality.clone().unwrap_or_else(|| "balanced".to_string()),
    ) {
        Ok(value) => value,
        Err(error) => {
            if json_mode {
                print_json(&json!({
                    "status": "error",
                    "code": error.code,
                    "message": error.message,
                    "details": error.details,
                    "suggestions": error.suggestions,
                }));
            } else {
                eprintln!("media: {}", error.message);
            }
            std::process::exit(1);
        }
    };
    let default_hardware =
        match parse_hardware_name(config.hardware.clone().unwrap_or_else(|| "auto".to_string())) {
            Ok(value) => value,
            Err(error) => {
                if json_mode {
                    print_json(&json!({
                        "status": "error",
                        "code": error.code,
                        "message": error.message,
                        "details": error.details,
                        "suggestions": error.suggestions,
                    }));
                } else {
                    eprintln!("media: {}", error.message);
                }
                std::process::exit(1);
            }
        };
    let context = Context {
        json: json_mode,
        dry_run: cli.dry_run,
        overwrite: cli.overwrite,
        verbose: cli.verbose || cli.debug,
        verify_after_execute: config.verify_after_execute.unwrap_or(true),
        progress: cli.progress || config.progress.unwrap_or(false),
        default_quality,
        default_hardware,
        default_video_codec: config
            .video
            .as_ref()
            .and_then(|value| value.preferred_codec.clone())
            .unwrap_or_else(|| "auto".to_string()),
        default_audio_codec: config
            .audio
            .as_ref()
            .and_then(|value| value.preferred_codec.clone())
            .unwrap_or_else(|| "auto".to_string()),
    };
    let result = dispatch(&context, cli.command);
    match result {
        Ok(value) => {
            if context.json {
                print_json(&value);
            } else {
                print_human(&value);
            }
        }
        Err(error) => {
            if context.json {
                print_json(&json!({
                    "status": "error",
                    "code": error.code,
                    "message": error.message,
                    "details": error.details,
                    "suggestions": error.suggestions,
                }));
            } else {
                eprintln!("media: {}: {}", error.code, error.message);
                if !error.suggestions.is_empty() {
                    eprintln!("suggestions:");
                    for suggestion in error.suggestions {
                        eprintln!("  - {suggestion}");
                    }
                }
            }
            std::process::exit(1);
        }
    }
}

fn dispatch(context: &Context, command: Command) -> Result<Value, AppError> {
    match command {
        Command::Inspect(args) => inspect_command(context, &args.input),
        Command::Plan(args) => plan_command(context, &args),
        Command::Convert(args) => convert_command(context, &args),
        Command::Compress(args) => compress_command(context, &args),
        Command::Resize(args) => resize_command(context, &args),
        Command::Clip(args) => clip_command(context, &args),
        Command::ExtractAudio(args) => extract_audio_command(context, &args),
        Command::Thumbnail(args) => thumbnail_command(context, &args),
        Command::Image(args) => image_command(context, &args),
        Command::Gif(args) => gif_command(context, &args),
        Command::Edit(args) => edit_command(context, &args),
        Command::Merge(args) => merge_command(context, &args),
        Command::Audio(args) => audio_command(context, &args),
        Command::Repair(args) => repair_command(context, &args),
        Command::Disc(args) => disc_command(context, &args),
        Command::Batch(args) => batch_command(context, &args),
        Command::Verify(args) => verify_command(context, &args.input, &args.output),
        Command::Capabilities => capabilities_command(context),
        Command::Presets => presets_command(context),
        Command::Tool(args) => tool_command(context, &args),
        Command::Ffmpeg(args) => raw_ffmpeg_command(context, &args.args),
    }
}

fn tool_command(context: &Context, args: &ToolArgs) -> Result<Value, AppError> {
    let request_text = if let Some(request) = &args.request {
        request.clone()
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).map_err(|error| {
            AppError::from_io(
                "INVALID_ARGUMENT",
                "Could not read the Tool request from stdin.",
                error,
            )
        })?;
        input
    };
    if request_text.trim().is_empty() {
        return Err(AppError::new("INVALID_ARGUMENT", "Tool request must be a JSON object."));
    }
    let request: ToolRequest = serde_json::from_str(&request_text).map_err(|error| {
        AppError::new("INVALID_ARGUMENT", format!("Tool request is not valid JSON: {error}"))
            .with_suggestions(&["Send one JSON object with an operation and input field."])
    })?;
    let mut tool_context = context.clone();
    tool_context.json = true;
    if let Some(dry_run) = request.dry_run {
        tool_context.dry_run = dry_run;
    }
    // Config cannot enable overwrite implicitly: only the request or an explicit
    // CLI flag may relax the core safety policy.
    tool_context.overwrite = context.overwrite || request.overwrite.unwrap_or(false);
    tool_context.verify_after_execute =
        request.verify_after_execute.unwrap_or(context.verify_after_execute);
    tool_context.progress = request.progress.unwrap_or(context.progress);
    let video_codec = request.video_codec.clone();
    let audio_codec = request.audio_codec.clone();
    let requested_quality = request.quality.clone();
    let quality =
        requested_quality.as_ref().map(|value| parse_quality_name(value.clone())).transpose()?;
    let hardware =
        request.hardware.as_ref().map(|value| parse_hardware_name(value.clone())).transpose()?;
    let operation = normalize_operation(&request.operation);
    let input = || required_input(request.input.clone());
    match operation.as_str() {
        "inspect" => dispatch(&tool_context, Command::Inspect(InputArgs { input: input()? })),
        "plan" => dispatch(
            &tool_context,
            Command::Plan(PlanArgs {
                input: input()?,
                inputs: request
                    .inputs
                    .clone()
                    .map(|values| values.into_iter().map(PathBuf::from).collect()),
                operation: request.target_operation.clone(),
                to: request.output_format.clone(),
                output: request.output.clone().map(PathBuf::from),
                video_codec: video_codec.clone(),
                audio_codec: audio_codec.clone(),
                hardware,
                quality,
                target_size: request.target_size.clone(),
                width: request.width,
                resolution: request.resolution.clone(),
                start: request.start.clone(),
                duration: request.duration.clone(),
                end: request.end.clone(),
                format: request.format.clone(),
                at: request.at.clone(),
                fps: request.fps,
                device: request.device.clone(),
                height: request.height,
                crop: request.crop.clone(),
                rotate: request.rotate,
                speed: request.speed,
                volume: request.volume,
                filter: request.filter.clone(),
                subtitle: request.subtitle.clone().map(PathBuf::from),
                subtitle_style: request.subtitle_style.clone(),
                watermark: request.watermark.clone().map(PathBuf::from),
                image_quality: request.image_quality,
                bitrate: request.bitrate.clone(),
                sample_rate: request.sample_rate,
                channels: request.channels,
                reencode: request.reencode.unwrap_or(false),
                kind: request.kind.clone(),
                action: request.action.clone().unwrap_or_else(|| "extract".to_string()),
                volume_label: request.volume_label.clone(),
                mode: request.mode.clone().unwrap_or_else(|| "concat".to_string()),
            }),
        ),
        "convert" => dispatch(
            &tool_context,
            Command::Convert(ConvertArgs {
                input: input()?,
                to: request.output_format.clone(),
                output: request.output.clone().map(PathBuf::from),
                video_codec,
                audio_codec,
                hardware,
                quality,
                device: request.device.clone(),
            }),
        ),
        "compress" => dispatch(
            &tool_context,
            Command::Compress(CompressArgs {
                input: input()?,
                quality,
                target_size: request.target_size.clone(),
                output: request.output.clone().map(PathBuf::from),
                hardware,
            }),
        ),
        "resize" => dispatch(
            &tool_context,
            Command::Resize(ResizeArgs {
                input: input()?,
                width: request.width,
                resolution: request.resolution.clone(),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "clip" => dispatch(
            &tool_context,
            Command::Clip(ClipArgs {
                input: input()?,
                start: request.start.clone().ok_or_else(|| AppError::new("INVALID_ARGUMENT", "Tool clip requests require start."))?,
                duration: request.duration.clone(),
                end: request.end.clone(),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "extract_audio" => dispatch(
            &tool_context,
            Command::ExtractAudio(ExtractAudioArgs {
                input: input()?,
                format: request.format.clone().unwrap_or_else(|| "m4a".to_string()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "thumbnail" => dispatch(
            &tool_context,
            Command::Thumbnail(ThumbnailArgs {
                input: input()?,
                at: request.at.clone().unwrap_or_else(|| "0".to_string()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "image" | "image_convert" | "image_compress" | "compress_image" => dispatch(
            &tool_context,
            Command::Image(ImageArgs {
                input: input()?,
                to: request.output_format.clone().or(request.format.clone()),
                output: request.output.clone().map(PathBuf::from),
                width: request.width,
                height: request.height,
                rotate: request.rotate,
                watermark: request.watermark.clone().map(PathBuf::from),
                image_quality: request.image_quality,
            }),
        ),
        "gif" | "video_to_gif" | "gif_convert" => dispatch(
            &tool_context,
            Command::Gif(GifArgs {
                input: input()?,
                start: request.start.clone().unwrap_or_else(|| "0".to_string()),
                duration: request.duration.clone().unwrap_or_else(|| "3".to_string()),
                fps: request.fps.unwrap_or(12),
                width: request.width,
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "edit" | "edit_media" => dispatch(
            &tool_context,
            Command::Edit(EditArgs {
                input: input()?,
                output: request.output.clone().map(PathBuf::from),
                crop: request.crop.clone(),
                rotate: request.rotate,
                speed: request.speed,
                volume: request.volume,
                filter: request.filter.clone(),
                subtitle: request.subtitle.clone().map(PathBuf::from),
                subtitle_style: request.subtitle_style.clone(),
                start: request.start.clone(),
                duration: request.duration.clone(),
            }),
        ),
        "merge" | "concat" | "mux" | "mix" => {
            let mut inputs = Vec::new();
            if let Some(value) = request.input.clone() {
                inputs.push(PathBuf::from(value));
            }
            if let Some(value) = request.inputs.clone() {
                inputs.extend(value.into_iter().map(PathBuf::from));
            }
            dispatch(
                &tool_context,
                Command::Merge(MergeArgs {
                    inputs,
                    mode: request.mode.clone().unwrap_or_else(|| operation.clone()),
                    output: request.output.clone().map(PathBuf::from),
                }),
            )
        }
        "audio" | "audio_convert" | "compress_audio" => dispatch(
            &tool_context,
            Command::Audio(AudioArgs {
                input: input()?,
                format: request.format.clone().unwrap_or_else(|| "m4a".to_string()),
                output: request.output.clone().map(PathBuf::from),
                bitrate: request.bitrate.clone(),
                sample_rate: request.sample_rate,
                channels: request.channels,
                volume: request.volume,
                start: request.start.clone(),
                duration: request.duration.clone(),
            }),
        ),
        "repair" | "repair_media" => dispatch(
            &tool_context,
            Command::Repair(RepairArgs {
                input: input()?,
                output: request.output.clone().map(PathBuf::from),
                reencode: request.reencode.unwrap_or(false),
            }),
        ),
        "disc" | "dvd" | "cd" | "iso" => dispatch(
            &tool_context,
            Command::Disc(DiscArgs {
                input: input()?,
                kind: request.kind.clone().unwrap_or_else(|| default_disc_kind(&operation)),
                action: request.action.clone().unwrap_or_else(|| "extract".to_string()),
                volume_label: request.volume_label.clone(),
                to: request.output_format.clone().or(request.format.clone()),
                output: request.output.clone().map(PathBuf::from),
            }),
        ),
        "batch" => dispatch(
            &tool_context,
            Command::Batch(BatchArgs {
                input: input()?.to_string_lossy().to_string(),
                convert: request.output_format.clone(),
                recursive: request.recursive.unwrap_or(false),
                output_dir: request.output_dir.clone().map(PathBuf::from),
            }),
        ),
        "verify" => dispatch(
            &tool_context,
            Command::Verify(VerifyArgs {
                input: input()?,
                output: PathBuf::from(required_string(request.output.clone(), "output")?),
            }),
        ),
        "capabilities" => dispatch(&tool_context, Command::Capabilities),
        "presets" | "device_presets" => dispatch(&tool_context, Command::Presets),
        "ffmpeg" => raw_ffmpeg_command(&tool_context, &request.args.unwrap_or_default()),
        _ => Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("Unsupported Tool operation: {}", request.operation),
        )
        .with_suggestions(&[
            "Use a semantic operation such as inspect_media, plan_media_operation, convert_media, compress_media, resize_media, clip_media, extract_audio, create_thumbnail, image_convert, image_compress, edit_media, merge, audio_convert, repair_media, disc, presets, batch, verify_media, capabilities, or ffmpeg.",
        ])),
    }
}

fn normalize_operation(value: &str) -> String {
    match value.to_lowercase().replace('-', "_").as_str() {
        "inspect_media" => "inspect",
        "plan_media_operation" => "plan",
        "convert_media" => "convert",
        "compress_media" => "compress",
        "resize_media" => "resize",
        "clip_media" => "clip",
        "create_thumbnail" => "thumbnail",
        "verify_media" => "verify",
        operation => operation,
    }
    .to_string()
}

fn raw_ffmpeg_command(context: &Context, args: &[String]) -> Result<Value, AppError> {
    if args.is_empty() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "Raw FFmpeg requires arguments after `media ffmpeg --`.",
        ));
    }
    if context.dry_run {
        return Ok(json!({
            "status": "planned",
            "operation": "ffmpeg",
            "will_execute": false,
            "command": "ffmpeg",
            "args": args,
        }));
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = run_program("ffmpeg", &refs, context.verbose)?;
    Ok(json!({
        "status": "success",
        "operation": "ffmpeg",
        "command": "ffmpeg",
        "args": args,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }))
}

fn required_input(input: Option<String>) -> Result<PathBuf, AppError> {
    Ok(PathBuf::from(required_string(input, "input")?))
}

fn required_string(value: Option<String>, field: &str) -> Result<String, AppError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("INVALID_ARGUMENT", format!("Tool requests require {field}.")))
}

fn default_disc_kind(operation: &str) -> String {
    match operation {
        "dvd" | "cd" | "iso" => operation.to_string(),
        _ => "dvd".to_string(),
    }
}

fn parse_quality_name(value: String) -> Result<Quality, AppError> {
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

fn parse_hardware_name(value: String) -> Result<HardwareMode, AppError> {
    match value.to_lowercase().as_str() {
        "auto" => Ok(HardwareMode::Auto),
        "cpu" => Ok(HardwareMode::Cpu),
        "gpu" => Ok(HardwareMode::Gpu),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported hardware mode: {other}")))
        }
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MEDIAFORGE_CONFIG") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path).join("mediaforge/config.toml"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/mediaforge/config.toml"))
}

fn load_config() -> Result<ConfigFile, AppError> {
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

fn inspect_command(context: &Context, input: &Path) -> Result<Value, AppError> {
    let probe = probe_media(input, context.verbose)?;
    let file = fs::metadata(input).map_err(|error| {
        AppError::from_io("FILE_NOT_FOUND", format!("Cannot read {}", input.display()), error)
    })?;
    let format = probe.raw.get("format").cloned().unwrap_or_else(|| json!({}));
    let format_name = format
        .get("format_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_string();
    let format_name = inspect_container_label(input, &format_name);
    let duration = probe.duration_seconds.or_else(|| number_field(&format, "duration"));
    let bitrate = number_field(&format, "bit_rate").map(|value| value as u64);
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();

    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitle = Vec::new();
    for stream in streams {
        match stream.get("codec_type").and_then(Value::as_str).unwrap_or("") {
            "video" => video.push(normalize_video(&stream)),
            "audio" => audio.push(normalize_audio(&stream)),
            "subtitle" => subtitle.push(normalize_subtitle(&stream)),
            _ => {}
        }
    }

    Ok(json!({
        "status": "success",
        "operation": "inspect",
        "file": {
            "path": absolute_display(input),
            "size_bytes": file.len(),
            "container": format_name,
            "duration_seconds": duration,
            "bitrate": bitrate,
            "metadata": format.get("tags").cloned().unwrap_or_else(|| json!({})),
        },
        "video": video,
        "audio": audio,
        "subtitle": subtitle,
    }))
}

fn plan_command(context: &Context, args: &PlanArgs) -> Result<Value, AppError> {
    let planning_context = Context { dry_run: true, ..context.clone() };
    let operation = args.operation.as_deref().map(normalize_operation).unwrap_or_else(|| {
        if args.target_size.is_some()
            || (args.quality.is_some()
                && args.to.is_none()
                && args.video_codec.is_none()
                && args.audio_codec.is_none())
        {
            "compress".to_string()
        } else if args.width.is_some() || args.resolution.is_some() {
            "resize".to_string()
        } else if args.start.is_some() {
            "clip".to_string()
        } else if args.format.is_some() {
            "extract_audio".to_string()
        } else if args.at.is_some() {
            "thumbnail".to_string()
        } else {
            "convert".to_string()
        }
    });
    match operation.as_str() {
        "compress" => {
            let quality = args.quality.unwrap_or(planning_context.default_quality);
            return compress_command(
                &planning_context,
                &CompressArgs {
                    input: args.input.clone(),
                    quality: Some(quality),
                    target_size: args.target_size.clone(),
                    output: args.output.clone(),
                    hardware: args.hardware,
                },
            );
        }
        "resize" => {
            return resize_command(
                &planning_context,
                &ResizeArgs {
                    input: args.input.clone(),
                    width: args.width,
                    resolution: args.resolution.clone(),
                    output: args.output.clone(),
                },
            );
        }
        "clip" => {
            return clip_command(
                &planning_context,
                &ClipArgs {
                    input: args.input.clone(),
                    start: args.start.clone().unwrap_or_else(|| "0".to_string()),
                    duration: args.duration.clone(),
                    end: args.end.clone(),
                    output: args.output.clone(),
                },
            );
        }
        "extract_audio" => {
            return extract_audio_command(
                &planning_context,
                &ExtractAudioArgs {
                    input: args.input.clone(),
                    format: args.format.clone().unwrap_or_else(|| "m4a".to_string()),
                    output: args.output.clone(),
                },
            );
        }
        "thumbnail" => {
            return thumbnail_command(
                &planning_context,
                &ThumbnailArgs {
                    input: args.input.clone(),
                    at: args.at.clone().unwrap_or_else(|| "0".to_string()),
                    output: args.output.clone(),
                },
            );
        }
        "image" | "image_convert" | "image_compress" | "compress_image" => {
            return image_command(
                &planning_context,
                &ImageArgs {
                    input: args.input.clone(),
                    to: args.to.clone().or(args.format.clone()),
                    output: args.output.clone(),
                    width: args.width,
                    height: args.height,
                    rotate: args.rotate,
                    watermark: args.watermark.clone(),
                    image_quality: args.image_quality,
                },
            );
        }
        "gif" | "video_to_gif" | "gif_convert" => {
            return gif_command(
                &planning_context,
                &GifArgs {
                    input: args.input.clone(),
                    start: args.start.clone().unwrap_or_else(|| "0".to_string()),
                    duration: args.duration.clone().unwrap_or_else(|| "3".to_string()),
                    fps: args.fps.unwrap_or(12),
                    width: args.width,
                    output: args.output.clone(),
                },
            );
        }
        "edit" | "edit_media" => {
            return edit_command(
                &planning_context,
                &EditArgs {
                    input: args.input.clone(),
                    output: args.output.clone(),
                    crop: args.crop.clone(),
                    rotate: args.rotate,
                    speed: args.speed,
                    volume: args.volume,
                    filter: args.filter.clone(),
                    subtitle: args.subtitle.clone(),
                    subtitle_style: args.subtitle_style.clone(),
                    start: args.start.clone(),
                    duration: args.duration.clone(),
                },
            );
        }
        "merge" | "concat" | "mux" | "mix" => {
            let mut inputs = vec![args.input.clone()];
            inputs.extend(args.inputs.clone().unwrap_or_default());
            let merge_mode = match operation.as_str() {
                "mux" | "mix" | "concat" => operation.clone(),
                _ => args.mode.clone(),
            };
            return merge_command(
                &planning_context,
                &MergeArgs { inputs, mode: merge_mode, output: args.output.clone() },
            );
        }
        "audio" | "audio_convert" | "compress_audio" => {
            return audio_command(
                &planning_context,
                &AudioArgs {
                    input: args.input.clone(),
                    format: args
                        .format
                        .clone()
                        .or(args.to.clone())
                        .unwrap_or_else(|| "m4a".to_string()),
                    output: args.output.clone(),
                    bitrate: args.bitrate.clone(),
                    sample_rate: args.sample_rate,
                    channels: args.channels,
                    volume: args.volume,
                    start: args.start.clone(),
                    duration: args.duration.clone(),
                },
            );
        }
        "repair" | "repair_media" => {
            return repair_command(
                &planning_context,
                &RepairArgs {
                    input: args.input.clone(),
                    output: args.output.clone(),
                    reencode: args.reencode,
                },
            );
        }
        "disc" | "dvd" | "cd" | "iso" => {
            return disc_command(
                &planning_context,
                &DiscArgs {
                    input: args.input.clone(),
                    kind: args.kind.clone().unwrap_or_else(|| default_disc_kind(&operation)),
                    action: args.action.clone(),
                    volume_label: args.volume_label.clone(),
                    to: args.to.clone().or(args.format.clone()),
                    output: args.output.clone(),
                },
            );
        }
        "convert" => {}
        _ => {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                format!("Planning operation {operation} is not supported."),
            )
            .with_suggestions(&[
                "Use convert, compress, resize, clip, extract_audio, thumbnail, image, edit, merge, audio, repair, or disc.",
            ]));
        }
    }
    let profile = args.device.as_deref().map(device_profile).transpose()?;
    let target_container = args.to.as_deref().or(profile.map(|profile| profile.container));
    let target_video_codec = args
        .video_codec
        .as_deref()
        .or(profile.map(|profile| profile.video_codec))
        .unwrap_or(&planning_context.default_video_codec);
    let target_audio_codec = args
        .audio_codec
        .as_deref()
        .or(profile.map(|profile| profile.audio_codec))
        .unwrap_or(&planning_context.default_audio_codec);
    let mut plan = build_convert_plan(
        &planning_context,
        &args.input,
        target_container,
        args.output.as_deref(),
        target_video_codec,
        target_audio_codec,
        args.hardware.unwrap_or(planning_context.default_hardware),
        args.quality.unwrap_or(planning_context.default_quality),
    )?;
    if let Some(profile) = profile {
        apply_device_profile(&mut plan, profile);
    }
    let mut value = plan.value;
    if let Some(object) = value.as_object_mut() {
        object.insert("status".to_string(), json!("planned"));
        object.insert("will_execute".to_string(), json!(false));
    }
    Ok(value)
}

fn convert_command(context: &Context, args: &ConvertArgs) -> Result<Value, AppError> {
    let profile = args.device.as_deref().map(device_profile).transpose()?;
    let target_container = args.to.as_deref().or(profile.map(|profile| profile.container));
    let target_video_codec = args
        .video_codec
        .as_deref()
        .or(profile.map(|profile| profile.video_codec))
        .unwrap_or(&context.default_video_codec);
    let target_audio_codec = args
        .audio_codec
        .as_deref()
        .or(profile.map(|profile| profile.audio_codec))
        .unwrap_or(&context.default_audio_codec);
    let mut plan = build_convert_plan(
        context,
        &args.input,
        target_container,
        args.output.as_deref(),
        target_video_codec,
        target_audio_codec,
        args.hardware.unwrap_or(context.default_hardware),
        args.quality.unwrap_or(context.default_quality),
    )?;
    if let Some(profile) = profile {
        apply_device_profile(&mut plan, profile);
    }
    if context.dry_run {
        let mut value = plan.value;
        if let Some(object) = value.as_object_mut() {
            object.insert("status".to_string(), json!("planned"));
            object.insert("will_execute".to_string(), json!(false));
        }
        return Ok(value);
    }
    execute_plan(context, &args.input, &plan)
}

fn select_video_hardware(
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

fn hardware_encoder_candidates(codec: &str) -> &'static [&'static str] {
    match codec {
        "h264" => H264_HARDWARE_ENCODERS,
        "h265" | "hevc" => HEVC_HARDWARE_ENCODERS,
        "av1" => AV1_HARDWARE_ENCODERS,
        _ => &[],
    }
}

fn software_encoder_candidates(codec: &str) -> &'static [&'static str] {
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

fn select_software_video_encoder(context: &Context, codec: &str) -> Result<String, AppError> {
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

#[allow(clippy::too_many_arguments)]
fn build_convert_plan(
    context: &Context,
    input: &Path,
    to: Option<&str>,
    output: Option<&Path>,
    video_codec: &str,
    audio_codec: &str,
    hardware: HardwareMode,
    quality: Quality,
) -> Result<OperationPlan, AppError> {
    let video_codec = video_codec.to_lowercase();
    let audio_codec = audio_codec.to_lowercase();
    ensure_input(input)?;
    let probe = probe_media(input, context.verbose)?;
    let format = probe.raw.get("format").cloned().unwrap_or_else(|| json!({}));
    let source_container = format
        .get("format_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_lowercase();
    let source_container = internal_container(input, &source_container);
    let target_container = normalize_container(to.unwrap_or_else(|| {
        output
            .and_then(|path| path.extension().and_then(OsStr::to_str))
            .unwrap_or(&source_container)
    }))?;
    let target_video_codec =
        preferred_codec(&video_codec, default_video_codec_for_container(&target_container));
    let target_audio_codec =
        preferred_codec(&audio_codec, default_audio_codec_for_container(&target_container));
    let target_path = resolve_output(context, input, output, &target_container)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source_video = first_stream(&streams, "video");
    let source_audio = first_stream(&streams, "audio");
    let source_video_codec = source_video
        .as_ref()
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_lowercase();
    let source_audio_codec = source_audio
        .as_ref()
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_lowercase();
    let source_video_compatible = is_video_compatible(&target_container, &source_video_codec);
    let source_audio_compatible = is_audio_compatible(&target_container, &source_audio_codec);
    if video_codec == "copy" && !source_video_compatible {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            format!(
                "Cannot copy {} video into {}.",
                display_codec(&source_video_codec),
                target_container.to_uppercase()
            ),
        )
        .with_details(json!({
            "stream": "video",
            "codec": source_video_codec,
            "container": target_container,
        }))
        .with_suggestions(&[
            "Use --video-codec auto to let MediaForge choose copy or transcode.",
            "Choose a compatible target container.",
        ]));
    }
    if audio_codec == "copy"
        && first_stream(&streams, "audio").is_some()
        && !source_audio_compatible
    {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            format!(
                "Cannot copy {} audio into {}.",
                display_codec(&source_audio_codec),
                target_container.to_uppercase()
            ),
        )
        .with_details(json!({
            "stream": "audio",
            "codec": source_audio_codec,
            "container": target_container,
        }))
        .with_suggestions(&[
            "Use --audio-codec auto to let MediaForge choose copy or transcode.",
            "Choose a compatible target container.",
        ]));
    }
    let video_compatible =
        (video_codec == "auto" || video_codec == "copy") && source_video_compatible;
    let audio_compatible =
        (audio_codec == "auto" || audio_codec == "copy") && source_audio_compatible;
    let video_action = if video_compatible { "copy" } else { "transcode" };
    let audio_action = if audio_compatible { "copy" } else { "transcode" };
    validate_transcode_compatibility(
        "video",
        video_action,
        &target_video_codec,
        &target_container,
    )?;
    validate_transcode_compatibility(
        "audio",
        audio_action,
        &target_audio_codec,
        &target_container,
    )?;
    let hardware_selection =
        select_video_hardware(context, hardware, &target_video_codec, video_action == "transcode")?;
    let software_encoder = if video_action == "transcode" && hardware_selection.encoder.is_none() {
        Some(select_software_video_encoder(context, &target_video_codec)?)
    } else {
        None
    };
    let selected_encoder = hardware_selection.encoder.as_deref().or(software_encoder.as_deref());
    let strategy = match (video_action, audio_action) {
        ("copy", "copy") if source_container == target_container => "copy",
        ("copy", "copy") => "remux",
        ("copy", "transcode") | ("transcode", "copy") => "partial_transcode",
        _ => "transcode",
    };
    let mut reasons = Vec::new();
    if video_action == "copy" {
        reasons.push(format!(
            "{} video is compatible with {}.",
            display_codec(&source_video_codec),
            target_container.to_uppercase()
        ));
    } else if video_codec != "auto" {
        reasons.push(format!("Video codec was explicitly requested as {}.", video_codec));
    } else {
        reasons.push(format!(
            "{} video is not compatible with {}.",
            display_codec(&source_video_codec),
            target_container.to_uppercase()
        ));
    }
    if audio_action == "copy" {
        reasons.push(format!(
            "{} audio is compatible with {}.",
            display_codec(&source_audio_codec),
            target_container.to_uppercase()
        ));
    } else if audio_codec != "auto" {
        reasons.push(format!("Audio codec was explicitly requested as {}.", audio_codec));
    } else {
        reasons.push(format!(
            "{} audio is not compatible with {}.",
            display_codec(&source_audio_codec),
            target_container.to_uppercase()
        ));
    }
    let quality_loss = match (video_action, audio_action) {
        ("copy", "copy") => "none",
        ("copy", "transcode") => "audio_only",
        ("transcode", "copy") => "video_only",
        _ => "video_and_audio",
    };
    let mut ffmpeg_args = vec!["-map".to_string(), "0".to_string()];
    if video_action == "copy" {
        ffmpeg_args.extend(["-c:v".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(video_encode_args(
            &target_video_codec,
            quality_name(quality),
            selected_encoder,
        )?);
    }
    if audio_action == "copy" {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(&target_audio_codec, DEFAULT_AUDIO_BITRATE)?);
    }
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    ffmpeg_args.extend(subtitle_codec_args(&target_container, &streams));
    let plan = json!({
        "status": "success",
        "operation": "convert",
        "input": absolute_display(input),
        "output": absolute_display(&target_path),
        "strategy": strategy,
        "quality": quality,
        "video": {"action": video_action, "codec": if video_action == "copy" { source_video_codec.clone() } else { target_video_codec }, "encoder": if video_action == "transcode" { selected_encoder } else { None::<&str> }},
        "audio": {"action": audio_action, "from": source_audio_codec, "to": if audio_action == "copy" { Value::Null } else { json!(target_audio_codec) }},
        "subtitle": {"action": subtitle_strategy(&target_container, &streams)},
        "metadata": {"action": "preserve"},
        "hardware": {"requested": hardware_selection.requested, "selected": hardware_selection.selected, "encoder": hardware_selection.encoder, "reason": hardware_selection.reason},
        "quality_loss": quality_loss,
        "reason": reasons,
        "warnings": subtitle_warnings(&streams, &target_container),
        "ffmpeg_args": ffmpeg_args,
    });
    Ok(OperationPlan {
        value: plan,
        output: target_path,
        args: ffmpeg_args,
        strategy: strategy.to_string(),
    })
}

fn execute_plan(context: &Context, input: &Path, plan: &OperationPlan) -> Result<Value, AppError> {
    if plan.output == input {
        return Err(AppError::new("OUTPUT_CONFLICT", "Input and output paths must be different.")
            .with_details(
                json!({"input": absolute_display(input), "output": absolute_display(&plan.output)}),
            ));
    }
    let args = build_ffmpeg_args(input, &plan.output, &plan.args, context.overwrite);
    run_ffmpeg(context, &args)?;
    finish_plan_execution(context, input, plan)
}

fn execute_two_pass_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    if plan.output == input {
        return Err(AppError::new("OUTPUT_CONFLICT", "Input and output paths must be different.")
            .with_details(
                json!({"input": absolute_display(input), "output": absolute_display(&plan.output)}),
            ));
    }
    let passlog = temporary_passlog_path();
    let mut first_pass = plan.args.clone();
    first_pass.extend([
        "-pass".to_string(),
        "1".to_string(),
        "-passlogfile".to_string(),
        passlog.to_string_lossy().to_string(),
        "-an".to_string(),
        "-sn".to_string(),
        "-f".to_string(),
        "null".to_string(),
    ]);
    let first_args =
        build_ffmpeg_args(input, Path::new(null_device()), &first_pass, context.overwrite);
    let first_result = run_ffmpeg(context, &first_args);
    if let Err(error) = first_result {
        cleanup_passlog(&passlog);
        return Err(error);
    }

    let mut second_pass = plan.args.clone();
    second_pass.extend([
        "-pass".to_string(),
        "2".to_string(),
        "-passlogfile".to_string(),
        passlog.to_string_lossy().to_string(),
    ]);
    let second_args = build_ffmpeg_args(input, &plan.output, &second_pass, context.overwrite);
    let second_result = run_ffmpeg(context, &second_args);
    cleanup_passlog(&passlog);
    second_result?;
    finish_plan_execution(context, input, plan)
}

fn build_ffmpeg_args(
    input: &Path,
    output: &Path,
    operation_args: &[String],
    overwrite: bool,
) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        if overwrite { "-y" } else { "-n" }.to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
    ];
    args.extend(operation_args.iter().cloned());
    args.push(output.to_string_lossy().to_string());
    args
}

fn finish_plan_execution(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let verification = if context.verify_after_execute {
        verify_operation(context, input, plan)
            .map_err(|error| verification_failed(&plan.output, error))?
    } else {
        json!({
            "status": "skipped",
            "valid": Value::Null,
            "reason": "disabled_by_configuration",
            "input": absolute_display(input),
            "output": absolute_display(&plan.output),
        })
    };
    if verification.get("valid").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "VERIFY_FAILED",
            "FFmpeg completed but the output did not pass verification.",
        )
        .with_details(
            json!({"output": absolute_display(&plan.output), "verification": verification}),
        ));
    }
    let operation = plan.value.get("operation").cloned().unwrap_or_else(|| json!("convert"));
    Ok(json!({
        "status": "success",
        "operation": operation,
        "input": absolute_display(input),
        "output": absolute_display(&plan.output),
        "strategy": plan.strategy,
        "video": plan.value.get("video").cloned().unwrap_or(Value::Null),
        "audio": plan.value.get("audio").cloned().unwrap_or(Value::Null),
        "quality": plan.value.get("quality").cloned().unwrap_or(Value::Null),
        "quality_loss": plan.value.get("quality_loss").cloned().unwrap_or(Value::Null),
        "reason": plan.value.get("reason").cloned().unwrap_or_else(|| json!([])),
        "warnings": plan.value.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "hardware": plan.value.get("hardware").cloned().unwrap_or(Value::Null),
        "subtitle": plan.value.get("subtitle").cloned().unwrap_or(Value::Null),
        "metadata": plan.value.get("metadata").cloned().unwrap_or(Value::Null),
        "target_size_bytes": plan.value.get("target_size_bytes").cloned().unwrap_or(Value::Null),
        "target_dimension": plan.value.get("target_dimension").cloned().unwrap_or(Value::Null),
        "passes": plan.value.get("passes").cloned().unwrap_or_else(|| json!(1)),
        "pass_strategy": plan
            .value
            .get("pass_strategy")
            .cloned()
            .unwrap_or_else(|| json!("single_pass")),
        "verification": verification,
    }))
}

fn temporary_passlog_path() -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    std::env::temp_dir().join(format!("mediaforge-pass-{}-{nanos}", std::process::id()))
}

fn cleanup_passlog(passlog: &Path) {
    let Some(name) = passlog.file_name().and_then(OsStr::to_str) else {
        return;
    };
    let Some(parent) = passlog.parent() else {
        return;
    };
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            let matches = path
                .file_name()
                .and_then(OsStr::to_str)
                .map(|value| value.starts_with(name))
                .unwrap_or(false);
            if matches {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn compress_command(context: &Context, args: &CompressArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let quality = args.quality.unwrap_or(context.default_quality);
    let hardware = args.hardware.unwrap_or(context.default_hardware);
    let probe = probe_media(&args.input, context.verbose)?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let video = first_stream(
        &probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default(),
        "video",
    )
    .ok_or_else(|| AppError::new("INVALID_MEDIA", "No video stream was found."))?;
    let duration = probe.duration_seconds.unwrap_or(0.0);
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let hardware_selection = select_video_hardware(context, hardware, "h264", true)?;
    let software_encoder = if hardware_selection.encoder.is_none() {
        Some(select_software_video_encoder(context, "h264")?)
    } else {
        None
    };
    let selected_encoder = hardware_selection.encoder.as_deref().or(software_encoder.as_deref());
    let mut ffmpeg_args =
        vec!["-map".to_string(), "0:v:0".to_string(), "-map".to_string(), "0:a?".to_string()];
    ffmpeg_args.extend(video_encode_args("h264", quality_name(quality), selected_encoder)?);
    let mut notes = vec![format!(
        "Compressing {} video with the {:?} quality preset.",
        video.get("codec_name").and_then(Value::as_str).unwrap_or("unknown"),
        quality
    )];
    let mut two_pass = false;
    let mut target_size_bytes = None;
    match args.target_size.as_deref().map(parse_size) {
        Some(Ok(target_bytes)) => {
            target_size_bytes = Some(target_bytes);
            if duration <= 0.0 {
                return Err(AppError::new(
                    "INVALID_MEDIA",
                    "Target-size compression requires a known duration.",
                ));
            }
            let audio_bits = 256_000.0 * duration;
            let total_bits = target_bytes as f64 * 8.0 * 0.96;
            let video_bitrate = ((total_bits - audio_bits).max(250_000.0) / duration) as u64;
            ffmpeg_args.extend(["-b:v".to_string(), format!("{video_bitrate}")]);
            ffmpeg_args.extend(["-maxrate".to_string(), format!("{video_bitrate}")]);
            ffmpeg_args
                .extend(["-bufsize".to_string(), format!("{}", video_bitrate.saturating_mul(2))]);
            if software_encoder.is_some() {
                remove_option(&mut ffmpeg_args, "-crf");
            }
            two_pass = hardware_selection.encoder.is_none();
            if two_pass {
                notes.push(
                    "Using two-pass software encoding to improve target-size accuracy.".to_string(),
                );
            }
            notes.push(format!("Target size is approximately {} bytes.", target_bytes));
        }
        Some(Err(error)) => return Err(error),
        None => {
            if hardware_selection.encoder.is_some() {
                ffmpeg_args
                    .extend(["-b:v".to_string(), hardware_quality_bitrate(quality).to_string()]);
            }
        }
    }
    ffmpeg_args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        DEFAULT_AUDIO_BITRATE.to_string(),
    ]);
    ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"compress","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"transcode","quality":quality,"target_size_bytes":target_size_bytes,"passes":if two_pass { 2 } else { 1 },"pass_strategy":if two_pass { "two_pass" } else { "single_pass" },"quality_loss":"video_and_audio","reason":notes,"hardware":{"requested":hardware_selection.requested,"selected":hardware_selection.selected,"encoder":hardware_selection.encoder,"reason":hardware_selection.reason},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "transcode".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    if two_pass {
        execute_two_pass_plan(context, &args.input, &plan)
    } else {
        execute_plan(context, &args.input, &plan)
    }
}

fn resize_command(context: &Context, args: &ResizeArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    if args.width.is_none() && args.resolution.is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", "Provide --width or --resolution."));
    }
    if args.width.is_some() && args.resolution.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Use only one of --width or --resolution."));
    }
    if args.width == Some(0) {
        return Err(AppError::new("INVALID_ARGUMENT", "Resize width must be greater than zero."));
    }
    let height = args.resolution.as_deref().map(parse_resolution).transpose()?;
    let (target_axis, requested_dimension) = if let Some(width) = args.width {
        ("width", width)
    } else {
        (
            "height",
            height.ok_or_else(|| {
                AppError::new("INVALID_ARGUMENT", "Provide --width or --resolution.")
            })?,
        )
    };
    let effective_dimension = even_dimension(requested_dimension)?;
    let filter = if target_axis == "width" {
        format!("scale={effective_dimension}:-2")
    } else {
        format!("scale=-2:{effective_dimension}")
    };
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let software_encoder = select_software_video_encoder(context, "h264")?;
    let mut warnings = subtitle_warnings(&streams, "mp4");
    if requested_dimension != effective_dimension {
        warnings.push(format!(
            "Requested {target_axis} {requested_dimension} was rounded to {effective_dimension} for an even encoder-compatible dimension."
        ));
    }
    let mut ffmpeg_args = vec![
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
        "-vf".to_string(),
        filter.clone(),
    ];
    ffmpeg_args.extend(video_encode_args("h264", "high", Some(&software_encoder))?);
    ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"resize","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"video_transcode","filter":filter,"target_dimension":{"axis":target_axis,"requested":requested_dimension,"effective":effective_dimension},"preserve_aspect_ratio":true,"even_dimensions":true,"quality_loss":"video_only","hardware":{"requested":"cpu","selected":"cpu","encoder":null,"reason":"Resize uses deterministic software filtering."},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":warnings,"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "video_transcode".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_plan(context, &args.input, &plan)
}

fn clip_command(context: &Context, args: &ClipArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    if args.duration.is_none() && args.end.is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", "Provide --duration or --end."));
    }
    if args.duration.is_some() && args.end.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Use only one of --duration or --end."));
    }
    let start_seconds = parse_time_seconds(&args.start)?;
    if start_seconds < 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "Clip start must not be negative."));
    }
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Clip duration must be greater than zero.",
            ));
        }
    }
    if let Some(end) = &args.end {
        if parse_time_seconds(end)? <= start_seconds {
            return Err(AppError::new("INVALID_ARGUMENT", "Clip end must be after start."));
        }
    }
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source_container = probe
        .raw
        .get("format")
        .and_then(|format| format.get("format_name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown");
    let source_container = internal_container(&args.input, source_container);
    let source_video_codec = first_stream(&streams, "video")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let source_audio_codec = first_stream(&streams, "audio")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let copy_compatible = start_seconds == 0.0
        && source_container == "mp4"
        && is_video_compatible("mp4", &source_video_codec)
        && (source_audio_codec == "unknown" || is_audio_compatible("mp4", &source_audio_codec));
    let software_encoder =
        if copy_compatible { None } else { Some(select_software_video_encoder(context, "h264")?) };
    let mut ffmpeg_args = Vec::new();
    if !copy_compatible {
        ffmpeg_args.extend([
            "-ss".to_string(),
            args.start.clone(),
            "-i".to_string(),
            args.input.to_string_lossy().to_string(),
            "-map".to_string(),
            "0:v:0".to_string(),
            "-map".to_string(),
            "0:a?".to_string(),
        ]);
    } else {
        ffmpeg_args.extend(["-map".to_string(), "0".to_string()]);
    }
    if let Some(duration) = &args.duration {
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    } else if let Some(end) = &args.end {
        let duration = parse_time_seconds(end)? - start_seconds;
        ffmpeg_args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    if copy_compatible {
        ffmpeg_args.extend(["-c".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(video_encode_args("h264", "high", software_encoder.as_deref())?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    }
    if copy_compatible {
        ffmpeg_args.extend(subtitle_codec_args("mp4", &streams));
    } else {
        ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    }
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let strategy = if copy_compatible { "stream_copy" } else { "precise_transcode" };
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"clip","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":strategy,"start":args.start,"duration":args.duration,"end":args.end,"quality_loss":if copy_compatible { "none" } else { "video_and_audio" },"reason":if copy_compatible { "Start is at zero and source streams are compatible with MP4; stream copy avoids re-encoding." } else { "Precise clipping re-encodes to honor the requested boundary." },"hardware":{"requested":"cpu","selected":if copy_compatible { "not_applicable" } else { "cpu" },"encoder":null,"reason":if copy_compatible { "Stream copy avoids video encoding." } else { "Precise clipping uses deterministic software encoding." }},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"metadata":{"action":"preserve"},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: strategy.to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}

fn extract_audio_command(context: &Context, args: &ExtractAudioArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let format = normalize_audio_format(&args.format)?;
    let audio_extension = audio_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &audio_extension)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "audio").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "No audio stream was found."));
    }
    let source_audio_codec = first_stream(&streams, "audio")
        .and_then(|stream| stream.get("codec_name").and_then(Value::as_str).map(str::to_lowercase))
        .unwrap_or_else(|| "unknown".to_string());
    let target_audio_codec = audio_codec_for_format(&format);
    let copy_audio = audio_copy_compatible(&source_audio_codec, &format);
    let mut ffmpeg_args = vec!["-map".to_string(), "0:a:0".to_string(), "-vn".to_string()];
    if copy_audio {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(target_audio_codec, DEFAULT_AUDIO_BITRATE)?);
    }
    ffmpeg_args.extend(audio_container_args(&format));
    ffmpeg_args.extend(["-map_metadata".to_string(), "0".to_string()]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"extract_audio","input":absolute_display(&args.input),"output":absolute_display(&output),"format":format,"source_codec":source_audio_codec,"target_codec":target_audio_codec,"strategy":if copy_audio { "copy" } else { "transcode" },"quality_loss":if copy_audio { "none" } else { "audio_only" },"hardware":{"requested":"cpu","selected":"not_applicable","encoder":null,"reason":"Audio extraction does not use video hardware."},"metadata":{"action":"preserve"},"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: if copy_audio { "copy".to_string() } else { "transcode".to_string() },
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}

fn thumbnail_command(context: &Context, args: &ThumbnailArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let at = parse_thumbnail_time(&args.at, probe.duration_seconds)?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "jpg")?;
    let ffmpeg_args = vec![
        "-ss".to_string(),
        at.clone(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-q:v".to_string(),
        "2".to_string(),
    ];
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"thumbnail","input":absolute_display(&args.input),"output":absolute_display(&output),"at":at,"format":"jpg","hardware":{"requested":"cpu","selected":"not_applicable","encoder":null,"reason":"Thumbnail extraction uses the software image pipeline."},"metadata":{"action":"not_applicable"},"ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "frame_extract".to_string(),
    };
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, &args.input, &plan)
}

fn finish_custom_plan(
    context: &Context,
    input: &Path,
    plan: OperationPlan,
) -> Result<Value, AppError> {
    if context.dry_run {
        let mut value = plan.value;
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    execute_simple_plan(context, input, &plan)
}

fn normalize_image_format(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    let normalized = match value.as_str() {
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "webp" => "webp",
        "gif" => "gif",
        "bmp" => "bmp",
        "tif" | "tiff" => "tiff",
        "ico" => "ico",
        "tga" => "tga",
        "avif" => "avif",
        _ => {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                format!("Unsupported image format: {value}"),
            ))
        }
    };
    Ok(normalized.to_string())
}

fn image_output_extension(format: &str) -> String {
    match format {
        "jpg" => "jpg".to_string(),
        "tiff" => "tiff".to_string(),
        other => other.to_string(),
    }
}

fn validate_positive_dimension(value: Option<u32>, field: &str) -> Result<(), AppError> {
    if value == Some(0) {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("{field} must be greater than zero."),
        ));
    }
    Ok(())
}

fn rotate_filter(value: u16) -> Result<&'static str, AppError> {
    match value {
        90 => Ok("transpose=1"),
        180 => Ok("hflip,vflip"),
        270 => Ok("transpose=2"),
        _ => Err(AppError::new("INVALID_ARGUMENT", "Rotation must be 90, 180, or 270 degrees.")),
    }
}

fn image_quality_args(format: &str, quality: u8) -> Vec<String> {
    match format {
        "jpg" => {
            let quantizer = 31_u16.saturating_sub((quality as u16 * 30) / 100).max(1);
            vec!["-q:v".to_string(), quantizer.to_string()]
        }
        "webp" | "avif" => vec!["-q:v".to_string(), quality.to_string()],
        "png" => {
            let compression = ((100_u16.saturating_sub(quality as u16)) / 12).min(9);
            vec!["-compression_level".to_string(), compression.to_string()]
        }
        _ => Vec::new(),
    }
}

fn image_codec_args(format: &str) -> Vec<String> {
    let codec = match format {
        "jpg" => "mjpeg",
        "png" => "png",
        "webp" => "libwebp",
        "gif" => "gif",
        "bmp" => "bmp",
        "tiff" => "tiff",
        "ico" => "bmp",
        "tga" => "targa",
        "avif" => "libaom-av1",
        _ => return Vec::new(),
    };
    vec!["-c:v".to_string(), codec.to_string()]
}

fn image_command(context: &Context, args: &ImageArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    validate_positive_dimension(args.width, "Image width")?;
    validate_positive_dimension(args.height, "Image height")?;
    if let Some(quality) = args.image_quality {
        if !(1..=100).contains(&quality) {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Image quality must be between 1 and 100.",
            ));
        }
    }
    if let Some(watermark) = &args.watermark {
        ensure_input(watermark)?;
    }
    let format = normalize_image_format(
        args.to
            .as_deref()
            .or_else(|| {
                args.output.as_deref().and_then(|path| path.extension().and_then(OsStr::to_str))
            })
            .unwrap_or_else(|| args.input.extension().and_then(OsStr::to_str).unwrap_or("png")),
    )?;
    let image_extension = image_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &image_extension)?;
    let mut filters = Vec::new();
    match (args.width, args.height) {
        (Some(width), Some(height)) => filters.push(format!("scale={width}:{height}")),
        (Some(width), None) => filters.push(format!("scale={width}:-1")),
        (None, Some(height)) => filters.push(format!("scale=-1:{height}")),
        (None, None) => {}
    }
    if let Some(rotate) = args.rotate {
        filters.push(rotate_filter(rotate)?.to_string());
    }
    let mut ffmpeg_args = vec!["-i".to_string(), args.input.to_string_lossy().to_string()];
    if let Some(watermark) = &args.watermark {
        ffmpeg_args.extend(["-i".to_string(), watermark.to_string_lossy().to_string()]);
        let base = if filters.is_empty() {
            "[0:v]".to_string()
        } else {
            format!("[0:v]{}[base]", filters.join(","))
        };
        let overlay_input = if filters.is_empty() { "[0:v]" } else { "[base]" };
        let filter_complex = if filters.is_empty() {
            "[0:v][1:v]overlay=W-w-16:H-h-16[v]".to_string()
        } else {
            format!("{base};{overlay_input}[1:v]overlay=W-w-16:H-h-16[v]")
        };
        ffmpeg_args.extend([
            "-filter_complex".to_string(),
            filter_complex,
            "-map".to_string(),
            "[v]".to_string(),
        ]);
    } else if !filters.is_empty() {
        ffmpeg_args.extend(["-vf".to_string(), filters.join(",")]);
    }
    ffmpeg_args.push("-frames:v".to_string());
    ffmpeg_args.push("1".to_string());
    ffmpeg_args.extend(image_codec_args(&format));
    ffmpeg_args.extend(image_quality_args(&format, args.image_quality.unwrap_or(90)));
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "image",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "resize": {"width": args.width, "height": args.height},
            "rotate": args.rotate,
            "watermark": args.watermark.as_ref().map(|path| absolute_display(path)),
            "quality": args.image_quality.unwrap_or(90),
            "strategy": "image_transcode",
            "quality_loss": if args.image_quality.is_some() { "possible" } else { "none" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "image_transcode".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}

fn gif_command(context: &Context, args: &GifArgs) -> Result<Value, AppError> {
    const MAX_GIF_DURATION_SECONDS: f64 = 600.0;
    const MAX_GIF_WIDTH: u32 = 16_384;
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "video").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "GIF conversion requires a video stream."));
    }
    if !(1..=60).contains(&args.fps) {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF FPS must be between 1 and 60."));
    }
    let start_seconds = parse_time_seconds(&args.start)?;
    if !start_seconds.is_finite() {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF start must be a finite timestamp."));
    }
    if start_seconds < 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF start must not be negative."));
    }
    let duration_seconds = parse_time_seconds(&args.duration)?;
    if !duration_seconds.is_finite() {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF duration must be a finite value."));
    }
    if duration_seconds <= 0.0 {
        return Err(AppError::new("INVALID_ARGUMENT", "GIF duration must be greater than zero."));
    }
    if duration_seconds > MAX_GIF_DURATION_SECONDS {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("GIF duration must not exceed {MAX_GIF_DURATION_SECONDS:.0} seconds."),
        )
        .with_suggestions(&["Use a shorter clip or split a long animation into multiple GIFs."]));
    }
    if args.width.is_some_and(|width| width > MAX_GIF_WIDTH) {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("GIF width must not exceed {MAX_GIF_WIDTH} pixels."),
        ));
    }
    validate_positive_dimension(args.width, "GIF width")?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "gif")?;
    let mut filters = vec![format!("fps={}", args.fps)];
    if let Some(width) = args.width {
        filters.push(format!("scale={width}:-1:flags=lanczos"));
    }
    let filter = format!(
        "{},split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=sierra2_4a",
        filters.join(",")
    );
    let ffmpeg_args = vec![
        "-ss".to_string(),
        args.start.clone(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-t".to_string(),
        args.duration.clone(),
        "-an".to_string(),
        "-vf".to_string(),
        filter.clone(),
        "-loop".to_string(),
        "0".to_string(),
        "-f".to_string(),
        "gif".to_string(),
    ];
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "gif",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "start": args.start,
            "duration": args.duration,
            "fps": args.fps,
            "width": args.width,
            "strategy": "palette_gif",
            "quality_loss": "video_only",
            "filter": filter,
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "palette_gif".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}

fn parse_crop(value: &str) -> Result<String, AppError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 4 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Crop must use WIDTH:HEIGHT:X:Y."));
    }
    for part in &parts {
        if part.parse::<u32>().is_err() {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Crop dimensions and offsets must be non-negative integers.",
            ));
        }
    }
    Ok(format!("crop={}", parts.join(":")))
}

fn named_video_filter(value: &str) -> Result<&'static str, AppError> {
    match value.to_lowercase().as_str() {
        "grayscale" | "gray" => Ok("hue=s=0"),
        "blur" => Ok("boxblur=2:1"),
        "sharpen" => Ok("unsharp=5:5:1.0:5:5:0.0"),
        "vintage" => Ok("curves=vintage"),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported named filter: {other}")))
        }
    }
}

fn atempo_filter(speed: f64) -> String {
    let mut value = speed;
    let mut filters = Vec::new();
    while value < 0.5 {
        filters.push("atempo=0.5".to_string());
        value /= 0.5;
    }
    while value > 2.0 {
        filters.push("atempo=2.0".to_string());
        value /= 2.0;
    }
    filters.push(format!("atempo={value:.6}"));
    filters.join(",")
}

fn escape_filter_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace(':', "\\:").replace('\'', "\\'")
}

fn subtitle_filter(path: &Path, style: Option<&str>) -> Result<String, AppError> {
    let mut filter = format!("subtitles={}", escape_filter_path(path));
    if let Some(style) = style {
        let style = style.trim();
        if style.is_empty() || !style.contains('=') {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Subtitle style must contain comma-separated key=value pairs.",
            ));
        }
        if style.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || " =,.:_&#%+-/".contains(character))
        }) {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Subtitle style contains unsupported filter characters.",
            )
            .with_suggestions(&[
                "Use values such as FontName=Arial,FontSize=24,PrimaryColour=&H00FFFFFF.",
            ]));
        }
        filter.push_str(":force_style='");
        filter.push_str(style);
        filter.push('\'');
    }
    Ok(filter)
}

fn ffmpeg_filter_available(context: &Context, name: &str) -> bool {
    run_program("ffmpeg", &["-hide_banner", "-filters"], context.verbose).ok().is_some_and(
        |result| {
            result.stdout.lines().any(|line| line.split_whitespace().any(|token| token == name))
        },
    )
}

fn edit_command(context: &Context, args: &EditArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if first_stream(&streams, "video").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "Edit requires a video stream."));
    }
    if let Some(speed) = args.speed {
        if !(0.25..=4.0).contains(&speed) {
            return Err(AppError::new("INVALID_ARGUMENT", "Speed must be between 0.25 and 4.0."));
        }
    }
    if let Some(volume) = args.volume {
        if !(0.0..=10.0).contains(&volume) {
            return Err(AppError::new("INVALID_ARGUMENT", "Volume must be between 0 and 10."));
        }
    }
    if let Some(subtitle) = &args.subtitle {
        ensure_input(subtitle)?;
    }
    if args.subtitle.is_none() && args.subtitle_style.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Subtitle style requires --subtitle."));
    }
    let subtitle_filter_available =
        args.subtitle.as_ref().is_none_or(|_| ffmpeg_filter_available(context, "subtitles"));
    if args.subtitle.is_some() && !subtitle_filter_available && !context.dry_run {
        return Err(AppError::new(
            "FILTER_UNAVAILABLE",
            "The installed FFmpeg build does not include the subtitles/libass filter.",
        )
        .with_details(json!({"filter":"subtitles","subtitle":args.subtitle.as_ref().map(|path| absolute_display(path))}))
        .with_suggestions(&[
            "Install an FFmpeg build compiled with libass, then retry.",
            "Use a subtitle stream conversion operation when burn-in is not required.",
        ]));
    }
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let mut video_filters = Vec::new();
    if let Some(crop) = &args.crop {
        video_filters.push(parse_crop(crop)?);
    }
    if let Some(rotate) = args.rotate {
        video_filters.push(rotate_filter(rotate)?.to_string());
    }
    if let Some(filter) = &args.filter {
        video_filters.push(named_video_filter(filter)?.to_string());
    }
    if let Some(speed) = args.speed {
        video_filters.push(format!("setpts=PTS/{speed:.6}"));
    }
    if let Some(subtitle) = &args.subtitle {
        video_filters.push(subtitle_filter(subtitle, args.subtitle_style.as_deref())?);
    }
    let mut ffmpeg_args = Vec::new();
    if let Some(start) = &args.start {
        parse_time_seconds(start)?;
        ffmpeg_args.extend(["-ss".to_string(), start.clone()]);
    }
    ffmpeg_args.extend(["-i".to_string(), args.input.to_string_lossy().to_string()]);
    ffmpeg_args.extend([
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
    ]);
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Edit duration must be greater than zero.",
            ));
        }
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    }
    if !video_filters.is_empty() {
        ffmpeg_args.extend(["-vf".to_string(), video_filters.join(",")]);
    }
    ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
    if args.speed.is_some() || args.volume.is_some() {
        let mut audio_filters = Vec::new();
        if let Some(speed) = args.speed {
            audio_filters.push(atempo_filter(speed));
        }
        if let Some(volume) = args.volume {
            audio_filters.push(format!("volume={volume:.6}"));
        }
        ffmpeg_args.extend(["-af".to_string(), audio_filters.join(",")]);
    }
    ffmpeg_args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        DEFAULT_AUDIO_BITRATE.to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "edit",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "strategy": "filter_transcode",
            "crop": args.crop,
            "rotate": args.rotate,
            "speed": args.speed,
            "volume": args.volume,
            "filter": args.filter,
            "subtitle": args.subtitle.as_ref().map(|path| absolute_display(path)),
            "subtitle_style": args.subtitle_style,
            "warnings": if args.subtitle.is_some() && !subtitle_filter_available {
                vec!["The current FFmpeg build lacks the subtitles/libass filter; execution is unavailable.".to_string()]
            } else {
                Vec::new()
            },
            "audio_present": first_stream(&streams, "audio").is_some(),
            "quality_loss": "video_and_audio",
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: "filter_transcode".to_string(),
    };
    finish_custom_plan(context, &args.input, plan)
}

fn merge_command(context: &Context, args: &MergeArgs) -> Result<Value, AppError> {
    if args.inputs.len() < 2 {
        return Err(AppError::new("INVALID_ARGUMENT", "Merge requires at least two input files."));
    }
    let mode = args.mode.to_lowercase();
    if !["concat", "mux", "mix"].contains(&mode.as_str()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Merge mode must be concat, mux, or mix."));
    }
    for input in &args.inputs {
        ensure_input(input)?;
    }
    if mode != "concat" && args.inputs.len() != 2 {
        return Err(AppError::new("INVALID_ARGUMENT", "Mux and mix require exactly two inputs."));
    }
    let probes = args
        .inputs
        .iter()
        .map(|input| probe_media(input, context.verbose))
        .collect::<Result<Vec<_>, _>>()?;
    let streams = probes
        .iter()
        .map(|probe| {
            probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let has_video = streams.iter().any(|value| first_stream(value, "video").is_some());
    let has_audio = streams.iter().any(|value| first_stream(value, "audio").is_some());
    if mode == "mux"
        && (first_stream(&streams[0], "video").is_none()
            || first_stream(&streams[1], "audio").is_none())
    {
        return Err(AppError::new(
            "INVALID_MEDIA",
            "Mux expects a video input followed by an audio input.",
        ));
    }
    if mode == "mix" && streams.iter().any(|value| first_stream(value, "audio").is_none()) {
        return Err(AppError::new("INVALID_MEDIA", "Mix expects audio streams in both inputs."));
    }
    let output_extension = if args.output.is_some() {
        args.output
            .as_deref()
            .and_then(|path| path.extension().and_then(OsStr::to_str))
            .unwrap_or("mp4")
            .to_string()
    } else if mode == "mix" && !has_video {
        "m4a".to_string()
    } else {
        "mp4".to_string()
    };
    let output =
        resolve_output(context, &args.inputs[0], args.output.as_deref(), &output_extension)?;
    let mut ffmpeg_args = Vec::new();
    for input in &args.inputs {
        ffmpeg_args.extend(["-i".to_string(), input.to_string_lossy().to_string()]);
    }
    match mode.as_str() {
        "concat" => {
            let all_video = streams.iter().all(|value| first_stream(value, "video").is_some());
            let all_audio = streams.iter().all(|value| first_stream(value, "audio").is_some());
            let mut filter = String::new();
            for index in 0..args.inputs.len() {
                if all_video {
                    filter.push_str(&format!("[{index}:v:0]"));
                }
                if all_audio {
                    filter.push_str(&format!("[{index}:a:0]"));
                }
            }
            filter.push_str(&format!(
                "concat=n={}:v={}:a={}",
                args.inputs.len(),
                all_video as u8,
                all_audio as u8
            ));
            if all_video {
                filter.push_str("[v]");
            }
            if all_audio {
                filter.push_str("[a]");
            }
            ffmpeg_args.extend(["-filter_complex".to_string(), filter]);
            if all_video {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "[v]".to_string(),
                    "-c:v".to_string(),
                    "libx264".to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                    "-crf".to_string(),
                    "23".to_string(),
                ]);
            }
            if all_audio {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "[a]".to_string(),
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    DEFAULT_AUDIO_BITRATE.to_string(),
                ]);
            }
        }
        "mux" => ffmpeg_args.extend([
            "-map".to_string(),
            "0:v:0".to_string(),
            "-map".to_string(),
            "1:a:0".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
        ]),
        "mix" => {
            ffmpeg_args.extend([
                "-filter_complex".to_string(),
                "[0:a:0][1:a:0]amix=inputs=2:duration=longest[a]".to_string(),
            ]);
            if has_video {
                ffmpeg_args.extend([
                    "-map".to_string(),
                    "0:v:0".to_string(),
                    "-c:v".to_string(),
                    "copy".to_string(),
                ]);
            }
            ffmpeg_args.extend([
                "-map".to_string(),
                "[a]".to_string(),
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                DEFAULT_AUDIO_BITRATE.to_string(),
            ]);
        }
        _ => unreachable!(),
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "merge",
            "mode": mode,
            "inputs": args.inputs.iter().map(|path| absolute_display(path)).collect::<Vec<_>>(),
            "input_count": args.inputs.len(),
            "output": absolute_display(&output),
            "strategy": mode,
            "quality_loss": if args.mode.eq_ignore_ascii_case("mux") { "none" } else { "possible" },
            "video_present": has_video,
            "audio_present": if mode == "concat" {
                streams.iter().all(|value| first_stream(value, "audio").is_some())
            } else {
                has_audio
            },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: mode.to_string(),
    };
    finish_custom_plan(context, &args.inputs[0], plan)
}

fn validate_bitrate(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("INVALID_ARGUMENT", "Audio bitrate cannot be empty."));
    }
    let (number, suffix) = trimmed
        .strip_suffix('k')
        .map(|value| (value, "k"))
        .or_else(|| trimmed.strip_suffix('K').map(|value| (value, "k")))
        .or_else(|| trimmed.strip_suffix('M').map(|value| (value, "M")))
        .or_else(|| trimmed.strip_suffix('m').map(|value| (value, "M")))
        .unwrap_or((trimmed, ""));
    if number.parse::<f64>().ok().filter(|number| number.is_finite() && *number > 0.0).is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", format!("Invalid audio bitrate: {value}")));
    }
    Ok(format!("{number}{suffix}"))
}

fn audio_command(context: &Context, args: &AudioArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let format = normalize_audio_format(&args.format)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let source = first_stream(&streams, "audio")
        .ok_or_else(|| AppError::new("INVALID_MEDIA", "No audio stream was found."))?;
    let source_codec =
        source.get("codec_name").and_then(Value::as_str).unwrap_or("unknown").to_lowercase();
    if let Some(rate) = args.sample_rate {
        if rate == 0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Sample rate must be greater than zero.",
            ));
        }
    }
    if let Some(channels) = args.channels {
        if channels == 0 {
            return Err(AppError::new("INVALID_ARGUMENT", "Channels must be greater than zero."));
        }
    }
    if let Some(volume) = args.volume {
        if !(0.0..=10.0).contains(&volume) {
            return Err(AppError::new("INVALID_ARGUMENT", "Volume must be between 0 and 10."));
        }
    }
    let bitrate = args
        .bitrate
        .as_deref()
        .map(validate_bitrate)
        .transpose()?
        .unwrap_or_else(|| DEFAULT_AUDIO_BITRATE.to_string());
    let audio_extension = audio_output_extension(&format);
    let output = resolve_output(context, &args.input, args.output.as_deref(), &audio_extension)?;
    let target_codec = audio_codec_for_format(&format);
    let copy_audio = args.bitrate.is_none()
        && args.sample_rate.is_none()
        && args.channels.is_none()
        && args.volume.is_none()
        && args.start.is_none()
        && args.duration.is_none()
        && audio_copy_compatible(&source_codec, &format);
    let mut ffmpeg_args = vec![
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:a:0".to_string(),
        "-vn".to_string(),
    ];
    if let Some(start) = &args.start {
        parse_time_seconds(start)?;
        ffmpeg_args.extend(["-ss".to_string(), start.clone()]);
    }
    if let Some(duration) = &args.duration {
        if parse_time_seconds(duration)? <= 0.0 {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                "Audio duration must be greater than zero.",
            ));
        }
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    }
    if copy_audio {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(target_codec, &bitrate)?);
    }
    if let Some(rate) = args.sample_rate {
        ffmpeg_args.extend(["-ar".to_string(), rate.to_string()]);
    }
    if let Some(channels) = args.channels {
        ffmpeg_args.extend(["-ac".to_string(), channels.to_string()]);
    }
    if let Some(volume) = args.volume {
        ffmpeg_args.extend(["-af".to_string(), format!("volume={volume:.6}")]);
    }
    ffmpeg_args.extend(audio_container_args(&format));
    ffmpeg_args.push("-map_metadata".to_string());
    ffmpeg_args.push("0".to_string());
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "audio",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "source_codec": source_codec,
            "target_codec": target_codec,
            "strategy": if copy_audio { "copy" } else { "transcode" },
            "bitrate": bitrate,
            "sample_rate": args.sample_rate,
            "channels": args.channels,
            "volume": args.volume,
            "quality_loss": if copy_audio { "none" } else { "audio_only" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: if copy_audio { "copy".to_string() } else { "transcode".to_string() },
    };
    finish_custom_plan(context, &args.input, plan)
}

fn repair_command(context: &Context, args: &RepairArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    let probe = probe_media(&args.input, context.verbose)?;
    let source_streams =
        probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let extension = args.input.extension().and_then(OsStr::to_str).unwrap_or("mp4");
    let output = if let Some(path) = &args.output {
        resolve_output(context, &args.input, Some(path), extension)?
    } else {
        let stem = args.input.file_stem().and_then(OsStr::to_str).unwrap_or("media");
        let requested = args.input.with_file_name(format!("{stem}_repaired.{extension}"));
        resolve_output(context, &args.input, Some(&requested), extension)?
    };
    let mut ffmpeg_args = vec![
        "-fflags".to_string(),
        "+genpts+discardcorrupt".to_string(),
        "-err_detect".to_string(),
        "ignore_err".to_string(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
    ];
    if args.reencode {
        ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    } else {
        ffmpeg_args.extend(["-c".to_string(), "copy".to_string()]);
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "repair",
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "strategy": if args.reencode { "repair_transcode" } else { "repair_copy" },
            "reencode": args.reencode,
            "audio_present": first_stream(&source_streams, "audio").is_some(),
            "quality_loss": if args.reencode { "possible" } else { "none" },
            "ffmpeg_args": ffmpeg_args,
        }),
        output,
        args: ffmpeg_args,
        strategy: if args.reencode {
            "repair_transcode".to_string()
        } else {
            "repair_copy".to_string()
        },
    };
    finish_custom_plan(context, &args.input, plan)
}

fn disc_command(context: &Context, args: &DiscArgs) -> Result<Value, AppError> {
    if !args.input.exists() {
        return Err(AppError::new(
            "FILE_NOT_FOUND",
            format!("Disc source does not exist: {}", args.input.display()),
        ));
    }
    let kind = args.kind.to_lowercase();
    if !["dvd", "cd", "iso"].contains(&kind.as_str()) {
        return Err(AppError::new("INVALID_ARGUMENT", "Disc kind must be dvd, cd, or iso."));
    }
    let action = normalize_disc_action(&args.action)?;
    if action == "create_iso" {
        return create_iso_command(context, args, &kind);
    }
    let default_format = if kind == "cd" { "flac" } else { "mp4" };
    let target = args.to.clone().unwrap_or_else(|| default_format.to_string());
    let (format, extension) = if kind == "cd" {
        let format = normalize_audio_format(&target)?;
        let extension = audio_output_extension(&format).to_string();
        (format, extension)
    } else {
        let format = normalize_container(&target)?;
        (format.clone(), format)
    };
    let output = resolve_output(context, &args.input, args.output.as_deref(), &extension)?;
    let mut ffmpeg_args = vec!["-i".to_string(), args.input.to_string_lossy().to_string()];
    if kind == "cd" {
        ffmpeg_args.extend(["-map".to_string(), "0:a?".to_string(), "-vn".to_string()]);
        ffmpeg_args
            .extend(audio_encode_args(audio_codec_for_format(&format), DEFAULT_AUDIO_BITRATE)?);
        ffmpeg_args.extend(audio_container_args(&format));
    } else if kind == "dvd" {
        ffmpeg_args.extend(["-map".to_string(), "0".to_string()]);
        ffmpeg_args.extend(video_encode_args("h264", "high", Some("libx264"))?);
        ffmpeg_args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            DEFAULT_AUDIO_BITRATE.to_string(),
        ]);
    } else {
        ffmpeg_args.extend([
            "-map".to_string(),
            "0".to_string(),
            "-c".to_string(),
            "copy".to_string(),
        ]);
    }
    let plan = OperationPlan {
        value: json!({
            "status": "success",
            "operation": "disc",
            "kind": kind,
            "action": action,
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
            "format": format,
            "strategy": if kind == "iso" { "disc_copy" } else { "disc_transcode" },
            "ffmpeg_args": ffmpeg_args,
            "warnings": ["Disc devices and protected media depend on platform permissions and FFmpeg build support."],
        }),
        output,
        args: ffmpeg_args,
        strategy: if kind == "iso" {
            "disc_copy".to_string()
        } else {
            "disc_transcode".to_string()
        },
    };
    finish_custom_plan(context, &args.input, plan)
}

fn normalize_disc_action(value: &str) -> Result<String, AppError> {
    match value.to_lowercase().replace('-', "_").as_str() {
        "extract" | "convert" | "remux" => Ok("extract".to_string()),
        "create_iso" | "author" | "write_iso" => Ok("create_iso".to_string()),
        other => {
            Err(AppError::new("INVALID_ARGUMENT", format!("Unsupported disc action: {other}"))
                .with_suggestions(&["Use --action extract or --action create-iso."]))
        }
    }
}

fn disc_authoring_tool() -> Option<&'static str> {
    ["xorriso", "genisoimage", "mkisofs", "hdiutil"]
        .into_iter()
        .find(|tool| program_available(tool))
}

fn create_iso_command(context: &Context, args: &DiscArgs, kind: &str) -> Result<Value, AppError> {
    if !args.input.is_dir() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "ISO creation requires a directory containing the disc files.",
        )
        .with_suggestions(&[
            "Mount or stage the DVD/CD contents in a directory, then pass that directory as input.",
            "Use --action extract when the input is an existing ISO or media file.",
        ]));
    }
    let output = resolve_output(context, &args.input, args.output.as_deref(), "iso")?;
    let label =
        args.volume_label.clone().unwrap_or_else(|| format!("MEDIAFORGE-{}", kind.to_uppercase()));
    if label.is_empty() || label.len() > 32 || label.chars().any(|character| character.is_control())
    {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            "ISO volume label must be 1-32 printable characters.",
        ));
    }
    let tool = disc_authoring_tool();
    let (program, tool_args) = match tool {
        Some("xorriso") => (
            "xorriso".to_string(),
            vec![
                "-as".to_string(),
                "mkisofs".to_string(),
                "-quiet".to_string(),
                "-J".to_string(),
                "-R".to_string(),
                "-V".to_string(),
                label.clone(),
                "-o".to_string(),
                output.to_string_lossy().to_string(),
                args.input.to_string_lossy().to_string(),
            ],
        ),
        Some("genisoimage") | Some("mkisofs") => {
            let program = tool.unwrap().to_string();
            (
                program,
                vec![
                    "-quiet".to_string(),
                    "-J".to_string(),
                    "-R".to_string(),
                    "-V".to_string(),
                    label.clone(),
                    "-o".to_string(),
                    output.to_string_lossy().to_string(),
                    args.input.to_string_lossy().to_string(),
                ],
            )
        }
        Some("hdiutil") => (
            "hdiutil".to_string(),
            vec![
                "makehybrid".to_string(),
                "-ov".to_string(),
                "-o".to_string(),
                output.to_string_lossy().to_string(),
                "-hfs".to_string(),
                "-joliet".to_string(),
                "-iso".to_string(),
                "-default-volume-name".to_string(),
                label.clone(),
                args.input.to_string_lossy().to_string(),
            ],
        ),
        _ => (String::new(), Vec::new()),
    };
    let mut value = json!({
        "status": "success",
        "operation": "disc",
        "action": "create_iso",
        "kind": kind,
        "input": absolute_display(&args.input),
        "output": absolute_display(&output),
        "format": "iso",
        "strategy": "disc_authoring",
        "tool": if program.is_empty() { Value::Null } else { json!(program) },
        "tool_available": !program.is_empty(),
        "tool_args": tool_args,
        "volume_label": label,
        "warnings": [
            "ISO creation depends on an installed authoring utility; FFmpeg alone does not author ISO images.",
            "This operation creates a filesystem image and does not bypass optical-media DRM."
        ],
    });
    if context.dry_run {
        value["status"] = json!("planned");
        value["will_execute"] = json!(false);
        return Ok(value);
    }
    if program.is_empty() {
        return Err(AppError::new(
            "DISC_TOOL_UNAVAILABLE",
            "No ISO authoring utility was found on PATH.",
        )
        .with_details(json!({
            "candidates": ["xorriso", "genisoimage", "mkisofs", "hdiutil"],
            "input": absolute_display(&args.input),
            "output": absolute_display(&output),
        }))
        .with_suggestions(&[
            "Install xorriso, genisoimage, or mkisofs, then retry.",
            "On macOS, hdiutil is normally available when the system permits it.",
        ]));
    }
    let refs = tool_args.iter().map(String::as_str).collect::<Vec<_>>();
    if let Err(error) = run_program(&program, &refs, context.verbose) {
        return Err(AppError::new(
            "DISC_TOOL_FAILED",
            format!("{program} could not create the ISO image."),
        )
        .with_details(json!({
            "tool": program,
            "arguments": refs,
            "cause": error.code,
            "message": error.message,
            "details": error.details,
        }))
        .with_suggestions(&[
            "Check source-directory permissions and available disk space.",
            "Run the same operation with --dry-run to inspect the authoring command.",
        ]));
    }
    let size_bytes = fs::metadata(&output).map(|metadata| metadata.len()).unwrap_or(0);
    if size_bytes == 0 {
        return Err(AppError::new(
            "DISC_TOOL_FAILED",
            "ISO authoring completed without producing a non-empty output.",
        )
        .with_details(json!({"output": absolute_display(&output), "tool": program})));
    }
    value["status"] = json!("success");
    value["output"] = json!(absolute_display(&output));
    value["verification"] = json!({
        "status": "success",
        "valid": true,
        "checks": {"size_bytes": size_bytes, "size_positive": true, "readable": true}
    });
    Ok(value)
}

fn device_presets() -> Vec<Value> {
    vec![
        json!({"id":"iphone","label":"iPhone","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1080}),
        json!({"id":"ipad","label":"iPad","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1440}),
        json!({"id":"android","label":"Android","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":1080}),
        json!({"id":"psp","label":"PSP","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":480}),
        json!({"id":"car","label":"车载通用","container":"mp4","video_codec":"h264","audio_codec":"aac","max_height":720}),
    ]
}

fn device_profile(value: &str) -> Result<DeviceProfile, AppError> {
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

fn apply_device_profile(plan: &mut OperationPlan, profile: DeviceProfile) {
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

fn presets_command(_context: &Context) -> Result<Value, AppError> {
    Ok(json!({"status":"success","operation":"presets","presets":device_presets()}))
}

fn execute_simple_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        if context.overwrite { "-y" } else { "-n" }.to_string(),
    ];
    if !plan.args.iter().any(|argument| argument == "-i") {
        args.extend(["-i".to_string(), input.to_string_lossy().to_string()]);
    }
    args.extend(plan.args.clone());
    args.push(plan.output.to_string_lossy().to_string());
    run_ffmpeg(context, &args)?;
    let verification = if context.verify_after_execute {
        verify_operation(context, input, plan)
            .map_err(|error| verification_failed(&plan.output, error))?
    } else {
        json!({
            "status": "skipped",
            "valid": Value::Null,
            "reason": "disabled_by_configuration",
            "input": absolute_display(input),
            "output": absolute_display(&plan.output),
        })
    };
    if verification.get("valid").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "VERIFY_FAILED",
            "Operation completed but the output did not pass verification.",
        )
        .with_details(verification));
    }
    Ok(json!({
        "status":"success",
        "operation":plan.value.get("operation").cloned().unwrap_or_else(|| json!("media")),
        "input":absolute_display(input),
        "output":absolute_display(&plan.output),
        "strategy":plan.strategy,
        "video":plan.value.get("video").cloned().unwrap_or(Value::Null),
        "audio":plan.value.get("audio").cloned().unwrap_or(Value::Null),
        "quality":plan.value.get("quality").cloned().unwrap_or(Value::Null),
        "quality_loss":plan.value.get("quality_loss").cloned().unwrap_or(Value::Null),
        "reason":plan.value.get("reason").cloned().unwrap_or_else(|| json!([])),
        "warnings":plan.value.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "hardware":plan.value.get("hardware").cloned().unwrap_or(Value::Null),
        "subtitle":plan.value.get("subtitle").cloned().unwrap_or(Value::Null),
        "metadata":plan.value.get("metadata").cloned().unwrap_or(Value::Null),
        "verification":verification
    }))
}

fn verify_operation(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    match plan.value.get("operation").and_then(Value::as_str) {
        Some("compress") => verify_compress_output(context, input, plan),
        Some("resize") => verify_resize_output(context, input, plan),
        Some("extract_audio") => verify_audio_output(context, input, &plan.output),
        Some("audio") => verify_audio_output(context, input, &plan.output),
        Some("thumbnail") => verify_thumbnail_output(context, &plan.output),
        Some("clip") => verify_clip_output(context, input, plan),
        Some("image") | Some("gif") | Some("edit") | Some("merge") | Some("repair")
        | Some("disc") => verify_transformed_output(context, input, plan),
        _ => verify_value(context, input, &plan.output),
    }
}

fn verify_transformed_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let probe = probe_media(&plan.output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let operation = plan.value.get("operation").and_then(Value::as_str).unwrap_or("media");
    let video_present = first_stream(&streams, "video").is_some();
    let audio_present = first_stream(&streams, "audio").is_some();
    let required_video = match operation {
        "audio" => false,
        "disc" => plan.value.get("kind").and_then(Value::as_str) != Some("cd"),
        "merge" => plan.value.get("video_present").and_then(Value::as_bool).unwrap_or(false),
        _ => true,
    };
    let required_audio = match operation {
        "image" | "gif" => false,
        "edit" | "repair" => {
            plan.value.get("audio_present").and_then(Value::as_bool).unwrap_or(true)
        }
        "merge" => plan.value.get("audio_present").and_then(Value::as_bool).unwrap_or(false),
        "disc" => plan.value.get("kind").and_then(Value::as_str) == Some("cd"),
        _ => false,
    };
    let decode_errors = decode_check(context, &plan.output).is_err();
    let size_bytes = fs::metadata(&plan.output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    let video_match = !required_video || video_present;
    let audio_match = !required_audio || audio_present;
    Ok(json!({
        "status": "success",
        "valid": size_positive && video_match && audio_match && !decode_errors,
        "input": absolute_display(input),
        "output": absolute_display(&plan.output),
        "checks": {
            "readable": true,
            "size_bytes": size_bytes,
            "size_positive": size_positive,
            "video_present": video_present,
            "video_match": video_match,
            "audio_present": audio_present,
            "audio_match": audio_match,
            "decode_errors": decode_errors,
        },
        "warnings": []
    }))
}

fn verify_compress_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut verification = verify_value(context, input, &plan.output)?;
    let Some(target_size_bytes) = plan.value.get("target_size_bytes").and_then(Value::as_u64)
    else {
        return Ok(verification);
    };
    let actual_size_bytes = verification
        .get("checks")
        .and_then(|checks| checks.get("size_bytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let target_size_match = actual_size_bytes <= target_size_bytes;
    verification["checks"]["target_size_bytes"] = json!(target_size_bytes);
    verification["checks"]["target_size_match"] = json!(target_size_match);
    if !target_size_match {
        if let Some(warnings) = verification.get_mut("warnings").and_then(Value::as_array_mut) {
            warnings.push(json!(format!(
                "Output size {actual_size_bytes} exceeds target size {target_size_bytes}."
            )));
        }
    }
    let base_valid = verification.get("valid").and_then(Value::as_bool).unwrap_or(false);
    verification["valid"] = json!(base_valid && target_size_match);
    Ok(verification)
}

fn verify_resize_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut verification = verify_value(context, input, &plan.output)?;
    let rendered = probe_media(&plan.output, context.verbose)?;
    let streams =
        rendered.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output_video = first_stream(&streams, "video");
    let width = output_video.as_ref().and_then(|video| video.get("width")).and_then(Value::as_u64);
    let height =
        output_video.as_ref().and_then(|video| video.get("height")).and_then(Value::as_u64);
    let target = plan.value.get("target_dimension").cloned().unwrap_or(Value::Null);
    let axis = target.get("axis").and_then(Value::as_str).unwrap_or("");
    let expected = target.get("effective").and_then(Value::as_u64);
    let actual = match axis {
        "width" => width,
        "height" => height,
        _ => None,
    };
    let target_dimension_match = expected.is_some() && actual == expected;
    let even_dimensions_match = width
        .zip(height)
        .is_some_and(|(width, height)| width.is_multiple_of(2) && height.is_multiple_of(2));
    verification["checks"]["resolution_match"] = json!(target_dimension_match);
    verification["checks"]["target_dimension_match"] = json!(target_dimension_match);
    verification["checks"]["even_dimensions_match"] = json!(even_dimensions_match);
    verification["checks"]["width"] = json!(width);
    verification["checks"]["height"] = json!(height);
    if let Some(warnings) = verification.get_mut("warnings").and_then(Value::as_array_mut) {
        warnings.retain(|warning| {
            warning.as_str() != Some("Output resolution differs from the input.")
        });
        if !target_dimension_match {
            warnings.push(json!("Output resolution does not match the resize plan."));
        }
        if !even_dimensions_match {
            warnings.push(json!("Output dimensions are not both even."));
        }
    }
    let base_valid = verification.get("valid").and_then(Value::as_bool).unwrap_or(false);
    verification["valid"] = json!(base_valid && target_dimension_match && even_dimensions_match);
    Ok(verification)
}

fn verify_audio_output(context: &Context, input: &Path, output: &Path) -> Result<Value, AppError> {
    let probe = probe_media(output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let audio_present = first_stream(&streams, "audio").is_some();
    let decode_errors = decode_check(context, output).is_err();
    let size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    Ok(
        json!({"status":"success","valid":audio_present && !decode_errors && size_positive,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"audio_present":audio_present,"decode_errors":decode_errors},"warnings":[]}),
    )
}

fn verify_thumbnail_output(context: &Context, output: &Path) -> Result<Value, AppError> {
    let probe = probe_media(output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video_present = first_stream(&streams, "video").is_some();
    let decode_errors = decode_check(context, output).is_err();
    let size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    Ok(
        json!({"status":"success","valid":video_present && !decode_errors && size_positive,"output":absolute_display(output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"frame_present":video_present,"decode_errors":decode_errors},"warnings":[]}),
    )
}

fn verify_clip_output(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let probe = probe_media(&plan.output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video_present = first_stream(&streams, "video").is_some();
    let decode_errors = decode_check(context, &plan.output).is_err();
    let size_bytes = fs::metadata(&plan.output).map(|metadata| metadata.len()).unwrap_or(0);
    let size_positive = size_bytes > 0;
    let expected_duration =
        if let Some(duration) = plan.value.get("duration").and_then(Value::as_str) {
            parse_time_seconds(duration).ok()
        } else if let (Some(start), Some(end)) = (
            plan.value.get("start").and_then(Value::as_str),
            plan.value.get("end").and_then(Value::as_str),
        ) {
            Some(
                (parse_time_seconds(end).unwrap_or(0.0) - parse_time_seconds(start).unwrap_or(0.0))
                    .max(0.0),
            )
        } else {
            None
        };
    let duration_match = match (probe.duration_seconds, expected_duration) {
        (Some(actual), Some(expected)) => (actual - expected).abs() <= 0.35,
        _ => true,
    };
    Ok(
        json!({"status":"success","valid":video_present && !decode_errors && duration_match && size_positive,"input":absolute_display(input),"output":absolute_display(&plan.output),"checks":{"readable":true,"size_bytes":size_bytes,"size_positive":size_positive,"video_present":video_present,"duration_match":duration_match,"decode_errors":decode_errors},"warnings":[]}),
    )
}

fn batch_command(context: &Context, args: &BatchArgs) -> Result<Value, AppError> {
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

fn verify_command(context: &Context, input: &Path, output: &Path) -> Result<Value, AppError> {
    ensure_input(input)?;
    ensure_input(output)?;
    let value = verify_value(context, input, output).map_err(|error| {
        AppError::new("VERIFY_FAILED", "Could not complete output verification.").with_details(
            json!({
                "cause": error.code,
                "message": error.message,
                "details": error.details,
            }),
        )
    })?;
    if !value.get("valid").and_then(Value::as_bool).unwrap_or(false) {
        return Err(
            AppError::new("VERIFY_FAILED", "One or more output checks failed.").with_details(value)
        );
    }
    Ok(value)
}

fn verification_failed(output: &Path, error: AppError) -> AppError {
    AppError::new("VERIFY_FAILED", "Could not complete output verification.").with_details(json!({
        "output": absolute_display(output),
        "cause": error.code,
        "message": error.message,
        "details": error.details,
    }))
}

fn verify_value(context: &Context, input: &Path, output: &Path) -> Result<Value, AppError> {
    let source = probe_media(input, context.verbose)?;
    let rendered = probe_media(output, context.verbose)?;
    let source_streams =
        source.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output_streams =
        rendered.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let duration_match = match (source.duration_seconds, rendered.duration_seconds) {
        (Some(a), Some(b)) => {
            let tolerance = (a * 0.02).max(1.5);
            (a - b).abs() <= tolerance
        }
        _ => true,
    };
    let video_present = first_stream(&output_streams, "video").is_some();
    let audio_present = first_stream(&output_streams, "audio").is_some();
    let source_video = first_stream(&source_streams, "video");
    let audio_expected = first_stream(&source_streams, "audio").is_some();
    let output_video = first_stream(&output_streams, "video");
    let source_audio = first_stream(&source_streams, "audio");
    let output_audio = first_stream(&output_streams, "audio");
    let resolution_match = match (source_video, output_video) {
        (Some(a), Some(b)) => {
            a.get("width") == b.get("width") && a.get("height") == b.get("height")
        }
        _ => true,
    };
    let video_codec_match =
        match (first_stream(&source_streams, "video"), first_stream(&output_streams, "video")) {
            (Some(a), Some(b)) => a.get("codec_name") == b.get("codec_name"),
            _ => true,
        };
    let audio_codec_match = match (source_audio, output_audio) {
        (Some(a), Some(b)) => a.get("codec_name") == b.get("codec_name"),
        _ => true,
    };
    let stream_counts_match = ["video", "audio", "subtitle"]
        .iter()
        .all(|kind| stream_count(&source_streams, kind) == stream_count(&output_streams, kind));
    let decode_errors = decode_check(context, output).is_err();
    let audio_match = !audio_expected || audio_present;
    let output_size_bytes = fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0);
    let output_size_positive = output_size_bytes > 0;
    let mut warnings = Vec::new();
    if !audio_match {
        warnings.push("Output is missing the input audio stream.".to_string());
    }
    if !resolution_match {
        warnings.push("Output resolution differs from the input.".to_string());
    }
    if !video_codec_match || !audio_codec_match {
        warnings.push("Output codec differs from the input.".to_string());
    }
    if !stream_counts_match {
        warnings.push("The output stream counts differ from the input.".to_string());
    }
    if !duration_match {
        warnings.push("Output duration differs materially from the input.".to_string());
    }
    let valid =
        output_size_positive && duration_match && video_present && audio_match && !decode_errors;
    Ok(
        json!({"status":"success","valid":valid,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"size_bytes":output_size_bytes,"size_positive":output_size_positive,"duration_match":duration_match,"video_present":video_present,"audio_present":audio_present,"audio_expected":audio_expected,"audio_match":audio_match,"resolution_match":resolution_match,"video_codec_match":video_codec_match,"audio_codec_match":audio_codec_match,"stream_counts_match":stream_counts_match,"decode_errors":decode_errors},"warnings":warnings}),
    )
}

fn capabilities_command(context: &Context) -> Result<Value, AppError> {
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

fn probe_media(input: &Path, verbose: bool) -> Result<Probe, AppError> {
    ensure_input(input)?;
    let result = run_program(
        "ffprobe",
        &[
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &input.to_string_lossy(),
        ],
        verbose,
    )?;
    let raw: Value = serde_json::from_str(&result.stdout).map_err(|error| {
        AppError::new("INVALID_MEDIA", format!("ffprobe returned invalid JSON: {error}"))
    })?;
    let duration_seconds =
        raw.get("format").and_then(|format| number_field(format, "duration")).or_else(|| {
            raw.get("streams").and_then(Value::as_array).and_then(|streams| {
                streams
                    .iter()
                    .filter_map(|stream| number_field(stream, "duration"))
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            })
        });
    Ok(Probe { raw, duration_seconds })
}

#[derive(Debug)]
struct ProcessResult {
    stdout: String,
    stderr: String,
}

fn run_program(program: &str, args: &[&str], verbose: bool) -> Result<ProcessResult, AppError> {
    let output = ProcessCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| process_start_error(program, error))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if verbose && !stderr.trim().is_empty() {
        eprintln!("[{program}] {}", stderr.trim());
    }
    if !output.status.success() {
        return Err(process_failure_error(program, args, output.status, &stderr));
    }
    Ok(ProcessResult { stdout, stderr })
}

fn run_ffmpeg(context: &Context, args: &[String]) -> Result<(), AppError> {
    if context.progress {
        return run_ffmpeg_with_progress(context, args);
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let mut command = ProcessCommand::new("ffmpeg");
    command.args(&refs).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| process_start_error("ffmpeg", error))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::new("FFMPEG_FAILED", "Could not capture FFmpeg stderr."))?;
    let mut reader = BufReader::new(stderr_pipe);
    let mut line = String::new();
    let mut stderr = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            AppError::from_io("FFMPEG_FAILED", "Could not read FFmpeg stderr.", error)
        })?;
        if bytes == 0 {
            break;
        }
        append_stderr_tail(&mut stderr, &line);
        if context.verbose && !line.trim().is_empty() {
            eprint!("[ffmpeg] {line}");
        }
    }
    let status = child
        .wait()
        .map_err(|error| AppError::from_io("FFMPEG_FAILED", "Could not wait for FFmpeg.", error))?;
    if !status.success() {
        return Err(process_failure_error("ffmpeg", &refs, status, &stderr));
    }
    Ok(())
}

fn process_start_error(program: &str, error: io::Error) -> AppError {
    if error.kind() == io::ErrorKind::NotFound {
        AppError::new("FFMPEG_NOT_FOUND", format!("{program} was not found on PATH."))
            .with_suggestions(&[
                "Install FFmpeg and FFprobe.",
                "Run media capabilities to inspect the current environment.",
            ])
    } else {
        AppError::from_io("FFMPEG_FAILED", format!("Could not start {program}."), error)
    }
}

fn process_failure_error(
    program: &str,
    args: &[&str],
    status: std::process::ExitStatus,
    stderr: &str,
) -> AppError {
    let stderr_lower = stderr.to_lowercase();
    let code = if stderr_lower.contains("unknown encoder")
        || (stderr_lower.contains("encoder") && stderr_lower.contains("not found"))
        || stderr_lower.contains("experimental feature")
        || stderr_lower.contains("only supports")
        || stderr_lower.contains("does not support")
    {
        "ENCODER_UNAVAILABLE"
    } else if stderr_lower.contains("unknown decoder") {
        "DECODER_UNAVAILABLE"
    } else if stderr_lower.contains("cannot create compression session")
        || stderr_lower.contains("no capable devices found")
        || stderr_lower.contains("hardware encoder")
    {
        "HARDWARE_UNAVAILABLE"
    } else {
        "FFMPEG_FAILED"
    };
    let error = AppError::new(code, format!("{program} exited with status {status}."))
        .with_details(json!({"command":program,"arguments":args,"stderr":stderr}));
    if code == "HARDWARE_UNAVAILABLE" {
        error.with_suggestions(&[
            "Retry with --hardware cpu.",
            "Run media capabilities to inspect available hardware encoders.",
        ])
    } else {
        error
    }
}

fn run_ffmpeg_with_progress(context: &Context, args: &[String]) -> Result<(), AppError> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let expected_duration = progress_duration_seconds(args)
        .or_else(|| progress_input_duration_seconds(args, context.verbose));
    let started_at = Instant::now();
    let mut command = ProcessCommand::new("ffmpeg");
    command
        .args(&refs)
        .stdin(Stdio::null())
        .args(["-progress", "pipe:2", "-nostats"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| process_start_error("ffmpeg", error))?;
    emit_progress_event(context, "start", Some(0.0), None, None, Some(0.0), None);
    let stderr_pipe = child.stderr.take().ok_or_else(|| {
        AppError::new("FFMPEG_FAILED", "Could not capture FFmpeg progress output.")
    })?;
    let mut reader = BufReader::new(stderr_pipe);
    let mut line = String::new();
    let mut stderr = String::new();
    let mut out_time_ms: Option<f64> = None;
    let mut speed: Option<String> = None;
    let mut ended = false;
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            AppError::from_io("FFMPEG_FAILED", "Could not read FFmpeg progress output.", error)
        })?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim_end();
        append_stderr_tail(&mut stderr, trimmed);
        append_stderr_tail(&mut stderr, "\n");
        if let Some((key, value)) = trimmed.split_once('=') {
            match key {
                "out_time_ms" => out_time_ms = value.parse::<f64>().ok(),
                "speed" => speed = Some(value.to_string()),
                "progress" => {
                    let is_end = value == "end";
                    let normalized =
                        expected_duration.zip(out_time_ms).map(|(duration, milliseconds)| {
                            (milliseconds / 1_000_000.0 / duration).clamp(0.0, 1.0)
                        });
                    let elapsed_seconds = started_at.elapsed().as_secs_f64();
                    let remaining_seconds =
                        estimated_remaining_seconds(normalized, elapsed_seconds);
                    emit_progress_event(
                        context,
                        if is_end { "complete" } else { "progress" },
                        normalized,
                        out_time_ms,
                        speed.as_deref(),
                        Some(elapsed_seconds),
                        remaining_seconds,
                    );
                    ended = is_end;
                }
                _ => {}
            }
        } else if context.verbose && !trimmed.is_empty() {
            eprintln!("[ffmpeg] {trimmed}");
        }
    }
    let status = child
        .wait()
        .map_err(|error| AppError::from_io("FFMPEG_FAILED", "Could not wait for FFmpeg.", error))?;
    if !status.success() {
        return Err(process_failure_error("ffmpeg", &refs, status, &stderr));
    }
    if !ended {
        emit_progress_event(
            context,
            "complete",
            Some(1.0),
            out_time_ms,
            speed.as_deref(),
            Some(started_at.elapsed().as_secs_f64()),
            Some(0.0),
        );
    }
    Ok(())
}

fn emit_progress_event(
    context: &Context,
    event: &str,
    value: Option<f64>,
    out_time_ms: Option<f64>,
    speed: Option<&str>,
    elapsed_seconds: Option<f64>,
    remaining_seconds: Option<f64>,
) {
    if context.json {
        eprintln!(
            "{}",
            json!({"event":event,"value":value,"out_time_ms":out_time_ms,"speed":speed,"elapsed_seconds":elapsed_seconds,"remaining_seconds":remaining_seconds})
        );
        return;
    }
    match event {
        "start" => eprintln!("Converting media..."),
        "complete" => {
            let elapsed = elapsed_seconds
                .map(|seconds| format!(" Elapsed: {}.", format_progress_time(seconds)))
                .unwrap_or_default();
            eprintln!("Complete.{elapsed}");
        }
        _ => {
            let percent = value
                .map(|progress| format!("{:.0}%", progress * 100.0))
                .unwrap_or_else(|| "unknown".to_string());
            let elapsed =
                elapsed_seconds.map(|seconds| format!("elapsed {}", format_progress_time(seconds)));
            let remaining = remaining_seconds
                .map(|seconds| format!("remaining ~{}", format_progress_time(seconds)));
            let speed = speed.map(|value| format!("speed {value}"));
            let details =
                [elapsed, remaining, speed].into_iter().flatten().collect::<Vec<_>>().join(" | ");
            if details.is_empty() {
                eprintln!("Progress: {percent}");
            } else {
                eprintln!("Progress: {percent} | {details}");
            }
        }
    }
}

fn append_stderr_tail(buffer: &mut String, text: &str) {
    buffer.push_str(text);
    if buffer.len() <= MAX_CAPTURED_STDERR_BYTES {
        return;
    }
    let mut start = buffer.len() - MAX_CAPTURED_STDERR_BYTES;
    while !buffer.is_char_boundary(start) {
        start += 1;
    }
    buffer.drain(..start);
}

fn progress_duration_seconds(args: &[String]) -> Option<f64> {
    args.windows(2).find(|pair| pair[0] == "-t").and_then(|pair| parse_time_seconds(&pair[1]).ok())
}

fn progress_input_duration_seconds(args: &[String], verbose: bool) -> Option<f64> {
    let input = args.windows(2).find(|pair| pair[0] == "-i").map(|pair| Path::new(&pair[1]))?;
    probe_media(input, verbose).ok()?.duration_seconds
}

fn estimated_remaining_seconds(progress: Option<f64>, elapsed_seconds: f64) -> Option<f64> {
    let progress = progress?;
    if progress <= f64::EPSILON {
        return None;
    }
    Some((elapsed_seconds * (1.0 - progress) / progress).max(0.0))
}

fn format_progress_time(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn decode_check(context: &Context, input: &Path) -> Result<(), AppError> {
    // Decode only a bounded sample. This catches malformed headers/frames while
    // avoiding an infinite read for intentionally looping animated GIFs.
    let refs = ["-v", "error", "-t", "1", "-i", &input.to_string_lossy(), "-f", "null", "-"];
    run_program("ffmpeg", &refs, context.verbose).map(|_| ())
}

fn ensure_input(input: &Path) -> Result<(), AppError> {
    if !input.exists() {
        return Err(AppError::new(
            "FILE_NOT_FOUND",
            format!("Input file does not exist: {}", input.display()),
        )
        .with_details(json!({"path":input.display().to_string()})));
    }
    if !input.is_file() {
        return Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("Input is not a file: {}", input.display()),
        ));
    }
    Ok(())
}

fn program_available(program: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|directory| {
        let candidate = directory.join(program);
        candidate.is_file() && (cfg!(unix) || candidate.extension().is_some())
    })
}

fn resolve_output(
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

fn next_available_path(path: &Path) -> PathBuf {
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

fn timestamp_suffix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
fn same_path(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a).ok() == fs::canonicalize(b).ok() || a == b
}
fn absolute_display(path: &Path) -> String {
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
fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| {
        item.as_f64().or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}
fn first_stream(streams: &[Value], kind: &str) -> Option<Value> {
    streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some(kind))
        .cloned()
}
fn stream_count(streams: &[Value], kind: &str) -> usize {
    streams
        .iter()
        .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some(kind))
        .count()
}

fn normalize_video(stream: &Value) -> Value {
    let codec = stream.get("codec_name").and_then(Value::as_str).unwrap_or("unknown");
    json!({"index":stream.get("index"),"codec":codec,"profile":stream.get("profile"),"width":stream.get("width"),"height":stream.get("height"),"fps":parse_ratio(stream.get("avg_frame_rate").and_then(Value::as_str).or_else(|| stream.get("r_frame_rate").and_then(Value::as_str))),"pixel_format":stream.get("pix_fmt"),"bit_depth":bit_depth(stream),"hdr":hdr_name(stream),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default": disposition_flag(stream, "default")})
}
fn normalize_audio(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"sample_rate":number_field(stream,"sample_rate").map(|v| v as u64),"channels":stream.get("channels"),"channel_layout":stream.get("channel_layout"),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default":disposition_flag(stream,"default")})
}
fn normalize_subtitle(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"language":stream.get("tags").and_then(|tags| tags.get("language")),"forced":disposition_flag(stream,"forced"),"default":disposition_flag(stream,"default")})
}
fn bit_depth(stream: &Value) -> Option<u8> {
    let explicit = stream
        .get("bits_per_raw_sample")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u8>().ok())
        .or_else(|| {
            stream
                .get("bits_per_raw_sample")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
        });
    if explicit.is_some() {
        return explicit;
    }
    let pixel_format = stream.get("pix_fmt").and_then(Value::as_str).unwrap_or("");
    [16, 14, 12, 10, 9, 8]
        .into_iter()
        .find(|depth| pixel_format.contains(&depth.to_string()))
        .or_else(|| (!pixel_format.is_empty()).then_some(8))
}
fn disposition_flag(stream: &Value, key: &str) -> bool {
    stream.get("disposition").and_then(|value| value.get(key)).and_then(Value::as_u64).unwrap_or(0)
        == 1
}
fn parse_ratio(value: Option<&str>) -> Option<f64> {
    let value = value?;
    let mut parts = value.split('/');
    let numerator = parts.next()?.parse::<f64>().ok()?;
    let denominator = parts.next().unwrap_or("1").parse::<f64>().ok()?;
    if denominator == 0.0 {
        None
    } else {
        Some((numerator / denominator * 1000.0).round() / 1000.0)
    }
}
fn hdr_name(stream: &Value) -> Value {
    let transfer = stream.get("color_transfer").and_then(Value::as_str).unwrap_or("");
    let primaries = stream.get("color_primaries").and_then(Value::as_str).unwrap_or("");
    if transfer.contains("smpte2084") {
        json!("HDR10")
    } else if transfer.contains("arib-std-b67") {
        json!("HLG")
    } else if primaries.contains("bt2020") {
        json!("HDR")
    } else {
        Value::Null
    }
}
fn display_codec(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" | "h265" => "HEVC".into(),
        "aac" => "AAC".into(),
        "truehd" => "TrueHD".into(),
        "opus" => "Opus".into(),
        "flac" => "FLAC".into(),
        _ => codec.to_uppercase(),
    }
}

fn inspect_container_label(input: &Path, format_name: &str) -> String {
    match input.extension().and_then(OsStr::to_str).map(|value| value.to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "mp4".to_string(),
        Some("mkv") => "matroska".to_string(),
        Some("mov") => "mov".to_string(),
        Some("webm") => "webm".to_string(),
        Some("avi") => "avi".to_string(),
        Some("wmv") | Some("asf") => "wmv".to_string(),
        Some("flv") => "flv".to_string(),
        Some("ogv") | Some("ogg") => "ogv".to_string(),
        Some("3gp") | Some("3g2") => "3gp".to_string(),
        Some("mpg") | Some("mpeg") => "mpeg".to_string(),
        Some("vob") => "vob".to_string(),
        Some("swf") => "swf".to_string(),
        _ => format_name.to_string(),
    }
}

fn internal_container(input: &Path, format_name: &str) -> String {
    match input.extension().and_then(OsStr::to_str).map(|value| value.to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "mp4".to_string(),
        Some("mkv") => "mkv".to_string(),
        Some("mov") => "mov".to_string(),
        Some("webm") => "webm".to_string(),
        Some("avi") => "avi".to_string(),
        Some("wmv") | Some("asf") => "wmv".to_string(),
        Some("flv") => "flv".to_string(),
        Some("ogv") | Some("ogg") => "ogv".to_string(),
        Some("3gp") | Some("3g2") => "3gp".to_string(),
        Some("mpg") | Some("mpeg") => "mpeg".to_string(),
        Some("vob") => "vob".to_string(),
        Some("swf") => "swf".to_string(),
        _ if format_name.contains("matroska") => "mkv".to_string(),
        _ => format_name.to_string(),
    }
}

fn normalize_container(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    let normalized = match value.as_str() {
        "mp4" | "m4v" => "mp4",
        "mkv" | "matroska" => "mkv",
        "mov" | "quicktime" => "mov",
        "webm" => "webm",
        "avi" => "avi",
        "wmv" | "asf" => "wmv",
        "flv" => "flv",
        "ogv" | "ogg" => "ogv",
        "3gp" | "3g2" => "3gp",
        "mpg" | "mpeg" | "mpeg1" | "mpeg2" => "mpeg",
        "vob" | "dvd" => "vob",
        "swf" => "swf",
        _ => {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                format!("Unsupported target container: {value}"),
            ))
        }
    };
    Ok(normalized.to_string())
}
fn normalize_audio_format(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    if [
        "mp3", "aac", "m4a", "flac", "wav", "opus", "ogg", "wma", "aiff", "aif", "alac", "amr",
        "ac3", "mp2",
    ]
    .contains(&value.as_str())
    {
        Ok(value)
    } else {
        Err(AppError::new("UNSUPPORTED_FORMAT", format!("Unsupported audio format: {value}")))
    }
}

fn audio_codec_for_format(format: &str) -> &'static str {
    match format {
        "m4a" | "aac" => "aac",
        "mp3" => "mp3",
        "flac" => "flac",
        "wav" => "wav",
        "opus" => "opus",
        "ogg" => "vorbis",
        "wma" => "wmav2",
        "aiff" | "aif" => "aiff",
        "alac" => "alac",
        "amr" => "amr_nb",
        "ac3" => "ac3",
        "mp2" => "mp2",
        _ => "aac",
    }
}

fn audio_output_extension(format: &str) -> String {
    match format {
        "aif" | "aiff" => "aiff".to_string(),
        "alac" => "m4a".to_string(),
        _ => format.to_string(),
    }
}

fn audio_copy_compatible(codec: &str, format: &str) -> bool {
    match format {
        "m4a" | "aac" => codec == "aac",
        "mp3" => codec == "mp3",
        "flac" => codec == "flac",
        "wav" => codec.starts_with("pcm_") || codec == "pcm_s16le",
        "opus" => codec == "opus",
        "ogg" => codec == "vorbis",
        "wma" => codec == "wmav1" || codec == "wmav2",
        "aiff" | "aif" => codec.starts_with("pcm_") || codec == "alac",
        "alac" => codec == "alac",
        "amr" => codec == "amr_nb" || codec == "amr_wb",
        "ac3" => codec == "ac3",
        "mp2" => codec == "mp2",
        _ => false,
    }
}

fn is_video_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["h264", "h265", "hevc", "mpeg4", "av1", "vp9"].contains(&codec),
        "webm" => ["vp8", "vp9", "av1"].contains(&codec),
        "mkv" | "avi" => true,
        "wmv" => ["wmv1", "wmv2", "msmpeg4", "msmpeg4v2", "h264"].contains(&codec),
        "flv" => ["flv1", "h263", "h264"].contains(&codec),
        "ogv" => ["theora"].contains(&codec),
        "3gp" => ["h264", "mpeg4", "h263"].contains(&codec),
        "mpeg" | "vob" => ["mpeg1video", "mpeg2video"].contains(&codec),
        "swf" => ["flv1", "h263"].contains(&codec),
        _ => false,
    }
}
fn is_audio_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["aac", "mp3", "ac3", "eac3"].contains(&codec),
        "webm" => ["opus", "vorbis"].contains(&codec),
        "mkv" | "avi" => true,
        "wmv" => ["wmav1", "wmav2", "wma", "aac", "mp3"].contains(&codec),
        "flv" => ["mp3", "aac"].contains(&codec),
        "ogv" => ["vorbis", "opus"].contains(&codec),
        "3gp" => ["aac", "amr_nb", "amr_wb", "mp3"].contains(&codec),
        "mpeg" | "vob" => ["mp1", "mp2", "mp3", "ac3", "dts", "pcm_s16be"].contains(&codec),
        "swf" => ["mp3"].contains(&codec),
        _ => false,
    }
}

fn default_video_codec_for_container(container: &str) -> &'static str {
    match container {
        "webm" => "vp9",
        "ogv" => "theora",
        "flv" | "swf" => "flv1",
        "wmv" => "wmv2",
        "mpeg" | "vob" => "mpeg2video",
        _ => "h264",
    }
}

fn default_audio_codec_for_container(container: &str) -> &'static str {
    match container {
        "webm" | "ogv" => "opus",
        "wmv" => "wmav2",
        "flv" | "swf" => "mp3",
        "mpeg" | "vob" => "mp2",
        _ => "aac",
    }
}

fn validate_transcode_compatibility(
    stream: &str,
    action: &str,
    codec: &str,
    container: &str,
) -> Result<(), AppError> {
    let compatible = match stream {
        "video" => is_video_compatible(container, codec),
        "audio" => is_audio_compatible(container, codec),
        _ => false,
    };
    if action != "transcode" || compatible {
        return Ok(());
    }
    let suggestions: &[&str] = if stream == "video" {
        &[
            "Use --video-codec auto to select a compatible codec.",
            "Choose a compatible target container.",
        ]
    } else {
        &[
            "Use --audio-codec auto to select a compatible codec.",
            "Choose a compatible target container.",
        ]
    };
    Err(AppError::new(
        "UNSUPPORTED_CODEC",
        format!(
            "Cannot encode {} {stream} into {}.",
            display_codec(codec),
            container.to_uppercase()
        ),
    )
    .with_details(json!({"stream":stream,"codec":codec,"container":container}))
    .with_suggestions(suggestions))
}

fn preferred_codec(requested: &str, fallback: &str) -> String {
    if requested == "auto" || requested == "copy" {
        fallback.to_string()
    } else {
        requested.to_string()
    }
}

fn is_hardware_encoder(encoder: &str) -> bool {
    encoder.ends_with("_videotoolbox")
        || encoder.ends_with("_nvenc")
        || encoder.ends_with("_qsv")
        || encoder.ends_with("_amf")
}

fn remove_option(args: &mut Vec<String>, option: &str) {
    let mut index = 0;
    while index + 1 < args.len() {
        if args[index] == option {
            args.drain(index..=index + 1);
        } else {
            index += 1;
        }
    }
}

fn video_encode_args(
    codec: &str,
    quality: &str,
    encoder_override: Option<&str>,
) -> Result<Vec<String>, AppError> {
    if codec == "copy" {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            "Video codec `copy` cannot be used for a transcode plan.",
        ));
    }
    let codec = preferred_codec(codec, "h264");
    let default_encoder =
        software_encoder_candidates(&codec).first().copied().ok_or_else(|| {
            AppError::new("UNSUPPORTED_CODEC", format!("Unsupported video codec: {codec}"))
        })?;
    if let Some(encoder) = encoder_override.filter(|encoder| is_hardware_encoder(encoder)) {
        return Ok(vec!["-c:v".into(), encoder.to_string()]);
    }
    let encoder = encoder_override.unwrap_or(default_encoder);
    match codec.as_str() {
        "h264" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "18",
                "high" => "20",
                "balanced" => "23",
                "small" => "28",
                "tiny" => "32",
                _ => "23",
            }
            .into(),
        ]),
        "h265" | "hevc" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "20",
                "high" => "23",
                "balanced" => "26",
                "small" => "30",
                "tiny" => "34",
                _ => "26",
            }
            .into(),
        ]),
        "vp9" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-deadline".into(),
            "good".into(),
            "-cpu-used".into(),
            "2".into(),
            "-crf".into(),
            match quality {
                "lossless" => "0",
                "very-high" => "18",
                "high" => "24",
                "balanced" => "30",
                "small" => "36",
                "tiny" => "42",
                _ => "30",
            }
            .into(),
            "-b:v".into(),
            "0".into(),
        ]),
        "av1" => {
            let mut args = vec![
                "-c:v".into(),
                encoder.to_string(),
                "-crf".into(),
                match quality {
                    "lossless" => "0",
                    "very-high" => "24",
                    "high" => "28",
                    "balanced" => "30",
                    "small" => "34",
                    "tiny" => "38",
                    _ => "30",
                }
                .into(),
            ];
            if encoder == "libaom-av1" {
                args.extend(["-b:v".into(), "0".into()]);
            }
            Ok(args)
        }
        "mpeg2video" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-q:v".into(),
            match quality {
                "lossless" => "1",
                "very-high" => "2",
                "high" => "3",
                "balanced" => "5",
                "small" => "7",
                "tiny" => "9",
                _ => "5",
            }
            .into(),
        ]),
        "flv1" | "wmv2" | "theora" | "mpeg4" => Ok(vec![
            "-c:v".into(),
            encoder.to_string(),
            "-q:v".into(),
            match quality {
                "lossless" => "1",
                "very-high" => "3",
                "high" => "5",
                "balanced" => "7",
                "small" => "9",
                "tiny" => "12",
                _ => "7",
            }
            .into(),
        ]),
        _ => unreachable!("software encoder candidates validate the codec"),
    }
}

fn quality_name(quality: Quality) -> &'static str {
    match quality {
        Quality::Lossless => "lossless",
        Quality::VeryHigh => "very-high",
        Quality::High => "high",
        Quality::Balanced => "balanced",
        Quality::Small => "small",
        Quality::Tiny => "tiny",
    }
}

fn hardware_quality_bitrate(quality: Quality) -> &'static str {
    match quality {
        Quality::Lossless => "12M",
        Quality::VeryHigh => "8M",
        Quality::High => "5M",
        Quality::Balanced => "3M",
        Quality::Small => "2M",
        Quality::Tiny => "1M",
    }
}
fn audio_encode_args(codec: &str, bitrate: &str) -> Result<Vec<String>, AppError> {
    if codec == "copy" {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            "Audio codec `copy` cannot be used for a transcode plan.",
        ));
    }
    match preferred_codec(codec, "aac").as_str() {
        "aac" => Ok(vec!["-c:a".into(), "aac".into(), "-b:a".into(), bitrate.into()]),
        "opus" => Ok(vec!["-c:a".into(), "libopus".into(), "-b:a".into(), bitrate.into()]),
        "mp3" => Ok(vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), bitrate.into()]),
        "flac" => Ok(vec!["-c:a".into(), "flac".into()]),
        "wav" => Ok(vec!["-c:a".into(), "pcm_s16le".into()]),
        "vorbis" => Ok(vec![
            "-strict".into(),
            "-2".into(),
            "-c:a".into(),
            "vorbis".into(),
            "-b:a".into(),
            bitrate.into(),
        ]),
        "wmav2" => Ok(vec!["-c:a".into(), "wmav2".into(), "-b:a".into(), bitrate.into()]),
        "aiff" => Ok(vec!["-c:a".into(), "pcm_s16be".into()]),
        "alac" => Ok(vec!["-c:a".into(), "alac".into()]),
        "amr_nb" => Ok(vec![
            "-c:a".into(),
            "libopencore_amrnb".into(),
            "-ar".into(),
            "8000".into(),
            "-ac".into(),
            "1".into(),
            "-b:a".into(),
            "12.2k".into(),
        ]),
        "ac3" => Ok(vec!["-c:a".into(), "ac3".into(), "-b:a".into(), bitrate.into()]),
        "mp2" => Ok(vec!["-c:a".into(), "mp2".into(), "-b:a".into(), bitrate.into()]),
        other => {
            Err(AppError::new("UNSUPPORTED_CODEC", format!("Unsupported audio codec: {other}")))
        }
    }
}

fn audio_container_args(format: &str) -> Vec<String> {
    match format {
        // ALAC is an audio codec carried by the ISO BMFF/M4A container; an
        // explicit format keeps custom `.alac` output paths deterministic.
        "alac" => vec!["-f".to_string(), "ipod".to_string()],
        _ => Vec::new(),
    }
}
fn subtitle_warnings(streams: &[Value], container: &str) -> Vec<String> {
    let subtitle_count = stream_count(streams, "subtitle");
    if subtitle_count == 0 {
        return Vec::new();
    }
    if matches!(container, "mp4" | "webm")
        && streams
            .iter()
            .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("subtitle"))
            .any(|stream| !subtitle_conversion_supported(container, stream))
    {
        let target = if container == "mp4" { "mov_text" } else { "WebVTT" };
        return vec![format!(
            "Some subtitle streams cannot be safely converted to {target}; review the plan before execution."
        )];
    }
    match container {
        "mp4" => {
            vec!["Subtitle streams will be converted to mov_text for MP4 compatibility.".to_string()]
        }
        "webm" => {
            vec!["Subtitle streams will be converted to WebVTT for WebM compatibility.".to_string()]
        }
        _ => Vec::new(),
    }
}

fn subtitle_strategy(container: &str, streams: &[Value]) -> &'static str {
    if stream_count(streams, "subtitle") == 0 {
        "none"
    } else if matches!(container, "mp4" | "webm")
        && streams
            .iter()
            .filter(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("subtitle"))
            .any(|stream| !subtitle_conversion_supported(container, stream))
    {
        "warning"
    } else {
        match container {
            "mp4" => "convert_to_mov_text",
            "webm" => "convert_to_webvtt",
            _ => "copy",
        }
    }
}

fn subtitle_conversion_supported(container: &str, stream: &Value) -> bool {
    let codec = stream.get("codec_name").and_then(Value::as_str).unwrap_or("").to_lowercase();
    match container {
        "mp4" => {
            ["subrip", "srt", "ass", "ssa", "webvtt", "mov_text", "text"].contains(&codec.as_str())
        }
        "webm" => ["subrip", "srt", "ass", "ssa", "webvtt", "text"].contains(&codec.as_str()),
        _ => true,
    }
}

fn subtitle_ffmpeg_args(container: &str, streams: &[Value]) -> Vec<String> {
    if stream_count(streams, "subtitle") == 0 {
        return Vec::new();
    }
    let codec = match container {
        "mp4" => "mov_text",
        "webm" => "webvtt",
        _ => "copy",
    };
    vec!["-map".to_string(), "0:s?".to_string(), "-c:s".to_string(), codec.to_string()]
}

fn subtitle_codec_args(container: &str, streams: &[Value]) -> Vec<String> {
    if stream_count(streams, "subtitle") == 0 {
        return Vec::new();
    }
    let codec = match container {
        "mp4" => "mov_text",
        "webm" => "webvtt",
        _ => "copy",
    };
    vec!["-c:s".to_string(), codec.to_string()]
}

fn parse_resolution(value: &str) -> Result<u32, AppError> {
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

fn even_dimension(value: u32) -> Result<u32, AppError> {
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
fn parse_thumbnail_time(value: &str, duration: Option<f64>) -> Result<String, AppError> {
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
fn parse_time_seconds(value: &str) -> Result<f64, AppError> {
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
fn parse_size(value: &str) -> Result<u64, AppError> {
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

fn collect_inputs(input: &str, recursive: bool) -> Result<Vec<PathBuf>, AppError> {
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
fn walk_files(root: &Path, recursive: bool) -> Vec<PathBuf> {
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
fn is_media_extension(path: &Path) -> bool {
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
fn wildcard_match(value: &str, pattern: &str) -> bool {
    wildcard_match_bytes(value.as_bytes(), pattern.as_bytes())
}
fn wildcard_match_bytes(value: &[u8], pattern: &[u8]) -> bool {
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

fn format_hardware(mode: HardwareMode) -> &'static str {
    match mode {
        HardwareMode::Auto => "auto",
        HardwareMode::Cpu => "cpu",
        HardwareMode::Gpu => "gpu",
    }
}
fn print_json(value: &Value) {
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    serde_json::to_writer_pretty(&mut stdout, value).expect("stdout should be writable");
    writeln!(stdout).expect("stdout should be writable");
}
fn print_human(value: &Value) {
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        println!("{status}");
    }
    if let Some(output) = value.get("output").and_then(Value::as_str) {
        println!("output: {output}");
    }
    if let Some(strategy) = value.get("strategy").and_then(Value::as_str) {
        println!("strategy: {strategy}");
    }
    if value.get("operation").and_then(Value::as_str) == Some("inspect") {
        println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            parse_quality_name("very_high".to_string()).unwrap() as u8,
            Quality::VeryHigh as u8
        );
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
        let subtitle = subtitle_filter(
            Path::new("captions.srt"),
            Some("FontSize=24,PrimaryColour=&H00FFFFFF"),
        )
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
}
