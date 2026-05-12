use std::collections::BTreeMap;
use std::path::Path;

use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for file read / list commands: cat/sed/head/tail and ls/find/tree.
pub struct FilesAdapter;

const VENDOR_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".next",
    ".nuxt",
    ".venv",
    "__pycache__",
    "vendor",
    ".cache",
];

impl CommandAdapter for FilesAdapter {
    fn name(&self) -> &'static str {
        "files"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::FileRead
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        let normalized = normalize_program(&ast.program);
        matches!(
            normalized.as_str(),
            "cat" | "head" | "tail" | "sed" | "ls" | "find" | "tree" | "dir"
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
        let normalized = normalize_program(&ast.program);
        let mut build = AdapterBuild::new("files");

        let total_lines = stdout_text.lines().count();
        let bytes = run.stdout.len();
        let path_arg = first_path_argument(&ast.args);

        match normalized.as_str() {
            "ls" | "find" | "tree" | "dir" => {
                let header = if run.exit_code == 0 {
                    format!("FILES {}", ast.original_command)
                } else {
                    format!("FAIL {} (exit: {})", ast.original_command, run.exit_code)
                };
                build.push_line(header);
                let listing = summarize_listing(&stdout_text, &normalized);
                build.push_line(&format!("{total_lines} entries, {bytes} bytes"));
                if !listing.is_empty() {
                    build.push_line("");
                    build.push_block(&listing);
                }
            }
            _ => {
                // cat/head/tail/sed → file content
                let header = if run.exit_code == 0 {
                    format!(
                        "FILE {} ({} lines, {} bytes)",
                        path_arg.unwrap_or(&ast.original_command),
                        total_lines,
                        bytes
                    )
                } else {
                    format!("FAIL {} (exit: {})", ast.original_command, run.exit_code)
                };
                build.push_line(header);
                if let Some(path) = path_arg {
                    build.push_line(&format!("command: {}", ast.original_command));
                    build.push_line(&format!("path: {path}"));
                }
                let symbols = detect_symbols(&stdout_text, path_arg);
                if !symbols.is_empty() {
                    build.push_line("");
                    build.push_line("symbols/headings:");
                    build.push_block(&symbols);
                }
                let head_cap = (budget.max_lines / 3).max(20);
                let head = stdout_text
                    .lines()
                    .take(head_cap)
                    .collect::<Vec<_>>()
                    .join("\n");
                if total_lines > head_cap {
                    build.push_line("");
                    build.push_line(&format!("first {head_cap} lines:"));
                    build.push_block(&head);
                    let suggest = suggest_range_read(path_arg, head_cap, total_lines);
                    if let Some(s) = suggest {
                        build.push_line("");
                        build.push_line("next:");
                        build.push_line(s);
                    }
                } else if !head.is_empty() {
                    build.push_line("");
                    build.push_block(&head);
                }
            }
        }

        if run.exit_code != 0 && !stderr_text.is_empty() {
            build.push_line("");
            build.push_line("stderr:");
            build.push_block(&stderr_text.lines().take(20).collect::<Vec<_>>().join("\n"));
        }

        let compacted = total_lines > budget.max_lines / 2 || run.exit_code != 0;
        build.finish(run, meta, compacted)
    }
}

fn first_path_argument(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|arg| {
            !arg.starts_with('-') && !arg.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|s| s.as_str())
}

fn summarize_listing(stdout: &str, _program: &str) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_entries = 0usize;
    let mut vendor_omitted: BTreeMap<&'static str, usize> = BTreeMap::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let entry = line.trim();
        total_entries += 1;
        let head_segment = entry
            .split(|c| c == '/' || c == '\\')
            .next()
            .unwrap_or(entry);
        if let Some(vendor) = VENDOR_DIRECTORIES.iter().copied().find(|name| {
            entry
                .split(|c| c == '/' || c == '\\')
                .any(|seg| seg == *name)
        }) {
            *vendor_omitted.entry(vendor).or_insert(0) += 1;
            continue;
        }
        if let Some(parent) = Path::new(entry).parent() {
            let key = if parent.as_os_str().is_empty() {
                head_segment.to_string()
            } else {
                parent.display().to_string()
            };
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    let mut output = String::new();
    output.push_str(&format!("total entries: {total_entries}\n"));
    if !counts.is_empty() {
        output.push_str("\nby directory:\n");
        let mut pairs: Vec<(&String, &usize)> = counts.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (dir, count) in pairs.iter().take(20) {
            output.push_str(&format!("- {} ({})\n", dir, count));
        }
    }
    if !vendor_omitted.is_empty() {
        output.push_str("\nomitted vendor/generated:\n");
        for (name, count) in vendor_omitted.iter() {
            output.push_str(&format!("- {name} ({count})\n"));
        }
    }
    output
}

