use std::collections::BTreeMap;

use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for build / lint / type-checker output. Groups diagnostics by file
/// and shows code + message instead of full traces.
pub struct BuildLintAdapter;

impl CommandAdapter for BuildLintAdapter {
    fn name(&self) -> &'static str {
        "build_lint"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Build
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        if matches!(ast.detected_kind, CommandKind::Build | CommandKind::Lint) {
            return true;
        }
        let normalized = normalize_program(&ast.program);
        matches!(
            normalized.as_str(),
            "tsc"
                | "eslint"
                | "biome"
                | "prettier"
                | "ruff"
                | "mypy"
                | "pyright"
                | "javac"
                | "mvn"
                | "gradle"
                | "gradlew"
                | "dotnet"
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
        let mut build = AdapterBuild::new("build_lint");
        let tool = detect_tool(ast);
        let diagnostics = parse_diagnostics(&combined, tool);
        let grouped: BTreeMap<String, Vec<Diagnostic>> =
            diagnostics
                .iter()
                .cloned()
                .fold(BTreeMap::new(), |mut map, diag| {
                    map.entry(diag.file.clone()).or_default().push(diag);
                    map
                });

        let header = if run.exit_code == 0 {
            format!("{} {} (ok)", header_tag(tool), ast.original_command)
        } else {
            format!(
                "FAIL {} (exit: {}, {} diagnostics in {} files)",
                ast.original_command,
                run.exit_code,
                diagnostics.len(),
                grouped.len()
            )
        };
        build.push_line(header);
        build.high_signal_count = diagnostics.len();

        if !grouped.is_empty() {
            let max_files = (budget.max_lines / 4).max(12);
            let per_file = 6usize;
            let mut shown_files = 0usize;
            let mut omitted_diags = 0usize;
            let mut omitted_files = 0usize;
            for (file, diags) in grouped.iter() {
                if shown_files >= max_files {
                    omitted_files += 1;
                    omitted_diags += diags.len();
                    continue;
                }
                build.push_line("");
                build.push_line(file);
                for diag in diags.iter().take(per_file) {
                    let location = match diag.column {
                        Some(col) => format!("{}:{col}", diag.line),
                        None => diag.line.to_string(),
                    };
                    let code = if diag.code.is_empty() {
                        diag.severity.clone()
                    } else {
                        format!("{} {}", diag.severity, diag.code)
                    };
                    build.push_line(&format!("  {location} {code} {}", diag.message));
                }
                if diags.len() > per_file {
                    let elided = diags.len() - per_file;
                    build.push_line(&format!("  ... ({elided} more in this file)"));
                    omitted_diags += elided;
                }
                shown_files += 1;
            }
            if omitted_files > 0 || omitted_diags > 0 {
                build.push_line("");
                build.push_line(&format!(
                    "omitted: {} diagnostics across {} files",
                    omitted_diags, omitted_files
                ));
            }
        } else if run.exit_code != 0 {
            // Couldn't parse; fall back to a small high-signal block.
            let head = stderr_text
                .lines()
                .chain(stdout_text.lines())
                .filter(|line| {
                    crate::proxy::render::is_high_signal(line)
                        || crate::proxy::render::looks_like_file_line(line)
                })
                .take(60)
                .collect::<Vec<_>>()
                .join("\n");
            if !head.is_empty() {
                build.push_line("");
                build.push_line("diagnostics:");
                build.push_block(&head);
            }
        }

        let compacted = !diagnostics.is_empty() || run.exit_code != 0;
        build.finish(run, meta, compacted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Tsc,
    Eslint,
    CargoCheck,
    CargoBuild,
    CargoClippy,
    GoBuild,
    Ruff,
    Mypy,
    Pyright,
    Prettier,
    Biome,
    Other,
}

fn detect_tool(ast: &CommandAst) -> Tool {
    let normalized = normalize_program(&ast.program);
    match normalized.as_str() {
        "tsc" => Tool::Tsc,
        "eslint" => Tool::Eslint,
        "biome" => Tool::Biome,
        "prettier" => Tool::Prettier,
        "ruff" => Tool::Ruff,
        "mypy" => Tool::Mypy,
        "pyright" => Tool::Pyright,
        "cargo" => match ast.args.first().map(|s| s.as_str()) {
            Some("clippy") => Tool::CargoClippy,
            Some("check") => Tool::CargoCheck,
            Some("build") | Some("run") => Tool::CargoBuild,
            _ => Tool::Other,
        },
        "go" if ast.args.first().map(|s| s.as_str()) == Some("build") => Tool::GoBuild,
        _ => Tool::Other,
    }
}

fn header_tag(tool: Tool) -> &'static str {
    match tool {
        Tool::Tsc => "BUILD tsc",
        Tool::Eslint | Tool::Biome | Tool::Prettier => "LINT",
        Tool::CargoBuild | Tool::CargoCheck => "BUILD cargo",
        Tool::CargoClippy => "LINT cargo clippy",
        Tool::Ruff | Tool::Mypy | Tool::Pyright => "LINT",
        Tool::GoBuild => "BUILD go",
        Tool::Other => "BUILD",
    }
}

#[derive(Debug, Clone)]
struct Diagnostic {
    file: String,
    line: u32,
    column: Option<u32>,
    severity: String,
    code: String,
    message: String,
}

fn parse_diagnostics(text: &str, tool: Tool) -> Vec<Diagnostic> {
    match tool {
        Tool::Tsc => parse_tsc(text),
        Tool::Eslint => parse_eslint(text),
        Tool::CargoBuild | Tool::CargoCheck | Tool::CargoClippy => parse_rustc(text),
        Tool::Ruff => parse_ruff(text),
        Tool::Mypy => parse_mypy(text),
        Tool::Pyright => parse_mypy(text),
        Tool::GoBuild => parse_go_build(text),
        _ => parse_generic_file_line(text),
    }
}

/// `path/to/file.ts(line,col): error TS2304: Cannot find name 'X'.`
fn parse_tsc(text: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    for raw_line in text.lines() {
        if let Some(paren) = raw_line.find('(') {
            let file = raw_line[..paren].trim().to_string();
            let rest = &raw_line[paren + 1..];
            if let Some(close) = rest.find(')') {
                let position = &rest[..close];
                let parts: Vec<&str> = position.split(',').collect();
                let line: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                let column: Option<u32> = parts.get(1).and_then(|s| s.parse().ok());
                let after = rest[close + 1..].trim_start_matches(':').trim();
                let (severity, rest_msg) = if let Some(rest) = after.strip_prefix("error") {
                    ("error".to_string(), rest.trim_start())
                } else if let Some(rest) = after.strip_prefix("warning") {
                    ("warning".to_string(), rest.trim_start())
                } else {
                    ("error".to_string(), after)
                };
                let (code, message) = split_code(rest_msg);
                if line > 0 {
                    output.push(Diagnostic {
                        file,
                        line,
                        column,
                        severity,
                        code,
                        message,
                    });
                }
            }
        }
    }
    output
}

/// `path/to/file.rs:line:col: error[E0000]: msg` (rustc/cargo).
fn parse_rustc(text: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    for raw_line in text.lines() {
        if let Some((severity, rest)) = strip_rust_severity(raw_line) {
            let (code, message) = split_bracketed_code(rest);
            output.push(Diagnostic {
                file: String::new(),
                line: 0,
                column: None,
                severity,
                code,
                message: message.trim().to_string(),
            });
        } else if let Some(point) = raw_line.strip_prefix("  --> ") {
            let parts: Vec<&str> = point.splitn(3, ':').collect();
            if let (Some(file), Some(line)) = (parts.first(), parts.get(1)) {
                let column = parts.get(2).and_then(|s| s.parse().ok());
                let line_no: u32 = line.parse().unwrap_or(0);
                if let Some(last) = output.last_mut() {
                    if last.file.is_empty() {
                        last.file = file.trim().to_string();
                        last.line = line_no;
                        last.column = column;
                    }
                }
            }
        }
    }
    output.retain(|diag| !diag.file.is_empty() && diag.line > 0);
    output
}

fn strip_rust_severity(line: &str) -> Option<(String, &str)> {
    if let Some(rest) = line.strip_prefix("error") {
        return Some(("error".to_string(), rest));
    }
    if let Some(rest) = line.strip_prefix("warning") {
        return Some(("warning".to_string(), rest));
    }
    None
}

fn split_bracketed_code(rest: &str) -> (String, String) {
    let trimmed = rest.trim_start_matches(':').trim_start();
    if let Some(start) = trimmed.strip_prefix('[') {
        if let Some(end) = start.find(']') {
            let code = start[..end].to_string();
            let message = start[end + 1..].trim_start_matches(':').trim().to_string();
            return (code, message);
        }
    }
    (
        String::new(),
        trimmed.trim_start_matches(':').trim().to_string(),
    )
}

fn split_code(message: &str) -> (String, String) {
    let mut iter = message.splitn(2, ':');
    let code_token = iter.next().unwrap_or("").trim();
    let rest = iter.next().unwrap_or("").trim();
    if code_token.starts_with("TS") || code_token.starts_with("E") {
        (code_token.to_string(), rest.to_string())
    } else {
        (String::new(), message.to_string())
    }
}

/// `path/file.js:LINE:COL: <code> <message>` (eslint default format).
fn parse_eslint(text: &str) -> Vec<Diagnostic> {
    parse_colon_form(text, "warning")
}

fn parse_ruff(text: &str) -> Vec<Diagnostic> {
    parse_colon_form(text, "error")
}

fn parse_mypy(text: &str) -> Vec<Diagnostic> {
    parse_colon_form(text, "error")
}

fn parse_go_build(text: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        // `./pkg/file.go:12:3: undefined: Foo`
        if let Some((file, rest)) = trimmed.split_once(':') {
            if !file.contains('.') {
                continue;
            }
            let mut iter = rest.splitn(3, ':');
            let line: Option<u32> = iter.next().and_then(|s| s.parse().ok());
            let column: Option<u32> = iter.next().and_then(|s| s.parse().ok());
            let message = iter.next().unwrap_or("").trim();
            if let Some(line) = line {
                output.push(Diagnostic {
                    file: file.to_string(),
                    line,
                    column,
                    severity: "error".to_string(),
                    code: String::new(),
                    message: message.to_string(),
                });
            }
        }
    }
    output
}

