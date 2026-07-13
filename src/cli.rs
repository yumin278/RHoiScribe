//------------------------------------------------------------------------------------
// cli.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use std::{
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};

pub type SkillCliCommand = crate::skill::SkillCommand;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogFilterCliArgs {
    pub store_path: Option<String>,
    pub mod_root: Option<String>,
    pub tool_name: Option<String>,
    pub success: Option<bool>,
    pub since_unix_seconds: Option<u64>,
    pub until_unix_seconds: Option<u64>,
    pub text_query: Option<String>,
    pub pattern: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogExportCliArgs {
    pub output_path: String,
    pub filters: LogFilterCliArgs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Serve,
    Help,
    Version,
    PrintCommand,
    Logs(LogFilterCliArgs),
    ExportLogs(LogExportCliArgs),
    Skill(SkillCliCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

pub fn parse_args<I, S>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let rest = args
        .into_iter()
        .skip(1)
        .map(|argument| argument.as_ref().to_string())
        .collect::<Vec<_>>();

    parse_rest(&rest)
}

fn parse_rest(rest: &[String]) -> Result<CliCommand, CliError> {
    if let Some(command) = parse_log_command(rest)? {
        return Ok(command);
    }
    match rest {
        [] => Ok(CliCommand::Serve),
        [flag] => parse_single_flag(flag),
        [flag, skill_args @ ..] if flag == "--skill" => parse_skill_args(skill_args),
        [argument, ..] => Err(unknown_argument(argument)),
    }
}

fn parse_log_command(rest: &[String]) -> Result<Option<CliCommand>, CliError> {
    if let Some(command) = parse_legacy_log_command(rest) {
        return Ok(Some(command));
    }
    if rest.first().is_some_and(|argument| argument == "logs") {
        return parse_named_log_command(&rest[1..]).map(Some);
    }
    Ok(None)
}

fn parse_legacy_log_command(rest: &[String]) -> Option<CliCommand> {
    match rest.first().map(String::as_str) {
        Some("--logs") => parse_legacy_log_query(&rest[1..]),
        Some("--export-logs") => parse_legacy_log_export(&rest[1..]),
        _ => None,
    }
}

fn parse_legacy_log_query(args: &[String]) -> Option<CliCommand> {
    match args {
        [] => Some(CliCommand::Logs(LogFilterCliArgs::default())),
        [pattern] => Some(CliCommand::Logs(LogFilterCliArgs {
            pattern: Some(pattern.clone()),
            ..LogFilterCliArgs::default()
        })),
        _ => None,
    }
}

fn parse_legacy_log_export(args: &[String]) -> Option<CliCommand> {
    match args {
        [output_path] => Some(legacy_export_command(output_path, None)),
        [output_path, pattern] => Some(legacy_export_command(output_path, Some(pattern))),
        _ => None,
    }
}

fn legacy_export_command(output_path: &str, pattern: Option<&String>) -> CliCommand {
    CliCommand::ExportLogs(LogExportCliArgs {
        output_path: output_path.to_string(),
        filters: LogFilterCliArgs {
            pattern: pattern.cloned(),
            ..LogFilterCliArgs::default()
        },
    })
}

fn parse_named_log_command(args: &[String]) -> Result<CliCommand, CliError> {
    match args {
        [command, flags @ ..] if command == "query" => {
            parse_log_flags(flags, false).map(|parsed| CliCommand::Logs(parsed.filters))
        }
        [command, flags @ ..] if command == "export" => parse_named_log_export(flags),
        [command, ..] => Err(unknown_argument(command)),
        [] => Err(CliError::message(
            "missing logs subcommand `query` or `export`",
        )),
    }
}

#[derive(Default)]
struct ParsedLogFlags {
    filters: LogFilterCliArgs,
    output_path: Option<String>,
}

fn parse_named_log_export(flags: &[String]) -> Result<CliCommand, CliError> {
    let parsed = parse_log_flags(flags, true)?;
    let output_path = parsed
        .output_path
        .ok_or_else(|| CliError::message("logs export requires `--output PATH`"))?;
    Ok(CliCommand::ExportLogs(LogExportCliArgs {
        output_path,
        filters: parsed.filters,
    }))
}

fn parse_log_flags(args: &[String], allow_output: bool) -> Result<ParsedLogFlags, CliError> {
    let mut parsed = ParsedLogFlags::default();
    let mut index = 0;
    while index < args.len() {
        index += parse_log_flag(args, index, allow_output, &mut parsed)?;
    }
    Ok(parsed)
}

fn parse_log_flag(
    args: &[String],
    index: usize,
    allow_output: bool,
    parsed: &mut ParsedLogFlags,
) -> Result<usize, CliError> {
    let flag = &args[index];
    if !is_log_flag(flag) {
        return Err(unknown_argument(flag));
    }
    let value = args
        .get(index + 1)
        .ok_or_else(|| CliError::message(format!("missing value for `{flag}`")))?;
    if is_log_flag(value) {
        return Err(CliError::message(format!("missing value for `{flag}`")));
    }
    apply_log_flag(flag, value, allow_output, parsed)?;
    Ok(2)
}

fn is_log_flag(flag: &str) -> bool {
    matches!(
        flag,
        "--store-path"
            | "--mod-root"
            | "--tool-name"
            | "--success"
            | "--since"
            | "--until"
            | "--text-query"
            | "--pattern"
            | "--limit"
            | "--output"
    )
}

fn apply_log_flag(
    flag: &str,
    value: &str,
    allow_output: bool,
    parsed: &mut ParsedLogFlags,
) -> Result<(), CliError> {
    if apply_text_log_flag(flag, value, &mut parsed.filters)? {
        return Ok(());
    }
    if apply_numeric_log_flag(flag, value, &mut parsed.filters)? {
        return Ok(());
    }
    apply_special_log_flag(flag, value, allow_output, parsed)
}

fn apply_text_log_flag(
    flag: &str,
    value: &str,
    filters: &mut LogFilterCliArgs,
) -> Result<bool, CliError> {
    let slot = match flag {
        "--store-path" => &mut filters.store_path,
        "--mod-root" => &mut filters.mod_root,
        "--tool-name" => &mut filters.tool_name,
        "--text-query" => &mut filters.text_query,
        "--pattern" => &mut filters.pattern,
        _ => return Ok(false),
    };
    set_once(slot, value.to_string(), flag)?;
    Ok(true)
}

fn apply_numeric_log_flag(
    flag: &str,
    value: &str,
    filters: &mut LogFilterCliArgs,
) -> Result<bool, CliError> {
    match flag {
        "--since" => set_parsed(&mut filters.since_unix_seconds, value, flag),
        "--until" => set_parsed(&mut filters.until_unix_seconds, value, flag),
        "--limit" => set_parsed(&mut filters.limit, value, flag),
        _ => return Ok(false),
    }?;
    Ok(true)
}

fn apply_special_log_flag(
    flag: &str,
    value: &str,
    allow_output: bool,
    parsed: &mut ParsedLogFlags,
) -> Result<(), CliError> {
    match flag {
        "--success" => set_bool(&mut parsed.filters.success, value, flag),
        "--output" if allow_output => set_once(&mut parsed.output_path, value.to_string(), flag),
        "--output" => Err(CliError::message(
            "`--output` is only valid for `logs export`",
        )),
        _ => Err(unknown_argument(flag)),
    }
}

fn set_parsed<T>(slot: &mut Option<T>, value: &str, flag: &str) -> Result<(), CliError>
where
    T: std::str::FromStr,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| CliError::message(format!("invalid value `{value}` for `{flag}`")))?;
    set_once(slot, parsed, flag)
}

