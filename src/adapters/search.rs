use std::collections::BTreeMap;

use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for code-search tools: rg, grep, ag, ack, git grep.
pub struct SearchAdapter;

impl CommandAdapter for SearchAdapter {
    fn name(&self) -> &'static str {
        "search"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Search
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        let normalized = normalize_program(&ast.program);
        if matches!(
            normalized.as_str(),
            "rg" | "grep" | "ag" | "ack" | "ripgrep"
        ) {
            return true;
        }
        if normalized == "git" && ast.args.iter().any(|a| a == "grep") {
            return true;
        }
        false
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
        let mut build = AdapterBuild::new("search");

        let header = if run.exit_code == 0 {
            format!("SEARCH {}", ast.original_command)
        } else if run.exit_code == 1 {
            // ripgrep/grep return 1 when no matches were found — treat as ok.
            format!("SEARCH {} (no matches)", ast.original_command)
        } else {
            format!("FAIL {} (exit: {})", ast.original_command, run.exit_code)
        };
        build.push_line(header);

        let grouped = group_matches(&stdout_text);
        let total_matches: usize = grouped.values().map(|m| m.len()).sum();
        let total_files = grouped.len();
        if total_matches > 0 {
            build.push_line(&format!("{total_matches} matches in {total_files} files"));
            let file_cap = (budget.max_lines / 5).max(8);
            let per_file_cap = 6usize;
            let mut shown_files = 0usize;
            let mut omitted_matches = 0usize;
            let mut omitted_files = 0usize;
            for (file, matches) in grouped.iter() {
                if shown_files >= file_cap {
                    omitted_files += 1;
                    omitted_matches += matches.len();
                    continue;
                }
                build.push_line("");
                build.push_line(file);
                let mut shown_in_file = 0usize;
                for m in matches.iter().take(per_file_cap) {
                    if m.line_number == "?" {
                        build.push_line(&format!("  {}", m.text));
                    } else {
                        build.push_line(&format!("  {}: {}", m.line_number, m.text));
                    }
                    shown_in_file += 1;
                }
                if matches.len() > shown_in_file {
                    let elided = matches.len() - shown_in_file;
                    build.push_line(&format!("  ... ({elided} more matches in this file)"));
                    omitted_matches += elided;
                }
                shown_files += 1;
            }
            if omitted_files > 0 || omitted_matches > 0 {
                build.push_line("");
                build.push_line(&format!(
                    "omitted: {} matches across {} files",
                    omitted_matches, omitted_files
                ));
            }
            build.high_signal_count = total_matches;
        } else {
            // Fallback for unfamiliar shapes — show a small head of stdout.
            let head = stdout_text.lines().take(20).collect::<Vec<_>>().join("\n");
            if !head.is_empty() {
                build.push_line("");
                build.push_block(&head);
            }
        }

        if run.exit_code != 0 && run.exit_code != 1 && !stderr_text.is_empty() {
            build.push_line("");
            build.push_line("stderr:");
            build.push_block(&stderr_text.lines().take(20).collect::<Vec<_>>().join("\n"));
        }

        let compacted = total_matches > 8 || run.exit_code > 1;
        build.finish(run, meta, compacted)
    }
}

#[derive(Debug, Clone)]
struct Match {
    line_number: String,
    text: String,
}

fn group_matches(stdout: &str) -> BTreeMap<String, Vec<Match>> {
    let mut grouped: BTreeMap<String, Vec<Match>> = BTreeMap::new();
    for line in stdout.lines() {
        if let Some((file, rest)) = split_file_line(line) {
            let (line_number, text) = split_line_text(rest);
            grouped.entry(file.to_string()).or_default().push(Match {
                line_number: line_number.to_string(),
                text: text.to_string(),
            });
        }
    }
    grouped
}

/// Split a typical `path/to/file.rs:line:col:content` or `file:line:content`
/// match. Returns `(file, "<rest after first colon>")`.
fn split_file_line(line: &str) -> Option<(&str, &str)> {
    // Skip Windows drive letters by looking for a colon after position 2.
    let bytes = line.as_bytes();
    let mut start = 0;
    if bytes.len() > 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        start = 2;
    }
    let idx = line[start..].find(':')?;
    let split_at = start + idx;
    if split_at == 0 {
        return None;
    }
    let file = &line[..split_at];
    let rest = &line[split_at + 1..];
    // Reject pure binary-style or non-path lines: must have an extension or
    // path separator to look file-like.
    if !file.contains('.') && !file.contains('/') && !file.contains('\\') {
        return None;
    }
    Some((file, rest))
}

fn split_line_text(rest: &str) -> (&str, &str) {
    if let Some(idx) = rest.find(':') {
        let prefix = &rest[..idx];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            return (prefix, rest[idx + 1..].trim_start_matches(':').trim_start());
        }
    }
    // Fallback: no line number detected.
    ("?", rest)
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
            adapter_name: "search".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    #[test]
    fn matches_rg_grep_and_git_grep() {
        assert!(SearchAdapter.matches(&ast_for(&["rg", "foo"])));
        assert!(SearchAdapter.matches(&ast_for(&["grep", "-r", "foo", "src"])));
        assert!(SearchAdapter.matches(&ast_for(&["git", "grep", "foo"])));
        assert!(!SearchAdapter.matches(&ast_for(&["cargo", "test"])));
    }

    #[test]
    fn rg_output_is_grouped_by_file() {
        let stdout = b"src/proxy/adapter.rs:12:pub struct CompactResult {\nsrc/proxy/adapter.rs:44:    fn compact(...) -> CompactResult\nsrc/proxy/run.rs:88:    let compact: CompactResult = ...\nsrc/proxy/run.rs:120:    write_compact(&compact_path, &compact)?;\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = SearchAdapter.compact(
            &ast_for(&["rg", "CompactResult"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("SEARCH rg CompactResult"));
        assert!(result.summary.contains("4 matches in 2 files"));
        assert!(result.summary.contains("src/proxy/adapter.rs"));
        assert!(result.summary.contains("12: pub struct CompactResult"));
    }

    #[test]
    fn no_matches_shows_friendly_summary() {
        let run = RawRun {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: 1,
            duration_ms: 10,
        };
        let result = SearchAdapter.compact(
            &ast_for(&["rg", "nothing-here"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("no matches"));
    }
}
