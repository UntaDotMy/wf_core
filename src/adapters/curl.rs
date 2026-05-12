use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for HTTP clients: curl, wget.
///
/// Summarises status, response size, headers, and first N lines of body.
pub struct CurlAdapter;

impl CommandAdapter for CurlAdapter {
    fn name(&self) -> &'static str {
        "curl"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::FileRead
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        let normalized = normalize_program(&ast.program);
        matches!(normalized.as_str(), "curl" | "wget")
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
        let mut build = AdapterBuild::new("curl");

        let body_bytes = run.stdout.len();
        let body_lines = stdout_text.lines().count();

        // Detect HTTP status from stderr (curl -v / default verbose) or exit code.
        let status = extract_http_status(&stderr_text);

        let header = if run.exit_code == 0 {
            format!(
                "HTTP {} ({} bytes, {} lines)",
                status.unwrap_or_else(|| "200".to_string()),
                body_bytes,
                body_lines
            )
        } else {
            format!(
                "HTTP FAIL {} (exit: {})",
                ast.original_command, run.exit_code
            )
        };
        build.push_line(header);

        // Extract response headers from stderr (curl -v writes them there)
        let headers = extract_response_headers(&stderr_text);
        if !headers.is_empty() {
            build.push_line("");
            build.push_line("headers:");
            for h in headers.iter().take(20) {
                build.push_line(&format!("  {h}"));
            }
            if headers.len() > 20 {
                build.push_line(&format!("  ... ({} more headers elided)", headers.len() - 20));
            }
        }

        // Show stderr diagnostics.
        let diagnostics = extract_diagnostics(&stderr_text);
        if !diagnostics.is_empty() {
            build.push_line("");
            build.push_line("diagnostics:");
            for d in diagnostics.iter().take(10) {
                build.push_line(&format!("  {d}"));
            }
            if diagnostics.len() > 10 {
                build.push_line(&format!("  ... ({} more)", diagnostics.len() - 10));
            }
        }

        // Show body head when there's meaningful content.
        if !stdout_text.is_empty() && body_lines > 0 {
            let head_cap = (budget.max_lines / 3).max(8);
            let head: Vec<&str> = stdout_text.lines().take(head_cap).collect();
            if !head.is_empty() {
                build.push_line("");
                build.push_line(&format!("body (first {} of {} lines):", head.len(), body_lines));
                for line in head {
                    build.push_line(line);
                }
                if body_lines > head_cap {
                    build.push_line(&format!("... ({} more lines elided)", body_lines - head_cap));
                }
            }
        }

        let compacted = body_lines > budget.max_lines / 4 || run.exit_code != 0 || !diagnostics.is_empty();
        build.finish(run, meta, compacted)
    }
}

fn extract_http_status(stderr: &str) -> Option<String> {
    // curl -v prints lines like: "< HTTP/1.1 200 OK"
    for line in stderr.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("< HTTP/") {
            let status_line = rest.splitn(2, ' ').nth(1).unwrap_or("200").trim();
            return Some(status_line.to_string());
        }
    }
    // Also check for "HTTP error" style messages.
    for line in stderr.lines() {
        let lowered = line.to_ascii_lowercase();
        if lowered.contains("http error") || lowered.contains("connection refused") {
            return Some("error".to_string());
        }
    }
    None
}

fn extract_diagnostics(stderr: &str) -> Vec<String> {
    let mut diags: Vec<String> = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with('<') || trimmed.starts_with('>') {
            continue;
        }
        let lowered = trimmed.to_ascii_lowercase();
        if lowered.contains("error") || lowered.contains("failed") || lowered.contains("unable")
            || lowered.contains("could not") || lowered.contains("timeout")
            || lowered.contains("refused") || lowered.contains("not found")
        {
            // Strip leading curl metadata.
            let clean = trimmed
                .trim_start_matches("curl: ")
                .trim_start_matches("curl: (")
                .trim_end_matches(')');
            diags.push(clean.to_string());
        }
    }
    diags
}

fn extract_response_headers(stderr: &str) -> Vec<String> {
    let mut headers: Vec<String> = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        // curl -v shows response headers as "< header-name: value"
        if let Some(header) = trimmed.strip_prefix("< ") {
            // Skip the status line (e.g. "< HTTP/1.1 200 OK")
            if !header.starts_with("HTTP/") {
                headers.push(header.to_string());
            }
        }
    }
    headers
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
            adapter_name: "curl".to_string(),
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
    fn matches_curl_commands() {
        assert!(CurlAdapter.matches(&ast_for(&["curl", "https://example.com"])));
        assert!(CurlAdapter.matches(&ast_for(&["wget", "https://example.com"])));
        assert!(!CurlAdapter.matches(&ast_for(&["rg", "foo"])));
    }

    #[test]
    fn curl_success_shows_size() {
        let stdout = b"line1\nline2\nline3\n";
        let stderr = b"< HTTP/1.1 200 OK\n< content-type: text/plain\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
            exit_code: 0,
            duration_ms: 100,
        };
        let result = CurlAdapter.compact(
            &ast_for(&["curl", "https://example.com"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("200"));
        assert!(result.summary.contains("18 bytes"));
        assert!(result.summary.contains("headers:"));
        assert!(result.summary.contains("content-type: text/plain"));
    }

    #[test]
    fn curl_failure_detects_diagnostics() {
        let stderr = b"curl: (6) Could not resolve host: nonexistent.example.com\n";
        let run = RawRun {
            stdout: vec![],
            stderr: stderr.to_vec(),
            exit_code: 6,
            duration_ms: 1000,
        };
        let result = CurlAdapter.compact(
            &ast_for(&["curl", "https://nonexistent.example.com"]),
            &run,
            &meta(),
            OutputBudget::default(),
        );
        assert!(result.summary.contains("FAIL"));
        assert!(result.summary.contains("diagnostics:"));
        assert!(result.summary.contains("Could not resolve host"));
    }
}
