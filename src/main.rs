use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const DEFAULT_AUDIO_BITRATE: &str = "256k";

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
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC", default_value = "auto")]
    video_codec: String,
    #[arg(long, value_name = "CODEC", default_value = "auto")]
    audio_codec: String,
}

#[derive(Args, Debug, Clone)]
struct ConvertArgs {
    input: PathBuf,
    #[arg(long, help = "Target container, for example mp4 or mkv")]
    to: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_name = "CODEC", default_value = "auto")]
    video_codec: String,
    #[arg(long, value_name = "CODEC", default_value = "auto")]
    audio_codec: String,
    #[arg(long, default_value = "auto", value_enum)]
    hardware: HardwareMode,
}

#[derive(Args, Debug, Clone)]
struct CompressArgs {
    input: PathBuf,
    #[arg(long, default_value = "balanced", value_enum)]
    quality: Quality,
    #[arg(long, help = "Target output size, e.g. 500MB or 1.5GB")]
    target_size: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, default_value = "auto", value_enum)]
    hardware: HardwareMode,
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

fn main() {
    let cli = Cli::parse();
    let tool_mode = matches!(&cli.command, Command::Tool(_));
    let context = Context {
        json: cli.json || tool_mode,
        dry_run: cli.dry_run,
        overwrite: cli.overwrite,
        verbose: cli.verbose,
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
    let config = load_config()?;
    let mut tool_context = context.clone();
    tool_context.json = true;
    if let Some(dry_run) = request.dry_run {
        tool_context.dry_run = dry_run;
    }
    // Config cannot enable overwrite implicitly: only the request or an explicit
    // CLI flag may relax the core safety policy.
    tool_context.overwrite = context.overwrite || request.overwrite.unwrap_or(false);
    let default_video_codec = config
        .video
        .as_ref()
        .and_then(|value| value.preferred_codec.clone())
        .unwrap_or_else(|| "auto".to_string());
    let default_audio_codec = config
        .audio
        .as_ref()
        .and_then(|value| value.preferred_codec.clone())
        .unwrap_or_else(|| "auto".to_string());
    let video_codec = request.video_codec.unwrap_or(default_video_codec);
    let audio_codec = request.audio_codec.unwrap_or(default_audio_codec);
    let quality = parse_quality_name(
        request
            .quality
            .or_else(|| config.default_quality.clone())
            .unwrap_or_else(|| "balanced".to_string()),
    )?;
    let hardware = parse_hardware_name(
        request.hardware.or_else(|| config.hardware.clone()).unwrap_or_else(|| "auto".to_string()),
    )?;
    let operation = request.operation.to_lowercase().replace('-', "_");
    let input = || required_input(request.input.clone());
    match operation.as_str() {
        "inspect" => dispatch(&tool_context, Command::Inspect(InputArgs { input: input()? })),
        "plan" => dispatch(
            &tool_context,
            Command::Plan(PlanArgs {
                input: input()?,
                to: request.output_format.clone(),
                output: request.output.clone().map(PathBuf::from),
                video_codec,
                audio_codec,
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
            "Use inspect, plan, convert, compress, resize, clip, extract_audio, thumbnail, batch, verify, capabilities, or ffmpeg.",
        ])),
    }
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
    let plan = build_convert_plan(
        context,
        &args.input,
        args.to.as_deref(),
        args.output.as_deref(),
        &args.video_codec,
        &args.audio_codec,
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
        &args.video_codec,
        &args.audio_codec,
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

fn build_convert_plan(
    context: &Context,
    input: &Path,
    to: Option<&str>,
    output: Option<&Path>,
    video_codec: &str,
    audio_codec: &str,
) -> Result<OperationPlan, AppError> {
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
    let video_compatible =
        video_codec == "auto" && is_video_compatible(&target_container, &source_video_codec);
    let audio_compatible =
        audio_codec == "auto" && is_audio_compatible(&target_container, &source_audio_codec);
    let video_action = if video_compatible { "copy" } else { "transcode" };
    let audio_action = if audio_compatible { "copy" } else { "transcode" };
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
        ffmpeg_args.extend(video_encode_args(video_codec, "balanced")?);
    }
    if audio_action == "copy" {
        ffmpeg_args.extend(["-c:a".to_string(), "copy".to_string()]);
    } else {
        ffmpeg_args.extend(audio_encode_args(audio_codec, DEFAULT_AUDIO_BITRATE)?);
    }
    ffmpeg_args.extend([
        "-map_metadata".to_string(),
        "0".to_string(),
        "-map_chapters".to_string(),
        "0".to_string(),
    ]);
    let plan = json!({
        "status": "success",
        "operation": "convert",
        "input": absolute_display(input),
        "output": absolute_display(&target_path),
        "strategy": strategy,
        "video": {"action": video_action, "codec": if video_action == "copy" { source_video_codec.clone() } else { preferred_codec(video_codec, "h264") }},
        "audio": {"action": audio_action, "from": source_audio_codec, "to": if audio_action == "copy" { Value::Null } else { json!(preferred_codec(audio_codec, "aac")) }},
        "subtitle": {"action": "preserve_when_compatible"},
        "metadata": {"action": "preserve"},
        "hardware": {"requested": "auto", "selected": "cpu"},
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
    let verification = verify_value(context, input, &plan.output)?;
    let verification_valid = verification.get("valid").and_then(Value::as_bool).unwrap_or(false);
    if !verification_valid {
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
    let probe = probe_media(&args.input, context.verbose)?;
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let video = first_stream(
        &probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default(),
        "video",
    )
    .ok_or_else(|| AppError::new("INVALID_MEDIA", "No video stream was found."))?;
    let duration = probe.duration_seconds.unwrap_or(0.0);
    let mut ffmpeg_args = vec![
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a?".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
    ];
    let mut notes = vec![format!(
        "Compressing {} video with the {:?} quality preset.",
        video.get("codec_name").and_then(Value::as_str).unwrap_or("unknown"),
        args.quality
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
            let crf = match args.quality {
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
    ffmpeg_args.extend([
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        DEFAULT_AUDIO_BITRATE.to_string(),
    ]);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"compress","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"transcode","quality":args.quality,"quality_loss":"video_and_audio","reason":notes,"hardware":{"requested":format_hardware(args.hardware),"selected":"cpu"},"ffmpeg_args":ffmpeg_args}),
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
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let ffmpeg_args = vec![
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
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"resize","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"video_transcode","filter":filter,"preserve_aspect_ratio":true,"even_dimensions":true,"quality_loss":"video_only","ffmpeg_args":ffmpeg_args}),
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
    let output = resolve_output(context, &args.input, args.output.as_deref(), "mp4")?;
    let mut ffmpeg_args = vec![
        "-ss".to_string(),
        args.start.clone(),
        "-i".to_string(),
        args.input.to_string_lossy().to_string(),
    ];
    if let Some(duration) = &args.duration {
        ffmpeg_args.extend(["-t".to_string(), duration.clone()]);
    } else if let Some(end) = &args.end {
        ffmpeg_args.extend(["-to".to_string(), end.clone()]);
    }
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
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"clip","input":absolute_display(&args.input),"output":absolute_display(&output),"strategy":"precise_transcode","start":args.start,"duration":args.duration,"end":args.end,"quality_loss":"video_and_audio","ffmpeg_args":ffmpeg_args}),
        output,
        args: ffmpeg_args,
        strategy: "precise_transcode".to_string(),
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
    let codec = audio_encode_args(&format, DEFAULT_AUDIO_BITRATE)?;
    let streams = probe_media(&args.input, context.verbose)?
        .raw
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if first_stream(&streams, "audio").is_none() {
        return Err(AppError::new("INVALID_MEDIA", "No audio stream was found."));
    }
    let mut ffmpeg_args = vec!["-map".to_string(), "0:a:0".to_string(), "-vn".to_string()];
    ffmpeg_args.extend(codec);
    let plan = OperationPlan {
        value: json!({"status":"success","operation":"extract_audio","input":absolute_display(&args.input),"output":absolute_display(&output),"format":format,"strategy":"transcode","ffmpeg_args":ffmpeg_args}),
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
    let verification = verify_operation(context, input, plan)?;
    if !verification.get("valid").and_then(Value::as_bool).unwrap_or(false) {
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
    Ok(
        json!({"status":"success","valid":audio_present && !decode_errors,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"audio_present":audio_present,"decode_errors":decode_errors},"warnings":[]}),
    )
}

fn verify_thumbnail_output(context: &Context, output: &Path) -> Result<Value, AppError> {
    let probe = probe_media(output, context.verbose)?;
    let streams = probe.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video_present = first_stream(&streams, "video").is_some();
    let decode_errors = decode_check(context, output).is_err();
    Ok(
        json!({"status":"success","valid":video_present && !decode_errors,"output":absolute_display(output),"checks":{"readable":true,"frame_present":video_present,"decode_errors":decode_errors},"warnings":[]}),
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
        json!({"status":"success","valid":video_present && !decode_errors && duration_match,"input":absolute_display(input),"output":absolute_display(&plan.output),"checks":{"readable":true,"video_present":video_present,"duration_match":duration_match,"decode_errors":decode_errors},"warnings":[]}),
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
            video_codec: "auto".to_string(),
            audio_codec: "auto".to_string(),
            hardware: HardwareMode::Auto,
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
    let value = verify_value(context, input, output)?;
    if !value.get("valid").and_then(Value::as_bool).unwrap_or(false) {
        return Err(
            AppError::new("VERIFY_FAILED", "One or more output checks failed.").with_details(value)
        );
    }
    Ok(value)
}

fn verify_value(context: &Context, input: &Path, output: &Path) -> Result<Value, AppError> {
    let source = probe_media(input, context.verbose)?;
    let rendered = probe_media(output, context.verbose)?;
    let source_streams =
        source.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let output_streams =
        rendered.raw.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let duration_match = match (source.duration_seconds, rendered.duration_seconds) {
        (Some(a), Some(b)) => (a - b).abs() <= 1.5,
        _ => true,
    };
    let video_present = first_stream(&output_streams, "video").is_some();
    let audio_present = first_stream(&output_streams, "audio").is_some();
    let source_video = first_stream(&source_streams, "video");
    let output_video = first_stream(&output_streams, "video");
    let resolution_match = match (source_video, output_video) {
        (Some(a), Some(b)) => {
            a.get("width") == b.get("width") && a.get("height") == b.get("height")
        }
        _ => true,
    };
    let decode_errors = decode_check(context, output).is_err();
    let valid = duration_match && video_present && !decode_errors;
    Ok(
        json!({"status":"success","valid":valid,"input":absolute_display(input),"output":absolute_display(output),"checks":{"readable":true,"duration_match":duration_match,"video_present":video_present,"audio_present":audio_present,"resolution_match":resolution_match,"decode_errors":decode_errors},"warnings": if audio_present { json!([]) } else { json!(["Output has no audio stream."]) }}),
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
        ("h264", vec!["libx264", "h264_videotoolbox", "h264_nvenc"]),
        ("hevc", vec!["libx265", "hevc_videotoolbox", "hevc_nvenc"]),
        ("av1", vec!["libaom-av1", "libsvtav1", "av1_nvenc"]),
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
    Ok(
        json!({"status":"success","ffmpeg":{"installed":version != "not installed","version":version},"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"hardware_acceleration":hwaccels,"encoders":encoders}),
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
    let output = ProcessCommand::new(program).args(args).output().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            AppError::new("FFMPEG_NOT_FOUND", format!("{program} was not found on PATH."))
                .with_suggestions(&[
                    "Install FFmpeg and FFprobe.",
                    "Run media capabilities to inspect the current environment.",
                ])
        } else {
            AppError::from_io("FFMPEG_FAILED", format!("Could not start {program}."), error)
        }
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if verbose && !stderr.trim().is_empty() {
        eprintln!("[{program}] {}", stderr.trim());
    }
    if !output.status.success() {
        let code =
            if stderr.contains("Unknown encoder") || stderr.contains("Encoder (.*) not found") {
                "ENCODER_UNAVAILABLE"
            } else if stderr.contains("Unknown decoder") {
                "DECODER_UNAVAILABLE"
            } else {
                "FFMPEG_FAILED"
            };
        return Err(AppError::new(
            code,
            format!("{program} exited with status {}.", output.status),
        )
        .with_details(json!({"command":program,"arguments":args,"stderr":stderr})));
    }
    Ok(ProcessResult { stdout, stderr })
}

fn run_ffmpeg(context: &Context, args: &[String]) -> Result<(), AppError> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let result = run_program("ffmpeg", &refs, context.verbose)?;
    if context.verbose && !result.stderr.trim().is_empty() {
        eprintln!("{}", result.stderr.trim());
    }
    Ok(())
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
fn video_encode_args(codec: &str, quality: &str) -> Result<Vec<String>, AppError> {
    let codec = preferred_codec(codec, "h264");
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
fn audio_encode_args(codec: &str, bitrate: &str) -> Result<Vec<String>, AppError> {
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
    if container == "mp4"
        && streams.iter().any(|stream| {
            stream.get("codec_type").and_then(Value::as_str) == Some("subtitle")
                && stream.get("codec_name").and_then(Value::as_str) == Some("subrip")
        })
    {
        vec!["SubRip subtitles may require conversion for MP4 compatibility.".to_string()]
    } else {
        Vec::new()
    }
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
            r#"{"operation":"extract-audio","input":"in.mp4","format":"flac","dry_run":true}"#,
        )
        .unwrap();
        assert_eq!(request.operation, "extract-audio");
        assert_eq!(
            parse_quality_name("very_high".to_string()).unwrap() as u8,
            Quality::VeryHigh as u8
        );
        assert!(matches!(parse_hardware_name("gpu".to_string()), Ok(HardwareMode::Gpu)));
    }
}
