use crate::proxy::adapter::{CompactResult, RawRun, RunMeta};
use crate::proxy::render::{raw_recovery_line, savings_line};
use crate::proxy::token_meter::TokenMeter;

/// Per-adapter rendering scaffolding. Builds the `summary` body and stamps the
/// trailing `raw:` and `saved:` lines so the on-the-wire shape is identical
/// across every adapter. Adapters compose their domain-specific content via
/// `push_line` / `push_block`, then call `finish` to compute token savings and
/// return a `CompactResult`.
pub(crate) struct AdapterBuild {
    pub adapter_name: &'static str,
    pub body: String,
    pub high_signal_count: usize,
}

impl AdapterBuild {
    pub fn new(adapter_name: &'static str) -> Self {
        Self {
            adapter_name,
            body: String::new(),
            high_signal_count: 0,
        }
    }

    pub fn push_line(&mut self, value: impl AsRef<str>) {
        let value = value.as_ref();
        self.body.push_str(value);
        if !value.ends_with('\n') {
            self.body.push('\n');
        }
    }

    pub fn push_block(&mut self, value: impl AsRef<str>) {
        let value = value.as_ref();
        if value.is_empty() {
            return;
        }
        self.body.push_str(value);
        if !value.ends_with('\n') {
            self.body.push('\n');
        }
    }

    pub fn finish(self, run: &RawRun, meta: &RunMeta, compacted: bool) -> CompactResult {
        let tokens_before =
            TokenMeter::estimate_bytes(&run.stdout) + TokenMeter::estimate_bytes(&run.stderr);
        let mut summary = String::new();
        summary.push_str(&self.body);
        if !summary.ends_with('\n') {
            summary.push('\n');
        }
        summary.push_str(&raw_recovery_line(&meta.raw_id));
        summary.push('\n');
        let tokens_after = TokenMeter::estimate_text(&summary);
        let estimated_tokens_saved = tokens_before as isize - tokens_after as isize;
        let savings_pct = if tokens_before > 0 {
            (estimated_tokens_saved as f64 / tokens_before as f64) * 100.0
        } else {
            0.0
        };
        summary.push_str(&savings_line(estimated_tokens_saved, savings_pct));
        // Adapters embed the full compact body in `summary` and leave
        // stdout/stderr empty so callers don't double-print. This mirrors the
        // contract followed by GenericAdapter.
        CompactResult {
            adapter_name: self.adapter_name.to_string(),
            compacted,
            summary,
            stdout: String::new(),
            stderr: String::new(),
            raw_id: meta.raw_id.clone(),
            raw_path: meta.raw_path.clone(),
            compact_path: meta.compact_path.clone(),
            original_stdout_bytes: run.stdout.len(),
            original_stderr_bytes: run.stderr.len(),
            compact_stdout_bytes: 0,
            compact_stderr_bytes: 0,
            estimated_tokens_before: tokens_before,
            estimated_tokens_after: tokens_after,
            estimated_tokens_saved,
            savings_pct,
            high_signal_count: self.high_signal_count,
            warnings: Vec::new(),
        }
    }
}
