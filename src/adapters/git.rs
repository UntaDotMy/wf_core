use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for git/gh commands. Summarises status/diff/log/branch output
/// without dumping the whole hunk to the model.
pub struct GitAdapter;

impl CommandAdapter for GitAdapter {
    fn name(&self) -> &'static str {
        "git"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Git
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        let normalized = normalize_program(&ast.program);
        matches!(normalized.as_str(), "git" | "gh")
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
        let subcommand = ast
            .args
            .iter()
            .find(|arg| !arg.starts_with('-'))
            .map(|s| s.as_str())
            .unwrap_or("");
        let mut build = AdapterBuild::new("git");

        let header = match (run.exit_code, subcommand) {
            (0, "diff") | (0, "show") => format!("DIFF {}", ast.original_command),
            (0, "status") => format!("STATUS {}", ast.original_command),
            (0, "log") => format!("LOG {}", ast.original_command),
            (0, "branch") => format!("BRANCH {}", ast.original_command),
            (0, _) => format!("GIT {}", ast.original_command),
            (code, _) => format!("FAIL {} (exit: {code})", ast.original_command),
        };
        build.push_line(header);

        let body = match subcommand {
            "status" => summarize_status(&stdout_text),
            "diff" | "show" => summarize_diff(&stdout_text, budget.max_lines),
            "log" => summarize_log(&stdout_text, budget.max_lines),
            "branch" => summarize_branch(&stdout_text, budget.max_lines),
            _ => default_summary(&stdout_text, &stderr_text, budget.max_lines),
        };
        if !body.is_empty() {
            build.push_line("");
            build.push_block(&body);
        }

        if run.exit_code != 0 && !stderr_text.is_empty() {
            let cap = budget.failure_max_lines / 4;
            let trimmed = head_n_lines(&stderr_text, cap.max(10));
            build.push_line("");
            build.push_line("stderr:");
            build.push_block(&trimmed);
        }

        let compacted = run.stdout.len() + run.stderr.len() > 1024
            || stdout_text.lines().count() + stderr_text.lines().count() > budget.max_lines / 2
            || subcommand == "diff"
            || subcommand == "show"
            || subcommand == "log";
        build.finish(run, meta, compacted || run.exit_code != 0)
    }
}

fn head_n_lines(text: &str, n: usize) -> String {
    text.lines()
        .take(n)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn summarize_status(stdout: &str) -> String {
    let mut branch: Option<String> = None;
    let mut modified = 0usize;
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut renamed = 0usize;
    let mut untracked = 0usize;
    let mut staged = 0usize;
    let mut files: Vec<String> = Vec::new();

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            branch = Some(rest.split_whitespace().next().unwrap_or(rest).to_string());
            continue;
        }
        // Long-form status output.
        if line.starts_with("On branch ") {
            branch = Some(line["On branch ".len()..].trim().to_string());
            continue;
        }
        // Short porcelain (-s / --short) two-char status code.
        if line.len() >= 3 {
            let code = &line[0..2];
            let path = line[3..].trim();
            match code.trim() {
                "M" | "AM" | "MM" => modified += 1,
                "A" | "??A" => added += 1,
                "D" => deleted += 1,
                "R" => renamed += 1,
                "??" => untracked += 1,
                _ if code.chars().next().is_some_and(|c| c != ' ' && c != '?') => staged += 1,
                _ => {}
            }
            if !path.is_empty() && files.len() < 30 {
                files.push(format!("{code} {path}"));
            }
        }
    }
    let mut output = String::new();
    if let Some(branch) = branch {
        output.push_str(&format!("branch: {branch}\n"));
    }
    output.push_str(&format!(
        "{} modified, {} added, {} deleted, {} renamed, {} untracked, {} staged\n",
        modified, added, deleted, renamed, untracked, staged
    ));
    if !files.is_empty() {
        output.push_str("\nfiles:\n");
        for line in &files {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn summarize_diff(stdout: &str, max_lines: usize) -> String {
    let mut files: Vec<(String, isize, isize)> = Vec::new();
    let mut current: Option<(String, isize, isize)> = None;
    let mut hunks: Vec<String> = Vec::new();

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(entry) = current.take() {
                files.push(entry);
            }
            let pieces: Vec<&str> = rest.split_whitespace().collect();
            let path = pieces
                .get(1)
                .map(|p| p.trim_start_matches("b/"))
                .unwrap_or("?")
                .to_string();
            current = Some((path, 0, 0));
            continue;
        }
        if let Some(entry) = current.as_mut() {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if line.starts_with("@@") {
                if hunks.len() < 12 {
                    hunks.push(format!("{}: {}", entry.0, line));
                }
                continue;
            }
            if line.starts_with('+') && !line.starts_with("+++") {
                entry.1 += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                entry.2 += 1;
            }
        }
    }
    if let Some(entry) = current.take() {
        files.push(entry);
    }

    let total_added: isize = files.iter().map(|f| f.1).sum();
    let total_removed: isize = files.iter().map(|f| f.2).sum();
    let mut output = String::new();
    output.push_str(&format!(
        "{} files changed, +{} -{}\n",
        files.len(),
        total_added,
        total_removed
    ));

    if !files.is_empty() {
        output.push_str("\nfiles:\n");
        for (path, added, removed) in files.iter().take(max_lines.min(50)) {
            output.push_str(&format!("- {path} +{added} -{removed}\n"));
        }
    }
    if !hunks.is_empty() {
        output.push_str("\nimportant hunks:\n");
        for hunk in hunks.iter().take(20) {
            output.push_str(hunk);
            output.push('\n');
        }
    }
    output
}

