use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};
use crate::proxy::render::is_high_signal;

use super::common::AdapterBuild;

/// Adapter for log-like commands: docker/kubectl/journalctl/etc.
pub struct LogsAdapter;

impl CommandAdapter for LogsAdapter {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Logs
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        if ast.detected_kind == CommandKind::Logs {
            return true;
        }
        let normalized = normalize_program(&ast.program);
        match normalized.as_str() {
            "docker" => ast
                .args
                .first()
                .map(|a| matches!(a.as_str(), "logs" | "compose"))
                .unwrap_or(false),
            "docker-compose" => true,
            "kubectl" => ast
                .args
                .first()
                .map(|a| matches!(a.as_str(), "logs" | "describe" | "events" | "get" | "top"))
                .unwrap_or(false),
            "journalctl" => true,
            "helm" => ast
                .args
                .first()
                .map(|a| matches!(a.as_str(), "template" | "upgrade"))
                .unwrap_or(false),
            "terraform" => ast
                .args
                .first()
                .map(|a| matches!(a.as_str(), "plan"))
                .unwrap_or(false),
            _ => false,
        }
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
        let combined = format!("{stdout_text}{stderr_text}");
        let total_lines = combined.lines().count();
        let groups = group_logs(&combined, budget.per_group_limit);
        let errors = group_high_signal(&combined);
        let recent_tail = tail_n(&combined, 20);
        let service = detect_service(ast);

        let mut build = AdapterBuild::new("logs");
        let header = format!(
            "LOGS {} (exit: {}, {} lines, {} repeated groups)",
            ast.original_command, run.exit_code, total_lines, groups
        );
        build.push_line(header);

        if let Some(service) = service {
            build.push_line(&format!("service: {service}"));
        }

        if !errors.is_empty() {
            build.push_line("");
            build.push_line("errors:");
            let cap = (budget.failure_max_lines / 4).max(10);
            for entry in errors.iter().take(cap) {
                build.push_line(&format!("- {}", entry.line));
                if entry.repeated > 1 {
                    build.push_line(&format!("  repeated {} times", entry.repeated));
                }
            }
            if errors.len() > cap {
                let omitted = errors.len() - cap;
                build.push_line(&format!("  ... ({omitted} more error groups elided)"));
            }
            build.high_signal_count = errors.len();
        }

        if !recent_tail.is_empty() {
            build.push_line("");
            build.push_line("recent tail:");
            build.push_block(&recent_tail);
        }

        let compacted = total_lines > budget.max_lines / 2 || run.exit_code != 0 || groups > 0;
        build.finish(run, meta, compacted)
    }
}

fn tail_n(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let len = lines.len();
    let start = len.saturating_sub(n);
    lines[start..].join("\n")
}

fn group_logs(text: &str, per_group_limit: usize) -> usize {
    let mut prev: Option<&str> = None;
    let mut repeat = 0usize;
    let mut collapsed = 0usize;
    for line in text.lines() {
        if Some(line) == prev {
            repeat += 1;
            if repeat >= per_group_limit {
                collapsed += 1;
            }
        } else {
            prev = Some(line);
            repeat = 0;
        }
    }
    collapsed
}

#[derive(Debug, Clone)]
struct HighSignalEntry {
    line: String,
    repeated: usize,
}

fn group_high_signal(text: &str) -> Vec<HighSignalEntry> {
    let mut output: Vec<HighSignalEntry> = Vec::new();
    for line in text.lines() {
        if !is_high_signal(line) {
            continue;
        }
        let normalized = line.trim();
        if normalized.is_empty() {
            continue;
        }
        if let Some(last) = output.last_mut() {
            if last.line == normalized {
                last.repeated += 1;
                continue;
            }
        }
        output.push(HighSignalEntry {
            line: normalized.to_string(),
            repeated: 1,
        });
    }
    // Coalesce non-adjacent duplicates so the same error reported many times
    // anywhere in the log surfaces once with a count.
    let mut coalesced: Vec<HighSignalEntry> = Vec::new();
    for entry in output.into_iter() {
        if let Some(existing) = coalesced.iter_mut().find(|e| e.line == entry.line) {
            existing.repeated += entry.repeated;
        } else {
            coalesced.push(entry);
        }
    }
    coalesced
}

fn detect_service(ast: &CommandAst) -> Option<String> {
    let normalized = normalize_program(&ast.program);
    match normalized.as_str() {
        "docker" => ast
            .args
            .iter()
            .find(|arg| !arg.starts_with('-') && *arg != "logs" && *arg != "compose")
            .cloned(),
        "kubectl" => ast
            .args
            .iter()
            .find(|arg| {
                !arg.starts_with('-')
                    && *arg != "logs"
                    && *arg != "describe"
                    && *arg != "events"
                    && *arg != "get"
                    && *arg != "top"
            })
            .cloned(),
        "journalctl" => ast
            .args
            .iter()
            .position(|a| a == "-u")
            .and_then(|i| ast.args.get(i + 1).cloned()),
        _ => None,
    }
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
            adapter_name: "logs".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    #[test]
    fn matches_logs_commands() {
        assert!(LogsAdapter.matches(&ast_for(&["docker", "logs", "api"])));
        assert!(LogsAdapter.matches(&ast_for(&["kubectl", "logs", "pod-1"])));
        assert!(LogsAdapter.matches(&ast_for(&["journalctl", "-u", "myservice"])));
        assert!(!LogsAdapter.matches(&ast_for(&["rg", "foo"])));
    }

    #[test]
    fn docker_logs_groups_errors_and_shows_tail() {
        let mut stdout = String::new();
        for _ in 0..50 {
            stdout.push_str("2026-05-12T12:33:01Z ERROR database connection refused\n");
        }
        for _ in 0..100 {
            stdout.push_str("2026-05-12T12:34:12Z WARN retrying request /v1/users\n");
        }
        for index in 0..50 {
            stdout.push_str(&format!(
                "2026-05-12T12:35:{:02}Z INFO request {index} ok\n",
                index
            ));
        }
        stdout.push_str("2026-05-12T12:36:00Z server listening on :8080\n");
        stdout.push_str("2026-05-12T12:36:02Z health check ok\n");
        let run = RawRun {
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 10,
        };
        let result = LogsAdapter.compact(
            &ast_for(&["docker", "logs", "api"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("LOGS docker logs api"));
        assert!(result.summary.contains("service: api"));
        assert!(result.summary.contains("errors:"));
        assert!(result.summary.contains("database connection refused"));
        assert!(result.summary.contains("repeated"));
        assert!(result.summary.contains("recent tail:"));
        assert!(result.summary.contains("health check ok"));
    }
}
