use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::AppError;

use super::command_ast::{build_ast, CommandAst, CommandKind};
use super::raw_store::ProxyTarget;
use super::run::{prevent_recursion_active, run_proxy, RunOptions};
use super::safety::{is_destructive, is_interactive_command};

pub const DEFAULT_SHIM_NAMES: &[&str] = &[
    "cargo",
    "pytest",
    "python",
    "go",
    "npm",
    "pnpm",
    "yarn",
    "bun",
    "npx",
    "jest",
    "vitest",
    "git",
    "gh",
    "rg",
    "grep",
    "find",
    "tree",
    "ls",
    "cat",
    "sed",
    "head",
    "tail",
    "docker",
    "docker-compose",
    "kubectl",
    "terraform",
    "helm",
    "make",
    "mvn",
    "gradle",
    "gradlew",
    "dotnet",
    "tsc",
    "eslint",
    "ruff",
    "mypy",
    "pyright",
];

#[derive(Debug, Clone)]
pub struct ShimInstallOptions {
    pub channel: String,
    pub target: ProxyTarget,
}

#[derive(Debug, Clone)]
pub struct ShellInitOptions {
    pub channel: String,
    pub target: ProxyTarget,
    pub shell: String,
}

#[derive(Debug, Clone)]
pub struct DispatchOptions {
    pub channel: String,
    pub target: ProxyTarget,
    pub shim_name: String,
    pub args: Vec<String>,
}

pub fn shim_dir(channel: &str, target: ProxyTarget) -> Result<PathBuf, AppError> {
    if let Ok(value) = env::var("WF_CORE_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value).join("shims"));
        }
    }
    match target {
        ProxyTarget::Windsurf => Ok(crate::channel_home(channel)?.join("wf-core").join("shims")),
        ProxyTarget::Devin => Ok(crate::devin_home()?.join("wf-core").join("shims")),
    }
}

pub fn install_shims(options: &ShimInstallOptions) -> Result<Vec<PathBuf>, AppError> {
    let dir = shim_dir(&options.channel, options.target)?;
    fs::create_dir_all(&dir)?;
    let binary = env::current_exe()?;
    let mut written = Vec::new();
    for name in DEFAULT_SHIM_NAMES {
        written.extend(write_shim_files(&dir, &binary, name)?);
    }
    Ok(written)
}