fn parse_colon_form(text: &str, default_severity: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut iter = trimmed.splitn(4, ':');
        let file = iter.next().unwrap_or("");
        if !file.contains('.') && !file.contains('/') {
            continue;
        }
        let line: Option<u32> = iter.next().and_then(|s| s.trim().parse().ok());
        let column: Option<u32> = iter.next().and_then(|s| s.trim().parse().ok());
        let rest = iter.next().unwrap_or("").trim();
        if let Some(line_no) = line {
            let (severity, message) = if let Some(rest) = rest.strip_prefix("error") {
                ("error".to_string(), rest.trim_start_matches(':').trim())
            } else if let Some(rest) = rest.strip_prefix("warning") {
                ("warning".to_string(), rest.trim_start_matches(':').trim())
            } else {
                (default_severity.to_string(), rest)
            };
            let (code, message) = split_leading_code(message);
            output.push(Diagnostic {
                file: file.to_string(),
                line: line_no,
                column,
                severity,
                code,
                message,
            });
        }
    }
    output
}

fn split_leading_code(message: &str) -> (String, String) {
    let trimmed = message.trim_start();
    let first = trimmed.split_whitespace().next().unwrap_or("");
    if !first.is_empty()
        && first.len() <= 8
        && first
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && first.chars().any(|c| c.is_ascii_alphabetic())
        && first.chars().any(|c| c.is_ascii_digit())
    {
        let remainder = trimmed[first.len()..].trim_start().to_string();
        (first.to_string(), remainder)
    } else {
        (String::new(), trimmed.to_string())
    }
}

