//! Process launch abstractions.

use std::path::PathBuf;
use std::process::Command;

use crate::{Error, Result};

/// A process launch command that can be inspected or executed by consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchCommand {
    /// Executable path.
    pub executable: PathBuf,

    /// Command-line arguments.
    pub args: Vec<String>,

    /// Optional working directory.
    pub working_dir: Option<PathBuf>,
}

impl LaunchCommand {
    /// Creates a launch command.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let executable = executable.into();
        let working_dir = executable.parent().map(PathBuf::from);
        Self {
            executable,
            args: Vec::new(),
            working_dir,
        }
    }
}

/// Executes launch commands.
pub trait LaunchRunner {
    /// Runs the supplied command.
    fn run(&self, command: &LaunchCommand) -> Result<()>;
}

/// Launch runner backed by `std::process::Command`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessLaunchRunner;

impl LaunchRunner for ProcessLaunchRunner {
    fn run(&self, command: &LaunchCommand) -> Result<()> {
        let mut process = Command::new(&command.executable);
        process.args(&command.args);
        if let Some(working_dir) = &command.working_dir {
            process.current_dir(working_dir);
        }

        process.spawn().map(|_| ()).map_err(|source| Error::Io {
            path: command.executable.clone(),
            source,
        })
    }
}

/// Splits a simple command-line string into arguments.
pub fn split_arguments(command_line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in command_line.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}
