use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for package manager output: npm install, pip install, etc.
///
/// Summarises added/updated/removed packages, audit findings, and errors.
pub struct PackageAdapter;

impl CommandAdapter for PackageAdapter {
    fn name(&self) -> &'static str {
        "package"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::PackageManager
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        if ast.detected_kind != CommandKind::PackageManager {
            return false;
        }
        let normalized = normalize_program(&ast.program);
        matches!(
            normalized.as_str(),
            "npm" | "pnpm" | "yarn" | "bun" | "cargo" | "pip" | "pip3"
        )
    }

    fn compact(
        &self,
        ast: &CommandAst,
        run: &RawRun,
        meta: &RunMeta,
        budget: OutputBudget,
    ) -> CompactResult {
        let stdout_text = String::from_utf8_lossy(&run.stdout).into_owned();
        let stderr_text = String::from_utf8_lossy(&run.stderr).into_owned();
        let combined = format!("{stdout_text}\n{stderr_text}");
        let mut build = AdapterBuild::new("package");

        let program = normalize_program(&ast.program);
        let header = if run.exit_code == 0 {
            format!("PACKAGE {} (ok)", ast.original_command)
        } else {
            format!("FAIL {} (exit: {})", ast.original_command, run.exit_code)
        };
        build.push_line(header);

        // Extract summary information based on package manager.
        match program.as_str() {
            "npm" | "pnpm" | "yarn" | "bun" => {
                extract_npm_summary(&combined, &mut build);
            }
            "pip" | "pip3" => {
                extract_pip_summary(&combined, &mut build);
            }
            "cargo" => {
                extract_cargo_summary(&combined, &mut build);
            }
            _ => {
                // Fallback: show first N lines and highlight errors.
                extract_fallback(&combined, &mut build, &budget);
            }
        }

        // If exit code is non-zero, always include error lines.
        if run.exit_code != 0 {
            let error_lines: Vec<&str> = combined
                .lines()
                .filter(|l| {
                    let lower = l.to_ascii_lowercase();
                    lower.contains("error") || lower.contains("failed") || lower.contains("err!")
                })
                .take(20)
                .collect();
            if !error_lines.is_empty() {
                build.push_line("");
                build.push_line("errors:");
                for line in error_lines {
                    build.push_line(line);
                }
            }
        }

        let total_lines = combined.lines().count();
        let compacted = total_lines > budget.max_lines / 4 || run.exit_code != 0;
        build.finish(run, meta, compacted)
    }
}

fn extract_npm_summary(text: &str, build: &mut AdapterBuild) {
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut removed = 0usize;
    let mut vulnerabilities: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // Individual package lines from npm/pnpm/yarn
        if trimmed.starts_with("+ ") {
            added += 1;
        }
        if trimmed.starts_with("- ") {
            removed += 1;
        }
        if trimmed.starts_with("~ ") {
            updated += 1;
        }

        // npm summary lines
        if let Some(rest) = trimmed.strip_prefix("added ") {
            if rest.contains("package") {
                let count: usize = rest.split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or(0);
                added += count;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("removed ") {
            if rest.contains("package") {
                let count: usize = rest.split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or(0);
                removed += count;
            }
        }
        if trimmed.starts_with("changed ") && trimmed.contains("package") {
            let count: usize = trimmed
                .split_whitespace()
                .nth(1)
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
            updated += count;
        }

        // npm audit
        if let Some(rest) = trimmed.strip_prefix("found ") {
            let _vuln_count: usize = rest.split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or(0);
        }
        if trimmed.contains("vulnerabilities") || trimmed.contains("vulnerability") {
            let num: usize = trimmed
                .split_whitespace()
                .find_map(|w| w.parse().ok())
                .unwrap_or(0);
            if num > 0 {
                vulnerabilities.push(trimmed.to_string());
            }
        }
        // Also capture "X packages are looking for funding"
        if trimmed.contains("funding") {
            build.push_line(&format!("  funding: {trimmed}"));
        }
    }

    build.push_line(&format!("  packages: +{added} -{removed} ~{updated}"));
    if !vulnerabilities.is_empty() {
        build.push_line("  vulnerabilities:");
        for v in vulnerabilities.iter().take(5) {
            build.push_line(&format!("    {v}"));
        }
    }
}

fn extract_pip_summary(text: &str, build: &mut AdapterBuild) {
    let mut installed = 0usize;
    let mut requirements = false;

    for line in text.lines() {
        let trimmed = line.trim();
        // pip install output usually starts with "Collecting", "Installing", "Successfully installed"
        if trimmed.starts_with("Successfully installed") {
            let count = trimmed.split_whitespace().count().saturating_sub(2);
            installed += count;
            requirements = true;
        }
        if trimmed.starts_with("Requirement already satisfied") {
            requirements = true;
        }
        if trimmed.starts_with("Installing collected packages") {
            requirements = true;
        }
        if trimmed.starts_with("Successfully built") {
            build.push_line(&format!("  built: {trimmed}"));
        }
    }

    if installed > 0 {
        build.push_line(&format!("  installed: {installed} packages"));
    }
    if !requirements {
        // Maybe it was a pip list or other command — show compact info.
        let total_lines = text.lines().count();
        if total_lines < 30 {
            // Short output, likely listing or info.
            build.push_line(&format!("  {total_lines} lines"));
        }
    }
}

fn extract_cargo_summary(text: &str, build: &mut AdapterBuild) {
    let mut fresh = 0usize;
    let mut changed = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Compiling") || trimmed.starts_with("   Compiling") {
            changed += 1;
        }
        if trimmed.starts_with("Finished") {
            build.push_line(&format!("  {trimmed}"));
        }
        if trimmed.starts_with("Downloading") {
            fresh += 1;
        }
    }
    if fresh > 0 {
        build.push_line(&format!("  downloaded: {fresh} crates"));
    }
    if changed > 0 {
        build.push_line(&format!("  compiled: {changed} crates"));
    }
}