fn set_bool(slot: &mut Option<bool>, value: &str, flag: &str) -> Result<(), CliError> {
    let parsed = match value {
        "true" => true,
        "false" => false,
        _ => {
            return Err(CliError::message(format!(
                "invalid value `{value}` for `{flag}`; expected true or false"
            )));
        }
    };
    set_once(slot, parsed, flag)
}

fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(CliError::message(format!("duplicate flag `{flag}`")));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_single_flag(flag: &str) -> Result<CliCommand, CliError> {
    match flag {
        "--help" | "-h" => Ok(CliCommand::Help),
        "--version" | "-V" => Ok(CliCommand::Version),
        "--print-command" | "--mcp-command" => Ok(CliCommand::PrintCommand),
        argument => Err(unknown_argument(argument)),
    }
}

fn parse_skill_args(args: &[String]) -> Result<CliCommand, CliError> {
    match args {
        [command] => parse_skill_command(command, None, None),
        [command, value] => parse_skill_command(command, Some(value), None),
        [command, value, arguments_json] => {
            parse_skill_command(command, Some(value), Some(arguments_json))
        }
        [argument, ..] => Err(unknown_skill_argument(argument)),
        [] => Err(unknown_skill_argument("--skill")),
    }
}

fn parse_skill_command(
    command: &str,
    value: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    if let Some(command) = fixed_skill_command(command) {
        return Ok(CliCommand::Skill(command));
    }

    match command {
        "read-resource" => skill_resource_command(command, value),
        "get-prompt" => skill_prompt_command(command, value, arguments_json),
        "call-tool" => skill_tool_command(command, value, arguments_json),
        argument => Err(unknown_argument(argument)),
    }
}

