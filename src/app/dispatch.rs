use super::prelude::*;
use super::{
    audio::*, basic::*, batch::*, capabilities::*, convert::*, disc::*, edit::*, error::*,
    image::*, inspect::*, merge::*, model::*, presets::*, state::*, tool::*, verify::*,
};

pub(crate) fn dispatch(context: &Context, command: Command) -> Result<Value, AppError> {
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