pub fn uninstall_shims(options: &ShimInstallOptions) -> Result<usize, AppError> {
    let dir = shim_dir(&options.channel, options.target)?;
    let mut removed = 0usize;
    for name in DEFAULT_SHIM_NAMES {
        for path in shim_file_paths(&dir, name) {
            if path.exists() {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

pub fn list_shims(options: &ShimInstallOptions) -> Result<Vec<(String, PathBuf, bool)>, AppError> {
    let dir = shim_dir(&options.channel, options.target)?;
    let mut rows = Vec::new();
    for name in DEFAULT_SHIM_NAMES {
        let paths = shim_file_paths(&dir, name);
        let installed = paths.iter().any(|path| path.exists());
        let path = paths.first().cloned().unwrap_or_else(|| dir.join(name));
        rows.push((name.to_string(), path, installed));
    }
    Ok(rows)
}

pub fn print_shell_init(options: &ShellInitOptions) -> Result<(), AppError> {
    let dir = shim_dir(&options.channel, options.target)?;
    let dir_text = crate::display_path(&dir);
    match options.shell.as_str() {
        "fish" => {
            println!("set -gx PATH {} $PATH", shell_quote(&dir_text));
            println!("set -gx WF_CORE_CHANNEL {}", shell_quote(&options.channel));
        }
        "powershell" | "pwsh" => {
            println!("$env:Path = \"{};$env:Path\"", escape_ps(&dir_text));
            println!("$env:WF_CORE_CHANNEL = \"{}\"", escape_ps(&options.channel));
        }
        "cmd" => {
            println!("set PATH={};%PATH%", dir_text);
            println!("set WF_CORE_CHANNEL={}", options.channel);
        }
        _ => {
            println!("export PATH=\"{}:$PATH\"", dir_text.replace('"', "\\\""));
            println!("export WF_CORE_CHANNEL=\"{}\"", options.channel);
        }
    }
    Ok(())
}

pub fn shim_doctor(
    channel: &str,
    target: ProxyTarget,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let mut ok = Vec::new();
    let mut warn = Vec::new();
    let dir = shim_dir(channel, target)?;
    if dir.exists() {
        ok.push(format!("shim dir: {}", crate::display_path(&dir)));
    } else {
        warn.push(format!("shim dir missing: {}", crate::display_path(&dir)));
    }
    if path_contains_dir(&dir) {
        ok.push("shim dir is in PATH".to_string());
    } else {
        warn.push(format!(
            "shim dir is not in PATH; run: eval \"$(wf-core shell init --channel {channel})\""
        ));
    }
    let installed = DEFAULT_SHIM_NAMES
        .iter()
        .filter(|name| shim_file_paths(&dir, name).iter().any(|path| path.exists()))
        .count();
    if installed == DEFAULT_SHIM_NAMES.len() {
        ok.push(format!(
            "known shims: {installed}/{}",
            DEFAULT_SHIM_NAMES.len()
        ));
    } else {
        warn.push(format!(
            "known shims: {installed}/{}",
            DEFAULT_SHIM_NAMES.len()
        ));
    }
    if find_real_command("cargo", &[dir.clone()]).is_some() {
        ok.push("real command lookup: cargo".to_string());
    } else {
        warn.push("real command lookup: cargo not found outside shim dirs".to_string());
    }
    Ok((ok, warn))
}

pub fn dispatch_command(options: DispatchOptions) -> Result<i32, AppError> {
    let shim_dir_hint = env::var("WF_CORE_SHIM_DIR").ok().map(PathBuf::from);
    let mut excluded = known_shim_dirs();
    if let Some(dir) = shim_dir_hint {
        excluded.push(dir);
    }
    let real = find_real_command(&options.shim_name, &excluded).ok_or_else(|| {
        AppError::new(format!(
            "real command for shim '{}' was not found outside wf-core shim dirs",
            options.shim_name
        ))
    })?;

    if prevent_recursion_active() {
        return passthrough(&real, &options.args, &options.channel);
    }

    let mut command_args = Vec::with_capacity(options.args.len() + 1);
    command_args.push(options.shim_name.clone());
    command_args.extend(options.args.clone());
    let ast = build_ast(
        &command_args,
        env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        false,
        Some(options.shim_name.clone()),
    );

    if should_proxy(&ast) {
        let report = run_proxy(
            &command_args,
            RunOptions {
                channel: options.channel.clone(),
                target: options.target,
                invoked_as_shim: Some(options.shim_name.clone()),
                executable_override: Some(real),
                ..RunOptions::default()
            },
        )?;
        print_human_report(&report.result);
        return Ok(crate::clamp_exit_code(report.exit_code));
    }

    passthrough(&real, &options.args, &options.channel)
}

pub fn should_proxy(ast: &CommandAst) -> bool {
    if is_interactive_command(&ast.program, &ast.args) || is_destructive(&ast.program, &ast.args) {
        return false;
    }
    let program = super::command_ast::normalize_program(&ast.program);
    let first = ast.args.first().map(String::as_str).unwrap_or("");
    match ast.detected_kind {
        CommandKind::Test | CommandKind::Search | CommandKind::Build | CommandKind::Lint => true,
        CommandKind::Logs | CommandKind::Infra => true,
        CommandKind::Git => {
            if program == "gh" {
                matches!(first, "pr" | "run" | "workflow" | "checks")
            } else {
                matches!(
                    first,
                    "status" | "diff" | "show" | "log" | "grep" | "branch"
                )
            }
        }
        CommandKind::FileRead => true,
        CommandKind::FileList => {
            program == "find" || program == "tree" || ast.args.iter().any(|a| a == "-R")
        }
        CommandKind::PackageManager => {
            matches!(first, "install" | "ci" | "update" | "add" | "remove")
        }
        CommandKind::Unknown => false,
    }
}

fn write_shim_files(dir: &Path, binary: &Path, name: &str) -> Result<Vec<PathBuf>, AppError> {
    let binary_text = crate::display_path(binary);
    let mut written = Vec::new();
    if cfg!(windows) {
        let cmd_path = dir.join(format!("{name}.cmd"));
        fs::write(
            &cmd_path,
            format!(
                "@echo off\r\nset WF_CORE_SHIM_DIR=%~dp0\r\n\"{}\" dispatch --shim-name \"{}\" -- %*\r\nexit /b %ERRORLEVEL%\r\n",
                binary_text, name
            ),
        )?;
        written.push(cmd_path);
        let ps1_path = dir.join(format!("{name}.ps1"));
        fs::write(
            &ps1_path,
            format!(
                "$env:WF_CORE_SHIM_DIR = Split-Path -Parent $MyInvocation.MyCommand.Path\r\n& \"{}\" dispatch --shim-name \"{}\" -- @args\r\nexit $LASTEXITCODE\r\n",
                escape_ps(&binary_text),
                name
            ),
        )?;
        written.push(ps1_path);
    } else {
        let sh_path = dir.join(name);
        fs::write(
            &sh_path,
            format!(
                "#!/usr/bin/env sh\nexport WF_CORE_SHIM_DIR=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\"\nexec \"{}\" dispatch --shim-name \"{}\" -- \"$@\"\n",
                binary_text.replace('"', "\\\""),
                name
            ),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&sh_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&sh_path, perms)?;
        }
        written.push(sh_path);
    }
    Ok(written)
}

fn shim_file_paths(dir: &Path, name: &str) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            dir.join(format!("{name}.cmd")),
            dir.join(format!("{name}.ps1")),
        ]
    } else {
        vec![dir.join(name)]
    }
}

