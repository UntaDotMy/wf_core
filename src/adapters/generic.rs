use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{CommandAst, CommandKind};
use crate::proxy::render::{
    head_tail_snapshot, is_high_signal, normalise_for_compression, raw_recovery_line, savings_line,
};
use crate::proxy::token_meter::TokenMeter;

/// Generic fallback adapter. Always matches (handled by the registry).
pub struct GenericAdapter;

impl CommandAdapter for GenericAdapter {
    fn name(&self) -> &'static str {
        "generic"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Unknown
    }

    fn matches(&self, _ast: &CommandAst) -> bool {
        // Registry treats `generic` as the last-resort fallback; never claim a
        // specific match here.
        false
    }

    fn compact(
        &self,
        _ast: &CommandAst,
        run: &RawRun,
        meta: &RunMeta,
        budget: OutputBudget,
    ) -> CompactResult {
        let stdout_text = String::from_utf8_lossy(&run.stdout);
        let stderr_text = String::from_utf8_lossy(&run.stderr);
        let stdout_bytes = run.stdout.len();
        let stderr_bytes = run.stderr.len();
        let total_bytes = stdout_bytes + stderr_bytes;
        let stdout_lines = stdout_text.lines().count();
        let stderr_lines = stderr_text.lines().count();
        let total_lines = stdout_lines + stderr_lines;
        let tokens_before =
            TokenMeter::estimate_text(&stdout_text) + TokenMeter::estimate_text(&stderr_text);

        let high_signal_count = stdout_text
            .lines()
            .chain(stderr_text.lines())
            .filter(|line| is_high_signal(line))
            .count();

        let should_compact = run.exit_code != 0
            || total_bytes > budget.max_bytes
            || total_lines > budget.max_lines
            || high_signal_count > 0;

        // Build compact stdout/stderr.
        let (stdout_compact, _) = compact_stream(&stdout_text, run.exit_code, &budget);
        let (stderr_compact, _) = compact_stream(&stderr_text, run.exit_code, &budget);

        let mut summary = String::new();
        summary.push_str(&format!("COMMAND {}\n", meta.command));
        summary.push_str(&format!("exit: {}\n", meta.exit_code));
        summary.push_str(&format!("adapter: {}\n", self.name()));
        summary.push_str(&format!(
            "stdout: {} lines, {} bytes\n",
            stdout_lines, stdout_bytes
        ));
        summary.push_str(&format!(
            "stderr: {} lines, {} bytes\n",
            stderr_lines, stderr_bytes
        ));

        let mut body = String::new();
        if !stdout_compact.is_empty() {
            body.push_str("\nstdout:\n");
            body.push_str(&stdout_compact);
            if !stdout_compact.ends_with('\n') {
                body.push('\n');
            }
        }
        if !stderr_compact.is_empty() {
            body.push_str("\nstderr:\n");
            body.push_str(&stderr_compact);
            if !stderr_compact.ends_with('\n') {
                body.push('\n');
            }
        }
        let high_signal_cap = (budget.failure_max_lines as f64 * 0.35) as usize;
        let high_signal_cap = high_signal_cap.max(10);
        let high_signal_block = collect_high_signal(&stdout_text, &stderr_text, high_signal_cap);
        if !high_signal_block.is_empty() {
            body.push_str("\nhigh-signal:\n");
            body.push_str(&high_signal_block);
            if !high_signal_block.ends_with('\n') {
                body.push('\n');
            }
        }

        let combined_for_size = format!("{summary}{body}");
        let tokens_after = TokenMeter::estimate_text(&combined_for_size);
        let estimated_tokens_saved = tokens_before as isize - tokens_after as isize;
        let savings_pct = if tokens_before > 0 {
            (estimated_tokens_saved as f64 / tokens_before as f64) * 100.0
        } else {
            0.0
        };

        if !should_compact {
            // Passthrough but still emit a summary line + savings note for parity.
            let mut header = String::new();
            header.push_str(&format!(
                "COMMAND {}\nexit: {}\nadapter: {}\n",
                meta.command,
                meta.exit_code,
                self.name()
            ));
            header.push_str(&raw_recovery_line(&meta.raw_id));
            header.push('\n');
            header.push_str(&savings_line(0, 0.0));
            header.push('\n');
            return CompactResult {
                adapter_name: self.name().to_string(),
                compacted: false,
                summary: header,
                stdout: stdout_text.to_string(),
                stderr: stderr_text.to_string(),
                raw_id: meta.raw_id.clone(),
                raw_path: meta.raw_path.clone(),
                compact_path: meta.compact_path.clone(),
                original_stdout_bytes: stdout_bytes,
                original_stderr_bytes: stderr_bytes,
                compact_stdout_bytes: stdout_text.len(),
                compact_stderr_bytes: stderr_text.len(),
                estimated_tokens_before: tokens_before,
                estimated_tokens_after: tokens_before,
                estimated_tokens_saved: 0,
                savings_pct: 0.0,
                high_signal_count,
                warnings: Vec::new(),
            };
        }

        summary.push_str(&body);
        summary.push('\n');
        summary.push_str(&raw_recovery_line(&meta.raw_id));
        summary.push('\n');
        summary.push_str(&savings_line(estimated_tokens_saved, savings_pct));

        // The summary already contains the full compact body; leave stdout/stderr
        // empty so callers don't duplicate the body when they also print the
        // summary block.
        CompactResult {
            adapter_name: self.name().to_string(),
            compacted: true,
            summary,
            stdout: String::new(),
            stderr: String::new(),
            raw_id: meta.raw_id.clone(),
            raw_path: meta.raw_path.clone(),
            compact_path: meta.compact_path.clone(),
            original_stdout_bytes: stdout_bytes,
            original_stderr_bytes: stderr_bytes,
            compact_stdout_bytes: stdout_compact.len(),
            compact_stderr_bytes: stderr_compact.len(),
            estimated_tokens_before: tokens_before,
            estimated_tokens_after: tokens_after,
            estimated_tokens_saved,
            savings_pct,
            high_signal_count,
            warnings: Vec::new(),
        }
    }
}

