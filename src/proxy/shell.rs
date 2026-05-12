use std::process::{Command, Output};

use crate::AppError;

/// Execute a real command, capturing stdout and stderr.
///
/// When `shell_mode` is true, the command is run through the platform shell so
/// pipes, redirects, and shell substitutions work as expected.
pub fn execute_command(command_args: &[String], shell_mode: bool) -> Result<Output, AppError> {
    if command_args.is_empty() {
        return Err(AppError::new("no command provided"));
    }
    if shell_mode {
        let command_text = command_args.join(" ");
        let output = if cfg!(windows) {
            Command::new("cmd").arg("/C").arg(command_text).output()?
        } else {
            Command::new("sh").arg("-c").arg(command_text).output()?
        };
        Ok(output)
    } else {
        let mut command = Command::new(&command_args[0]);
        if command_args.len() > 1 {
            command.args(&command_args[1..]);
        }
        Ok(command.output()?)
    }
}
