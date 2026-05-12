use std::ffi::OsStr;
use std::process::{Command, Output};

use crate::AppError;

/// Execute a real command, capturing stdout and stderr.
///
/// When `shell_mode` is true, the command is run through the platform shell so
/// pipes, redirects, and shell substitutions work as expected.
pub fn execute_command(command_args: &[String], shell_mode: bool) -> Result<Output, AppError> {
    execute_command_with_env(command_args, shell_mode, std::iter::empty::<(&str, &str)>())
}

pub fn execute_command_with_env<'a, I, K, V>(
    command_args: &[String],
    shell_mode: bool,
    envs: I,
) -> Result<Output, AppError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    if command_args.is_empty() {
        return Err(AppError::new("no command provided"));
    }
    let env_pairs: Vec<(K, V)> = envs.into_iter().collect();
    if shell_mode {
        let command_text = command_args.join(" ");
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(command_text);
            command
        } else {
            let mut command = Command::new("sh");
            command.arg("-c").arg(command_text);
            command
        };
        for (key, value) in env_pairs {
            command.env(key, value);
        }
        Ok(command.output()?)
    } else {
        let mut command = Command::new(&command_args[0]);
        if command_args.len() > 1 {
            command.args(&command_args[1..]);
        }
        for (key, value) in env_pairs {
            command.env(key, value);
        }
        Ok(command.output()?)
    }
}