fn extract_fallback(text: &str, build: &mut AdapterBuild, _budget: &OutputBudget) {
    let total_lines = text.lines().count();
    // Find packages or key-value style lines that look informational.
    let mut info_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with('=') {
            continue;
        }
        if trimmed.contains("→") || trimmed.contains("✔") || trimmed.contains("✗") {
            info_lines.push(trimmed);
        }
    }
    if info_lines.len() > 4 {
        build.push_line(&format!("  {total_lines} total lines, {} info signals", info_lines.len()));
        for line in info_lines.iter().take(10) {
            build.push_line(&format!("  {line}"));
        }
        if info_lines.len() > 10 {
            build.push_line(&format!("  ... ({} more)", info_lines.len() - 10));
        }
    } else {
        build.push_line(&format!("  {total_lines} lines"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::command_ast::build_ast;
    use std::path::PathBuf;

    fn meta() -> RunMeta {
        RunMeta {
            raw_id: "rid".to_string(),
            command: "fake".to_string(),
            cwd: PathBuf::from("/tmp"),
            started_at_unix_ms: 0,
            duration_ms: 0,
            exit_code: 0,
            adapter_name: "package".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    fn ast_for(cmd: &[&str]) -> CommandAst {
        let owned: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
        build_ast(&owned, PathBuf::from("/tmp"), false, None)
    }

    #[test]
    fn matches_npm_install() {
        assert!(PackageAdapter.matches(&ast_for(&["npm", "install"])));
        assert!(PackageAdapter.matches(&ast_for(&["pip", "install", "requests"])));
        assert!(PackageAdapter.matches(&ast_for(&["pnpm", "add", "react"])));
        assert!(!PackageAdapter.matches(&ast_for(&["rg", "foo"])));
    }

    #[test]
    fn npm_install_summarises_counts() {
        let mut stdout = String::new();
        stdout.push_str("+ react@18.2.0\n");
        stdout.push_str("+ lodash@4.17.21\n");
        stdout.push_str("+ express@4.18.0\n");
        stdout.push_str("added 42 packages in 2s\n");
        stdout.push_str("found 0 vulnerabilities\n");
        let run = RawRun {
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 2000,
        };
        let result = PackageAdapter.compact(
            &ast_for(&["npm", "install"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("PACKAGE npm install"));
        assert!(result.summary.contains("packages: +"));
    }

    #[test]
    fn pip_install_summarises() {
        let stdout = b"Collecting requests\n  Downloading requests-2.31.0-py3-none-any.whl (62 kB)\nInstalling collected packages: requests\nSuccessfully installed requests-2.31.0\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 3000,
        };
        let result = PackageAdapter.compact(
            &ast_for(&["pip", "install", "requests"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("PACKAGE pip install requests"));
        assert!(result.summary.contains("installed: 1 packages"));
    }

    #[test]
    fn npm_failure_shows_errors() {
        let stdout = b"npm ERR! code ENOENT\nnpm ERR! syscall open\nnpm ERR! path /nonexistent/package.json\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 1,
            duration_ms: 500,
        };
        let result = PackageAdapter.compact(
            &ast_for(&["npm", "install"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FAIL"));
        assert!(result.summary.contains("errors:"));
        assert!(result.summary.contains("ENOENT"));
    }
}
