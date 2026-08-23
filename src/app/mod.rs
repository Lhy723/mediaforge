use dispatch::dispatch;
use output::{print_human, print_json};
use prelude::*;

mod audio;
mod basic;
mod batch;
mod capabilities;
mod codec;
mod config;
mod convert;
mod disc;
mod dispatch;
mod edit;
mod error;
mod execution;
mod format;
mod hardware;
mod image;
mod inspect;
mod merge;
mod metadata;
mod model;
mod output;
mod parse;
mod paths;
mod prelude;
mod presets;
mod process;
mod state;
mod tool;
mod verify;

pub(crate) use config::*;
pub(crate) use model::*;
pub(crate) use state::*;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use {
    audio::*, basic::*, batch::*, capabilities::*, convert::*, disc::*, dispatch::*, edit::*,
    error::*, execution::*, format::*, hardware::*, image::*, inspect::*, merge::*, output::*,
    parse::*, paths::*, presets::*, process::*, tool::*, verify::*,
};

pub(crate) fn run() {
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

#[cfg(test)]
mod tests;