fn detect_symbols(stdout: &str, path: Option<&str>) -> String {
    let extension = path
        .and_then(|p| Path::new(p).extension().and_then(|s| s.to_str()))
        .unwrap_or("");
    let mut symbols: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        match extension {
            "rs" => {
                if trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub(crate) fn ")
                    || trimmed.starts_with("struct ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("enum ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("trait ")
                    || trimmed.starts_with("pub trait ")
                    || trimmed.starts_with("impl ")
                {
                    symbols.push(format!("- {}", strip_after_brace(trimmed)));
                }
            }
            "py" => {
                if trimmed.starts_with("def ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("async def ")
                {
                    symbols.push(format!("- {}", strip_after_brace(trimmed)));
                }
            }
            "ts" | "tsx" | "js" | "jsx" => {
                if trimmed.starts_with("export function ")
                    || trimmed.starts_with("function ")
                    || trimmed.starts_with("export class ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("export const ")
                {
                    symbols.push(format!("- {}", strip_after_brace(trimmed)));
                }
            }
            "md" | "markdown" => {
                if trimmed.starts_with('#') {
                    symbols.push(format!("- {}", trimmed));
                }
            }
            _ => {}
        }
        if symbols.len() >= 25 {
            break;
        }
    }
    symbols.join("\n")
}

fn strip_after_brace(line: &str) -> String {
    line.split_terminator(|c| c == '{' || c == ':')
        .next()
        .unwrap_or(line)
        .trim()
        .to_string()
}

fn suggest_range_read(path: Option<&str>, head_lines: usize, total: usize) -> Option<String> {
    let path = path?;
    let start = head_lines + 1;
    let end = (head_lines + head_lines).min(total);
    if end <= start {
        return None;
    }
    Some(format!("wf-core run -- sed -n '{start},{end}p' {path}"))
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
            adapter_name: "files".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    #[test]
    fn matches_file_commands() {
        assert!(FilesAdapter.matches(&ast_for(&["cat", "src/main.rs"])));
        assert!(FilesAdapter.matches(&ast_for(&["ls", "-R", "src"])));
        assert!(FilesAdapter.matches(&ast_for(&["find", ".", "-type", "f"])));
        assert!(!FilesAdapter.matches(&ast_for(&["rg", "foo"])));
    }

    #[test]
    fn cat_large_rs_extracts_symbols_and_suggests_range() {
        let mut stdout = String::new();
        stdout.push_str("pub fn main() {\n");
        stdout.push_str("    println!(\"x\");\n");
        for index in 0..3000 {
            stdout.push_str(&format!("line {index}\n"));
        }
        stdout.push_str("pub struct Frobnicator {\n    x: u32,\n}\n");
        stdout.push_str("fn helper() {\n");
        let run = RawRun {
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = FilesAdapter.compact(
            &ast_for(&["cat", "src/main.rs"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FILE src/main.rs"));
        assert!(result.summary.contains("symbols/headings:"));
        assert!(result.summary.contains("pub fn main()"));
        assert!(result.summary.contains("pub struct Frobnicator"));
        assert!(result.summary.contains("next:"));
        assert!(result.summary.contains("sed -n"));
    }

    #[test]
    fn ls_groups_by_directory_and_omits_vendor() {
        let stdout = b"src/main.rs\nsrc/proxy/run.rs\nsrc/proxy/adapter.rs\nnode_modules/foo/index.js\nnode_modules/foo/package.json\ntarget/release/wf-core\ndocs/readme.md\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = FilesAdapter.compact(
            &ast_for(&["ls", "-R", "."]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FILES ls -R ."));
        assert!(result.summary.contains("omitted vendor/generated:"));
        assert!(result.summary.contains("node_modules"));
        assert!(result.summary.contains("target"));
        assert!(result.summary.contains("by directory:"));
    }
}