fn summarize_log(stdout: &str, max_lines: usize) -> String {
    // Each commit block starts with `commit <sha>`. Show one line per commit.
    let mut entries: Vec<String> = Vec::new();
    let mut current_sha: Option<String> = None;
    let mut current_subject: Option<String> = None;
    let mut current_author: Option<String> = None;
    let cap = max_lines.min(60);
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("commit ") {
            if let (Some(sha), Some(subject)) = (current_sha.take(), current_subject.take()) {
                let author = current_author.take().unwrap_or_default();
                entries.push(format!(
                    "{} {} {}",
                    &sha.chars().take(8).collect::<String>(),
                    author,
                    subject
                ));
                if entries.len() >= cap {
                    break;
                }
            }
            current_sha = Some(rest.split_whitespace().next().unwrap_or(rest).to_string());
            current_subject = None;
        } else if let Some(rest) = line.strip_prefix("Author:") {
            current_author = Some(rest.trim().to_string());
        } else if line.starts_with("    ") && current_subject.is_none() && !line.trim().is_empty() {
            current_subject = Some(line.trim().to_string());
        }
    }
    if let (Some(sha), Some(subject)) = (current_sha, current_subject) {
        let author = current_author.unwrap_or_default();
        entries.push(format!(
            "{} {} {}",
            &sha.chars().take(8).collect::<String>(),
            author,
            subject
        ));
    }
    let mut output = String::new();
    output.push_str(&format!("{} commits\n", entries.len()));
    if !entries.is_empty() {
        output.push('\n');
        for entry in entries.iter().take(cap) {
            output.push_str(entry);
            output.push('\n');
        }
    }
    output
}

fn summarize_branch(stdout: &str, max_lines: usize) -> String {
    let mut current: Option<String> = None;
    let mut other: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix("* ") {
            current = Some(rest.to_string());
        } else if !trimmed.trim().is_empty() {
            other.push(trimmed.trim().to_string());
        }
    }
    let mut output = String::new();
    if let Some(branch) = &current {
        output.push_str(&format!("* {branch}\n"));
    }
    output.push_str(&format!("{} other branches\n", other.len()));
    for branch in other.iter().take(max_lines.min(40)) {
        output.push_str(&format!("  {branch}\n"));
    }
    output
}

fn default_summary(stdout: &str, stderr: &str, max_lines: usize) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "stdout: {} lines\nstderr: {} lines\n",
        stdout.lines().count(),
        stderr.lines().count()
    ));
    let cap = max_lines / 3;
    if !stdout.is_empty() {
        output.push_str("\nstdout (head):\n");
        output.push_str(&head_n_lines(stdout, cap.max(10)));
        output.push('\n');
    }
    output
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
            adapter_name: "git".to_string(),
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
    fn matches_git_and_gh() {
        assert!(GitAdapter.matches(&ast_for(&["git", "status"])));
        assert!(GitAdapter.matches(&ast_for(&["gh", "pr", "view"])));
        assert!(!GitAdapter.matches(&ast_for(&["cargo", "test"])));
    }

    #[test]
    fn git_status_short_summarizes_counts() {
        let stdout = b"## main\n M src/main.rs\n?? new.txt\n M docs/readme.md\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = GitAdapter.compact(
            &ast_for(&["git", "status", "--short"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("STATUS git status --short"));
        assert!(result.summary.contains("branch: main"));
        assert!(result.summary.contains("2 modified"));
        assert!(result.summary.contains("1 untracked"));
    }

    #[test]
    fn git_diff_summarizes_files_and_hunks() {
        let stdout = b"\
diff --git a/src/main.rs b/src/main.rs
index 1111..2222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +12,5 @@ fn run(args: &[String]) {
+    let mut x = 1;
+    let mut y = 2;
-    println!(\"old\");
diff --git a/README.md b/README.md
index 3333..4444 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-old title
+new title
";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = GitAdapter.compact(
            &ast_for(&["git", "diff"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("DIFF git diff"));
        assert!(result.summary.contains("2 files changed"));
        assert!(result.summary.contains("src/main.rs"));
        assert!(result.summary.contains("README.md"));
        assert!(result.summary.contains("important hunks:"));
    }
}