fn compact_stream(text: &str, exit_code: i32, budget: &OutputBudget) -> (String, usize) {
    if text.is_empty() {
        return (String::new(), 0);
    }
    // Apply full normalisation: strip ANSI → global dedup → collapse repeats → blank lines.
    let normalised = normalise_for_compression(text, budget.per_group_limit);
    let total_cap = if exit_code != 0 {
        budget.failure_max_lines
    } else {
        budget.max_lines
    };
    // Reserve room for the high-signal block and summary header so the total
    // compact body stays under `total_cap` lines.
    let cap = (total_cap as f64 * 0.45) as usize;
    let cap = cap.max(10).min(total_cap.saturating_sub(5));
    let (snapshot, included) = head_tail_snapshot(&normalised, cap, budget.max_bytes);
    let original_line_count = text.lines().count();
    (snapshot, original_line_count.saturating_sub(included))
}

fn collect_high_signal(stdout: &str, stderr: &str, cap: usize) -> String {
    let mut output = String::new();
    let mut shown = 0usize;
    for source in [stdout, stderr] {
        for line in source.lines() {
            if shown >= cap {
                break;
            }
            if is_high_signal(line) {
                output.push_str(line);
                output.push('\n');
                shown += 1;
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::adapter::{OutputBudget, RawRun, RunMeta};
    use crate::proxy::command_ast::build_ast;
    use std::path::PathBuf;

    fn meta(raw_id: &str) -> RunMeta {
        RunMeta {
            raw_id: raw_id.to_string(),
            command: "fake".to_string(),
            cwd: PathBuf::from("/tmp"),
            started_at_unix_ms: 0,
            duration_ms: 0,
            exit_code: 0,
            adapter_name: "generic".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".into(),
            target_agent: "windsurf".into(),
            invoked_as_shim: None,
            wf_core_version: "test".into(),
        }
    }

    #[test]
    fn ten_thousand_lines_compact_below_two_hundred() {
        let stdout: Vec<u8> = (0..10_000)
            .map(|i| format!("line {i}\n"))
            .collect::<String>()
            .into_bytes();
        let run = RawRun {
            stdout,
            stderr: b"ERROR: something exploded\n".to_vec(),
            exit_code: 1,
            duration_ms: 50,
        };
        let ast = build_ast(
            &["pytest".to_string(), "tests".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let result = GenericAdapter.compact(&ast, &run, &meta("rid"), OutputBudget::default());
        assert!(result.compacted);
        let lines = result.summary.lines().count();
        assert!(lines < 200, "expected <200 lines, got {lines}");
        assert!(result.summary.contains("ERROR"));
        assert!(result.summary.contains("raw: wf-core raw rid"));
        let pct =
            (result.estimated_tokens_saved as f64 / result.estimated_tokens_before as f64) * 100.0;
        assert!(pct >= 80.0, "expected >=80% savings, got {pct}");
    }

    #[test]
    fn small_output_is_passthrough_with_summary() {
        let run = RawRun {
            stdout: b"hello\n".to_vec(),
            stderr: vec![],
            exit_code: 0,
            duration_ms: 1,
        };
        let ast = build_ast(
            &["echo".to_string(), "hello".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let result = GenericAdapter.compact(&ast, &run, &meta("rid"), OutputBudget::default());
        assert!(!result.compacted);
        assert_eq!(result.original_stdout_bytes, 6);
    }
}
