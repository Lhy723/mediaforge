use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const DEFAULT_AUDIO_BITRATE: &str = "256k";
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
    #[arg(long, global = true, help = "Emit FFmpeg progress events as NDJSON on stderr")]
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
    Batch(BatchArgs),
    Verify(VerifyArgs),
    Capabilities,
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

fn main() {
    let cli = Cli::parse();
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
        Command::Batch(args) => batch_command(context, &args),
        Command::Verify(args) => verify_command(context, &args.input, &args.output),
        Command::Capabilities => capabilities_command(context),
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
        "ffmpeg" => raw_ffmpeg_command(&tool_context, &request.args.unwrap_or_default()),
        _ => Err(AppError::new(
            "INVALID_ARGUMENT",
            format!("Unsupported Tool operation: {}", request.operation),
        )
        .with_suggestions(&[
            "Use a semantic operation such as inspect_media, plan_media_operation, convert_media, compress_media, resize_media, clip_media, extract_audio, create_thumbnail, batch, verify_media, capabilities, or ffmpeg.",
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
        if args.target_size.is_some() || args.quality.is_some() {
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
        "convert" => {}
        _ => {
            return Err(AppError::new(
                "INVALID_ARGUMENT",
                format!("Planning operation {operation} is not supported."),
            )
            .with_suggestions(&[
                "Use convert, compress, resize, clip, extract_audio, or thumbnail.",
            ]));
        }
    }
    let plan = build_convert_plan(
        &planning_context,
        &args.input,
        args.to.as_deref(),
        args.output.as_deref(),
        args.video_codec.as_deref().unwrap_or(&planning_context.default_video_codec),
        args.audio_codec.as_deref().unwrap_or(&planning_context.default_audio_codec),
        args.hardware.unwrap_or(planning_context.default_hardware),
    )?;
    let mut value = plan.value;
    if let Some(object) = value.as_object_mut() {
        object.insert("status".to_string(), json!("planned"));
        object.insert("will_execute".to_string(), json!(false));
    }
    Ok(value)
}

fn convert_command(context: &Context, args: &ConvertArgs) -> Result<Value, AppError> {
    let plan = build_convert_plan(
        context,
        &args.input,
        args.to.as_deref(),
        args.output.as_deref(),
        args.video_codec.as_deref().unwrap_or(&context.default_video_codec),
        args.audio_codec.as_deref().unwrap_or(&context.default_audio_codec),
        args.hardware.unwrap_or(context.default_hardware),
    )?;
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

fn build_convert_plan(
    context: &Context,
    input: &Path,
    to: Option<&str>,
    output: Option<&Path>,
    video_codec: &str,
    audio_codec: &str,
    hardware: HardwareMode,
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
    let hardware_selection =
        select_video_hardware(context, hardware, &video_codec, video_action == "transcode")?;
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
            &video_codec,
            "balanced",
            hardware_selection.encoder.as_deref(),
        )?);
    }
    if audio_action == "copy" {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(&audio_codec, DEFAULT_AUDIO_BITRATE)?);
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
        "video": {"action": video_action, "codec": if video_action == "copy" { source_video_codec.clone() } else { preferred_codec(&video_codec, "h264") }},
        "audio": {"action": audio_action, "from": source_audio_codec, "to": if audio_action == "copy" { Value::Null } else { json!(preferred_codec(&audio_codec, "aac")) }},
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
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().to_string(),
    ];
    args.extend(plan.args.clone());
    args.push(plan.output.to_string_lossy().to_string());
    run_ffmpeg(context, &args)?;
    let verification = if context.verify_after_execute {
        verify_value(context, input, &plan.output)
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
        "verification": verification,
    }))
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
    let mut ffmpeg_args =
        vec!["-map".to_string(), "0:v:0".to_string(), "-map".to_string(), "0:a?".to_string()];
    ffmpeg_args.extend(video_encode_args(
        "h264",
        quality_name(quality),
        hardware_selection.encoder.as_deref(),
    )?);
    let mut notes = vec![format!(
        "Compressing {} video with the {:?} quality preset.",
        video.get("codec_name").and_then(Value::as_str).unwrap_or("unknown"),
        quality
    )];
    match args.target_size.as_deref().map(parse_size) {
        Some(Ok(target_bytes)) => {
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
            notes.push(format!("Target size is approximately {} bytes.", target_bytes));
        }
        Some(Err(error)) => return Err(error),
        None => {
            if hardware_selection.encoder.is_some() {
                ffmpeg_args
                    .extend(["-b:v".to_string(), hardware_quality_bitrate(quality).to_string()]);
            } else {
                let crf = match quality {
                    Quality::Lossless => 0,
                    Quality::VeryHigh => 18,
                    Quality::High => 21,
                    Quality::Balanced => 24,
                    Quality::Small => 28,
                    Quality::Tiny => 32,
                };
                ffmpeg_args.extend([
                    "-crf".to_string(),
                    crf.to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                ]);
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
        value: json!({"status":"success","operation":"compress","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"transcode","quality":quality,"quality_loss":"video_and_audio","reason":notes,"hardware":{"requested":hardware_selection.requested,"selected":hardware_selection.selected,"encoder":hardware_selection.encoder,"reason":hardware_selection.reason},"subtitle":{"action":subtitle_strategy("mp4", &streams)},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
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
    execute_plan(context, &args.input, &plan)
}

fn resize_command(context: &Context, args: &ResizeArgs) -> Result<Value, AppError> {
    ensure_input(&args.input)?;
    if args.width.is_none() && args.resolution.is_none() {
        return Err(AppError::new("INVALID_ARGUMENT", "Provide --width or --resolution."));
    }
    if args.width.is_some() && args.resolution.is_some() {
        return Err(AppError::new("INVALID_ARGUMENT", "Use only one of --width or --resolution."));
    }
    let height = args.resolution.as_deref().map(parse_resolution).transpose()?;
    let filter = if let Some(width) = args.width {
        format!("scale={width}:-2")
    } else {
        format!("scale=-2:{:?}", height.unwrap())
    };
    let probe = probe_media(&args.input, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let mut ffmpeg_args = vec![
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
        "-vf".to_string(),
        filter.clone(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
    ];
    ffmpeg_args.extend(subtitle_ffmpeg_args("mp4", &streams));
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"resize","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"video_transcode","filter":filter,"preserve_aspect_ratio":true,"even_dimensions":true,"quality_loss":"video_only","subtitle":{"action":subtitle_strategy("mp4", &streams)},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
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
        ffmpeg_args.extend([
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
            "-crf".to_string(),
            "20".to_string(),
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
    ffmpeg_args.extend(["-map_metadata".to_string(), "0".to_string()]);
    let strategy = if copy_compatible { "stream_copy" } else { "precise_transcode" };
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"clip","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":strategy,"start":args.start,"duration":args.duration,"end":args.end,"quality_loss":if copy_compatible { "none" } else { "video_and_audio" },"reason":if copy_compatible { "Start is at zero and source streams are compatible with MP4; stream copy avoids re-encoding." } else { "Precise clipping re-encodes to honor the requested boundary." },"subtitle":{"action":subtitle_strategy("mp4", &streams)},"warnings":subtitle_warnings(&streams, "mp4"),"ffmpeg_args":ffmpeg_args}),
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
    let output = resolve_output(context, &args.input, args.output.as_deref(), &format)?;
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
    ffmpeg_args.extend(["-map_metadata".to_string(), "0".to_string()]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"extract_audio","input":absolute_display(&args.input),"output":absolute_display(&output),"format":format,"source_codec":source_audio_codec,"target_codec":target_audio_codec,"strategy":if copy_audio { "copy" } else { "transcode" },"quality_loss":if copy_audio { "none" } else { "audio_only" },"ffmpeg_args":ffmpeg_args}),
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
        value: json!({"status":"success","operation":"thumbnail","input":absolute_display(&args.input),"output":absolute_display(&output),"at":at,"format":"jpg","ffmpeg_args":ffmpeg_args}),
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

fn execute_simple_plan(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    let mut args = vec!["-hide_banner".to_string(), "-nostdin".to_string(), "-y".to_string()];
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
    Ok(
        json!({"status":"success","operation":plan.value.get("operation").cloned().unwrap_or_else(|| json!("media")),"input":absolute_display(input),"output":absolute_display(&plan.output),"strategy":plan.strategy,"verification":verification}),
    )
}

fn verify_operation(
    context: &Context,
    input: &Path,
    plan: &OperationPlan,
) -> Result<Value, AppError> {
    match plan.value.get("operation").and_then(Value::as_str) {
        Some("extract_audio") => verify_audio_output(context, input, &plan.output),
        Some("thumbnail") => verify_thumbnail_output(context, &plan.output),
        Some("clip") => verify_clip_output(context, input, plan),
        _ => verify_value(context, input, &plan.output),
    }
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
        ("av1", vec!["libaom-av1", "libsvtav1", "av1_nvenc", "av1_qsv", "av1_amf"]),
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
    Ok(
        json!({"status":"success","ffmpeg":{"installed":version != "not installed","version":version},"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"hardware_acceleration":hardware_acceleration,"hardware_acceleration_list":hwaccels,"encoders":encoders}),
    )
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
    let result = run_program("ffmpeg", &refs, context.verbose)?;
    if context.verbose && !result.stderr.trim().is_empty() {
        eprintln!("{}", result.stderr.trim());
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
    let code = if stderr.contains("Unknown encoder") || stderr.contains("Encoder (.*) not found") {
        "ENCODER_UNAVAILABLE"
    } else if stderr.contains("Unknown decoder") {
        "DECODER_UNAVAILABLE"
    } else if stderr.contains("Cannot create compression session")
        || stderr.contains("No capable devices found")
        || stderr.contains("hardware encoder")
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
    let expected_duration = progress_duration_seconds(args);
    let mut command = ProcessCommand::new("ffmpeg");
    command
        .args(&refs)
        .args(["-progress", "pipe:2", "-nostats"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| process_start_error("ffmpeg", error))?;
    eprintln!("{}", json!({"event":"start","command":"ffmpeg","value":0.0}));
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
        stderr.push_str(trimmed);
        stderr.push('\n');
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
                    eprintln!(
                        "{}",
                        json!({"event":if is_end { "complete" } else { "progress" },"value":normalized,"out_time_ms":out_time_ms,"speed":speed.clone()})
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
        eprintln!("{}", json!({"event":"complete","value":1.0,"speed":speed}));
    }
    Ok(())
}

fn progress_duration_seconds(args: &[String]) -> Option<f64> {
    args.windows(2).find(|pair| pair[0] == "-t").and_then(|pair| parse_time_seconds(&pair[1]).ok())
}

fn decode_check(context: &Context, input: &Path) -> Result<(), AppError> {
    let refs = ["-v", "error", "-i", &input.to_string_lossy(), "-f", "null", "-"];
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
            if !parent.as_os_str().is_empty() && !parent.exists() {
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
    json!({"index":stream.get("index"),"codec":codec,"profile":stream.get("profile"),"width":stream.get("width"),"height":stream.get("height"),"fps":parse_ratio(stream.get("avg_frame_rate").and_then(Value::as_str).or_else(|| stream.get("r_frame_rate").and_then(Value::as_str))),"pixel_format":stream.get("pix_fmt"),"bit_depth":stream.get("bits_per_raw_sample").and_then(Value::as_str).and_then(|v| v.parse::<u8>().ok()).or_else(|| stream.get("bits_per_raw_sample").and_then(Value::as_u64).map(|v| v as u8)),"hdr":hdr_name(stream),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default": disposition_flag(stream, "default")})
}
fn normalize_audio(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"sample_rate":number_field(stream,"sample_rate").map(|v| v as u64),"channels":stream.get("channels"),"channel_layout":stream.get("channel_layout"),"bitrate":number_field(stream,"bit_rate").map(|v| v as u64),"language":stream.get("tags").and_then(|tags| tags.get("language")),"default":disposition_flag(stream,"default")})
}
fn normalize_subtitle(stream: &Value) -> Value {
    json!({"index":stream.get("index"),"codec":stream.get("codec_name"),"language":stream.get("tags").and_then(|tags| tags.get("language")),"forced":disposition_flag(stream,"forced"),"default":disposition_flag(stream,"default")})
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
        _ => format_name.to_string(),
    }
}

fn internal_container(input: &Path, format_name: &str) -> String {
    match input.extension().and_then(OsStr::to_str).map(|value| value.to_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "mp4".to_string(),
        Some("mkv") => "mkv".to_string(),
        Some("mov") => "mov".to_string(),
        Some("webm") => "webm".to_string(),
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
    if ["mp3", "aac", "m4a", "flac", "wav", "opus"].contains(&value.as_str()) {
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
        _ => "aac",
    }
}

fn audio_copy_compatible(codec: &str, format: &str) -> bool {
    match format {
        "m4a" | "aac" => codec == "aac",
        "mp3" => codec == "mp3",
        "flac" => codec == "flac",
        "wav" => codec.starts_with("pcm_") || codec == "pcm_s16le",
        "opus" => codec == "opus",
        _ => false,
    }
}

fn is_video_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["h264", "hevc", "mpeg4", "av1", "vp9"].contains(&codec),
        "webm" => ["vp8", "vp9", "av1"].contains(&codec),
        "mkv" | "avi" => true,
        _ => false,
    }
}
fn is_audio_compatible(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => ["aac", "mp3", "ac3", "eac3"].contains(&codec),
        "webm" => ["opus", "vorbis"].contains(&codec),
        "mkv" | "avi" => true,
        _ => false,
    }
}
fn preferred_codec(requested: &str, fallback: &str) -> String {
    if requested == "auto" || requested == "copy" {
        fallback.to_string()
    } else {
        requested.to_string()
    }
}
fn video_encode_args(
    codec: &str,
    quality: &str,
    hardware_encoder: Option<&str>,
) -> Result<Vec<String>, AppError> {
    if codec == "copy" {
        return Err(AppError::new(
            "UNSUPPORTED_CODEC",
            "Video codec `copy` cannot be used for a transcode plan.",
        ));
    }
    let codec = preferred_codec(codec, "h264");
    if let Some(encoder) = hardware_encoder {
        return Ok(vec!["-c:v".into(), encoder.into()]);
    }
    match codec.as_str() {
        "h264" => Ok(vec![
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            if quality == "balanced" { "23".into() } else { "20".into() },
        ]),
        "h265" | "hevc" => Ok(vec![
            "-c:v".into(),
            "libx265".into(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            "26".into(),
        ]),
        "av1" => Ok(vec![
            "-c:v".into(),
            "libaom-av1".into(),
            "-crf".into(),
            "30".into(),
            "-b:v".into(),
            "0".into(),
        ]),
        other => {
            Err(AppError::new("UNSUPPORTED_CODEC", format!("Unsupported video codec: {other}")))
        }
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
        other => {
            Err(AppError::new("UNSUPPORTED_CODEC", format!("Unsupported audio codec: {other}")))
        }
    }
}
fn subtitle_warnings(streams: &[Value], container: &str) -> Vec<String> {
    let subtitle_count = stream_count(streams, "subtitle");
    if subtitle_count == 0 {
        return Vec::new();
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
    } else {
        match container {
            "mp4" => "convert_to_mov_text",
            "webm" => "convert_to_webvtt",
            _ => "copy",
        }
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
    value
        .parse::<u32>()
        .map_err(|_| AppError::new("INVALID_ARGUMENT", format!("Invalid resolution: {value}")))
}
fn parse_thumbnail_time(value: &str, duration: Option<f64>) -> Result<String, AppError> {
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent
            .parse::<f64>()
            .map_err(|_| AppError::new("INVALID_ARGUMENT", "Invalid percentage for --at."))?;
        let duration = duration.ok_or_else(|| {
            AppError::new(
                "INVALID_MEDIA",
                "Percentage thumbnail position requires a known duration.",
            )
        })?;
        return Ok(format!("{:.3}", duration * percent / 100.0));
    }
    parse_time_seconds(value).map(|_| value.to_string())
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
                "mp4", "mkv", "mov", "webm", "avi", "m4v", "mts", "m2ts", "mp3", "wav", "flac",
                "m4a", "aac", "opus",
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
    fn wildcard_match_supports_globs() {
        assert!(wildcard_match("clip-01.mov", "*.mov"));
        assert!(wildcard_match("a1.mp4", "a?.mp4"));
        assert!(!wildcard_match("clip.wav", "*.mov"));
    }

    #[test]
    fn parses_agent_tool_request_and_aliases() {
        let request: ToolRequest = serde_json::from_str(
            r#"{"operation":"extract-audio","input":"in.mp4","format":"flac","dry_run":true,"verify_after_execute":false}"#,
        )
        .unwrap();
        assert_eq!(request.operation, "extract-audio");
        assert_eq!(normalize_operation("convert_media"), "convert");
        assert_eq!(normalize_operation("plan-media-operation"), "plan");
        assert_eq!(normalize_operation("create_thumbnail"), "thumbnail");
        assert_eq!(request.verify_after_execute, Some(false));
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
        let streams = vec![
            json!({"codec_type":"video"}),
            json!({"codec_type":"audio"}),
            json!({"codec_type":"subtitle"}),
            json!({"codec_type":"subtitle"}),
        ];
        assert_eq!(stream_count(&streams, "video"), 1);
        assert_eq!(stream_count(&streams, "subtitle"), 2);
        assert_eq!(subtitle_strategy("mp4", &streams), "convert_to_mov_text");
    }

    #[test]
    fn subtitle_mapping_does_not_duplicate_existing_full_map() {
        let streams = vec![json!({"codec_type":"subtitle"})];
        assert_eq!(subtitle_ffmpeg_args("mp4", &streams), ["-map", "0:s?", "-c:s", "mov_text"]);
        assert_eq!(subtitle_codec_args("mp4", &streams), ["-c:s", "mov_text"]);
    }
}