fn parse_generic_file_line(text: &str) -> Vec<Diagnostic> {
    parse_colon_form(text, "error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::command_ast::build_ast;
    use std::path::PathBuf;

    fn ast_for(cmd: &[&str]) -> CommandAst {
        let owned: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
        build_ast(&owned, PathBuf::from("/tmp"), false, None)
    }

    fn meta() -> RunMeta {
        RunMeta {
            raw_id: "rid".to_string(),
            command: "fake".to_string(),
            cwd: PathBuf::from("/tmp"),
            started_at_unix_ms: 0,
            duration_ms: 0,
            exit_code: 0,
            adapter_name: "build_lint".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    #[test]
    fn matches_known_tools() {
        assert!(BuildLintAdapter.matches(&ast_for(&["tsc", "-p", "."])));
        assert!(BuildLintAdapter.matches(&ast_for(&["eslint", "src"])));
        assert!(BuildLintAdapter.matches(&ast_for(&["cargo", "clippy"])));
        assert!(BuildLintAdapter.matches(&ast_for(&["cargo", "build"])));
        assert!(BuildLintAdapter.matches(&ast_for(&["mypy", "src"])));
        assert!(!BuildLintAdapter.matches(&ast_for(&["rg", "foo"])));
    }

    #[test]
    fn tsc_errors_grouped_by_file() {
        let stderr = b"\
src/proxy/run.ts(44,12): error TS2345: Argument of type X is not assignable to Y.
src/proxy/run.ts(80,9): error TS7006: Parameter implicitly has an 'any' type.
src/adapters/git.ts(22,15): error TS2304: Cannot find name 'AdapterResult'.
Found 3 errors.
";
        let run = RawRun {
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
            exit_code: 2,
            duration_ms: 10,
        };
        let result = BuildLintAdapter.compact(
            &ast_for(&["tsc", "-p", "."]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FAIL tsc -p ."));
        assert!(result.summary.contains("3 diagnostics in 2 files"));
        assert!(result.summary.contains("src/proxy/run.ts"));
        assert!(result.summary.contains("44:12 error TS2345"));
        assert!(result.summary.contains("src/adapters/git.ts"));
    }

    #[test]
    fn rustc_errors_link_files() {
        let stderr = b"\
error[E0382]: borrow of moved value: `x`
  --> src/main.rs:10:5
   |
9  |     let x = String::new();
   |         - move occurs because `x` has type `String`, which does not implement the `Copy` trait
warning: unused variable: `y`
  --> src/main.rs:15:9
";
        let run = RawRun {
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
            exit_code: 1,
            duration_ms: 10,
        };
        let result = BuildLintAdapter.compact(
            &ast_for(&["cargo", "build"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FAIL cargo build"));
        assert!(result.summary.contains("src/main.rs"));
        assert!(result.summary.contains("10:5 error E0382"));
        assert!(result.summary.contains("15:9 warning"));
    }
}
