use super::model::*;
use super::prelude::*;

pub(crate) fn format_hardware(mode: HardwareMode) -> &'static str {
    match mode {
        HardwareMode::Auto => "auto",
        HardwareMode::Cpu => "cpu",
        HardwareMode::Gpu => "gpu",
    }
}
pub(crate) fn print_json(value: &Value) {
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    serde_json::to_writer_pretty(&mut stdout, value).expect("stdout should be writable");
    writeln!(stdout).expect("stdout should be writable");
}
pub(crate) fn print_human(value: &Value) {
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