fn passthrough(real: &Path, args: &[String], channel: &str) -> Result<i32, AppError> {
    let status = Command::new(real)
        .args(args)
        .env("WF_CORE_PROXY_ACTIVE", "1")
        .env("WF_CORE_CHANNEL", channel)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(crate::clamp_exit_code(status.code().unwrap_or(1)))
}

fn print_human_report(result: &super::adapter::CompactResult) {
    if result.compacted {
        if !result.summary.is_empty() {
            print!("{}", result.summary);
            if !result.summary.ends_with('\n') {
                println!();
            }
        }
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
            if !result.stdout.ends_with('\n') {
                println!();
            }
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
            if !result.stderr.ends_with('\n') {
                eprintln!();
            }
        }
    } else {
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
            if !result.stdout.ends_with('\n') {
                println!();
            }
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
            if !result.stderr.ends_with('\n') {
                eprintln!();
            }
        }
        if !result.summary.is_empty() {
            print!("{}", result.summary);
            if !result.summary.ends_with('\n') {
                println!();
            }
        }
    }
}

fn find_real_command(program: &str, excluded_dirs: &[PathBuf]) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return is_allowed_command(program_path, excluded_dirs).then(|| program_path.to_path_buf());
    }
    let path_env = env::var_os("PATH")?;
    let extensions = command_extensions(program);
    for dir in env::split_paths(&path_env) {
        if is_excluded_dir(&dir, excluded_dirs) || looks_like_shim_dir(&dir) {
            continue;
        }
        for ext in &extensions {
            let candidate = dir.join(format!("{program}{ext}"));
            if is_allowed_command(&candidate, excluded_dirs) {
                return Some(candidate);
            }
        }
    }
    None
}

fn command_extensions(program: &str) -> Vec<String> {
    if Path::new(program).extension().is_some() {
        return vec![String::new()];
    }
    if cfg!(windows) {
        let mut exts = env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.PS1".to_string())
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>();
        exts.push(String::new());
        exts
    } else {
        vec![String::new()]
    }
}

