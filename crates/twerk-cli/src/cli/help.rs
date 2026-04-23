//! Help rendering and JSON payload builders.

use clap::CommandFactory;
use serde_json::{json, Value};
use std::ffi::OsString;

use crate::commands::Cli;
use crate::error::CliError;

use super::{os_string_eq, GIT_COMMIT, VERSION};

/// Help variant indicating the level of help requested
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HelpVariant {
    /// No help requested
    None,
    /// Short help via `-h` or `--help` (without Examples)
    Short,
    /// Long help via `--help --long` (with Examples)
    Long,
}

/// Detect the help variant from command line arguments
pub(super) fn detect_help_variant(args: &[OsString]) -> HelpVariant {
    let has_long = args.iter().any(|arg| os_string_eq(arg, "--long"));
    let has_help = args
        .iter()
        .any(|arg| os_string_eq(arg, "--help") || os_string_eq(arg, "-h"));

    if has_long {
        HelpVariant::Long
    } else if has_help {
        HelpVariant::Short
    } else {
        HelpVariant::None
    }
}

pub(super) fn render_help_for_path(
    path: &[String],
    variant: HelpVariant,
) -> Result<String, CliError> {
    match variant {
        HelpVariant::None => Err(CliError::MissingArgument("no help requested".to_string())),
        HelpVariant::Short | HelpVariant::Long => {
            let mut command = Cli::command();
            let mut buffer = Vec::new();
            let target = path.iter().try_fold(&mut command, |current, segment| {
                current.find_subcommand_mut(segment).ok_or_else(|| {
                    CliError::MissingArgument(format!("unknown help target: {segment}"))
                })
            })?;

            match variant {
                HelpVariant::Short => {
                    target.write_help(&mut buffer).map_err(CliError::Io)?;
                }
                HelpVariant::Long => {
                    target.write_long_help(&mut buffer).map_err(CliError::Io)?;
                }
                HelpVariant::None => {
                    return Ok(String::new());
                }
            }
            String::from_utf8(buffer).map_err(|error| CliError::Config(error.to_string()))
        }
    }
}

/// Write help content to stdout
pub(super) fn write_help_to_stdout(content: &str) {
    print!("{content}");
}

#[allow(dead_code)]
pub(super) fn render_top_level_help() -> Result<String, CliError> {
    render_help_for_path(&[], HelpVariant::Short)
}

pub(super) fn print_json(value: &Value) {
    println!("{value}");
}

pub(super) fn json_help_payload(content: String) -> Value {
    json!({
        "type": "help",
        "version": VERSION,
        "commit": GIT_COMMIT,
        "content": content,
    })
}

pub(super) fn json_version_payload(content: String) -> Value {
    json!({
        "type": "version",
        "version": VERSION,
        "commit": GIT_COMMIT,
        "content": content,
    })
}

pub(super) fn json_success_payload(command: &str, data: Value) -> Value {
    json!({
        "type": "success",
        "command": command,
        "exit_code": 0,
        "version": VERSION,
        "commit": GIT_COMMIT,
        "data": data,
    })
}

pub(super) fn json_error_payload(kind: &str, message: String, content: Option<String>) -> Value {
    json!({
        "type": "error",
        "kind": kind,
        "version": VERSION,
        "commit": GIT_COMMIT,
        "message": message,
        "content": content,
    })
}

pub(super) const fn clap_error_kind_name(kind: clap::error::ErrorKind) -> &'static str {
    match kind {
        clap::error::ErrorKind::DisplayHelp => "help",
        clap::error::ErrorKind::DisplayVersion => "version",
        clap::error::ErrorKind::InvalidValue => "invalid_value",
        clap::error::ErrorKind::InvalidSubcommand => "invalid_subcommand",
        clap::error::ErrorKind::MissingRequiredArgument => "missing_required_argument",
        clap::error::ErrorKind::UnknownArgument => "unknown_argument",
        _ => "parse_error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn render_top_level_help_contains_usage() {
        let help = render_top_level_help().expect("help should render");

        assert!(help.contains("Usage:"));
    }
}
