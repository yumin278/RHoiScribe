use std::{error::Error, fmt, io, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Serve,
    Help,
    Version,
    PrintCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    argument: String,
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

    match rest.as_slice() {
        [] => Ok(CliCommand::Serve),
        [flag] if flag == "--help" || flag == "-h" => Ok(CliCommand::Help),
        [flag] if flag == "--version" || flag == "-V" => Ok(CliCommand::Version),
        [flag] if flag == "--print-command" || flag == "--mcp-command" => {
            Ok(CliCommand::PrintCommand)
        }
        [argument, ..] => Err(CliError {
            argument: argument.clone(),
        }),
    }
}

pub fn version_text() -> String {
    format!("rhoiscribe {}", env!("CARGO_PKG_VERSION"))
}

pub fn command_path() -> io::Result<PathBuf> {
    std::env::current_exe()
}

pub fn help_text() -> &'static str {
    "RHoiScribe - local MCP server for HOI4 Modding agents\n\n\
Usage:\n\
  rhoiscribe                  Run the MCP server over stdio\n\
  rhoiscribe --print-command  Print the absolute command path for MCP config\n\
  rhoiscribe --mcp-command    Alias for --print-command\n\
  rhoiscribe --help           Show this help text\n\
  rhoiscribe --version        Show version information\n\n\
MCP clients should launch this binary as a local stdio server.\n"
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown argument `{}`", self.argument)
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::{CliCommand, command_path, help_text, parse_args};

    #[test]
    fn parses_print_command_flags() {
        let command =
            parse_args(["rhoiscribe", "--print-command"]).expect("print command should parse");
        let alias = parse_args(["rhoiscribe", "--mcp-command"]).expect("alias should parse");

        assert_eq!(command, CliCommand::PrintCommand);
        assert_eq!(alias, CliCommand::PrintCommand);
    }

    #[test]
    fn help_mentions_print_command() {
        assert!(help_text().contains("--print-command"));
    }

    #[test]
    fn command_path_is_absolute() {
        let path = command_path().expect("current executable path should be available");

        assert!(path.is_absolute());
    }
}