fn fixed_skill_command(command: &str) -> Option<SkillCliCommand> {
    match command {
        "list-tools" => Some(SkillCliCommand::ListTools),
        "list-resources" => Some(SkillCliCommand::ListResources),
        "list-prompts" => Some(SkillCliCommand::ListPrompts),
        _ => None,
    }
}

fn skill_resource_command(command: &str, uri: Option<&String>) -> Result<CliCommand, CliError> {
    uri.map(|uri| CliCommand::Skill(SkillCliCommand::ReadResource { uri: uri.clone() }))
        .ok_or_else(|| unknown_skill_argument(command))
}

fn skill_prompt_command(
    command: &str,
    name: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    name.map(|name| {
        CliCommand::Skill(SkillCliCommand::GetPrompt {
            name: name.clone(),
            arguments_json: arguments_json.cloned().unwrap_or_else(|| "{}".to_string()),
        })
    })
    .ok_or_else(|| unknown_skill_argument(command))
}

fn skill_tool_command(
    command: &str,
    name: Option<&String>,
    arguments_json: Option<&String>,
) -> Result<CliCommand, CliError> {
    name.map(|name| {
        CliCommand::Skill(SkillCliCommand::CallTool {
            name: name.clone(),
            arguments_json: arguments_json.cloned().unwrap_or_else(|| "{}".to_string()),
        })
    })
    .ok_or_else(|| unknown_skill_argument(command))
}

fn unknown_skill_argument(argument: &str) -> CliError {
    unknown_argument(argument)
}

fn unknown_argument(argument: &str) -> CliError {
    CliError::message(format!("unknown argument `{argument}`"))
}

pub fn version_text() -> String {
    format!("rhoiscribe {}", env!("CARGO_PKG_VERSION"))
}

pub fn command_path() -> io::Result<PathBuf> {
    std::env::current_exe()
}

pub fn command_path_for_mcp_json() -> io::Result<String> {
    command_path().map(|path| path_for_mcp_json(&path))
}

pub fn path_for_mcp_json(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

pub fn help_text() -> &'static str {
    "RHoiScribe - local MCP server for HOI4 Modding agents\n\n\
Usage:\n\
  rhoiscribe                  Run the MCP server over stdio\n\
  rhoiscribe --print-command  Print the absolute command path for MCP config\n\
  rhoiscribe --mcp-command    Alias for --print-command\n\
  rhoiscribe --logs [REGEX]   Print recent tool logs as JSON\n\
  rhoiscribe --export-logs <PATH> [REGEX]\n\
                              Export matching tool logs as JSON\n\
  rhoiscribe logs query [--store-path P] [--mod-root P] [--tool-name N]\n\
                       [--success true|false] [--since UNIX] [--until UNIX]\n\
                       [--text-query Q] [--pattern R] [--limit N]\n\
  rhoiscribe logs export --output P [same filters as logs query]\n\
  rhoiscribe --skill list-tools\n\
  rhoiscribe --skill list-resources\n\
  rhoiscribe --skill list-prompts\n\
  rhoiscribe --skill read-resource <URI>\n\
  rhoiscribe --skill get-prompt <NAME> <JSON_ARGUMENTS>\n\
  rhoiscribe --skill call-tool <NAME> <JSON_ARGUMENTS>\n\
  rhoiscribe --help           Show this help text\n\
  rhoiscribe --version        Show version information\n\n\
MCP clients should launch this binary as a local stdio server. Skill clients can use --skill commands for direct JSON output without MCP setup.\n"
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl CliError {
    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Error for CliError {}