fn is_allowed_command(candidate: &Path, excluded_dirs: &[PathBuf]) -> bool {
    candidate.is_file()
        && candidate
            .parent()
            .map(|parent| !is_excluded_dir(parent, excluded_dirs) && !looks_like_shim_dir(parent))
            .unwrap_or(true)
}

fn is_excluded_dir(dir: &Path, excluded_dirs: &[PathBuf]) -> bool {
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    excluded_dirs.iter().any(|excluded| {
        let excluded = excluded
            .canonicalize()
            .unwrap_or_else(|_| excluded.to_path_buf());
        canonical == excluded
    })
}

fn looks_like_shim_dir(dir: &Path) -> bool {
    let text = crate::display_path(dir)
        .replace('\\', "/")
        .to_ascii_lowercase();
    text.ends_with("/wf-core/shims") || text.contains("/wf-core/shims/")
}

fn known_shim_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(value) = env::var("WF_CORE_SHIM_DIR") {
        if !value.trim().is_empty() {
            dirs.push(PathBuf::from(value));
        }
    }
    if let Ok(value) = env::var("WF_CORE_HOME") {
        if !value.trim().is_empty() {
            dirs.push(PathBuf::from(value).join("shims"));
        }
    }
    for channel in ["stable", "next", "insiders"] {
        if let Ok(dir) = shim_dir(channel, ProxyTarget::Windsurf) {
            dirs.push(dir);
        }
    }
    if let Ok(dir) = shim_dir("next", ProxyTarget::Devin) {
        dirs.push(dir);
    }
    dirs
}

fn path_contains_dir(dir: &Path) -> bool {
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path).any(|entry| is_excluded_dir(&entry, &[dir.to_path_buf()]))
        })
        .unwrap_or(false)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn escape_ps(value: &str) -> String {
    value.replace('`', "``").replace('"', "`\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ast(tokens: &[&str]) -> CommandAst {
        build_ast(
            &tokens.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            PathBuf::from("/tmp"),
            false,
            Some(tokens[0].to_string()),
        )
    }

    #[test]
    fn should_proxy_noisy_but_not_safe_or_interactive() {
        assert!(should_proxy(&ast(&["cargo", "test", "--workspace"])));
        assert!(should_proxy(&ast(&["git", "diff"])));
        assert!(should_proxy(&ast(&["rg", "foo", "."])));
        assert!(!should_proxy(&ast(&["echo", "hello"])));
        assert!(!should_proxy(&ast(&["npm", "run", "dev"])));
        assert!(!should_proxy(&ast(&["git", "reset", "--hard"])));
    }

    #[test]
    fn shim_dir_uses_wf_core_home_override() {
        let scratch = env::temp_dir().join(format!(
            "wf-core-shim-dir-{}",
            super::super::raw_store::now_unix_ms()
        ));
        env::set_var("WF_CORE_HOME", &scratch);
        assert_eq!(
            shim_dir("next", ProxyTarget::Windsurf).unwrap(),
            scratch.join("shims")
        );
        env::remove_var("WF_CORE_HOME");
    }

    #[test]
    fn real_lookup_excludes_shim_dir() {
        let scratch = env::temp_dir().join(format!(
            "wf-core-real-{}",
            super::super::raw_store::now_unix_ms()
        ));
        let shim = scratch.join("wf-core").join("shims");
        let real = scratch.join("bin");
        fs::create_dir_all(&shim).unwrap();
        fs::create_dir_all(&real).unwrap();
        let name = if cfg!(windows) { "cargo.cmd" } else { "cargo" };
        fs::write(shim.join(name), "shim").unwrap();
        fs::write(real.join(name), "real").unwrap();
        let old_path = env::var_os("PATH");
        let joined = env::join_paths([shim.clone(), real.clone()]).unwrap();
        env::set_var("PATH", joined);
        let found = find_real_command("cargo", &[shim.clone()]).unwrap();
        assert_eq!(found, real.join(name));
        if let Some(path) = old_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}
