use std::path::{Path, PathBuf};

use super::command_ast::{CommandAst, CommandKind};

/// Output of executing a real command capturing the full raw stdout/stderr.
#[derive(Debug, Clone, Default)]
pub struct RawRun {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub duration_ms: u128,
}

/// Persisted metadata for a single proxy run.
#[derive(Debug, Clone)]
pub struct RunMeta {
    pub raw_id: String,
    pub command: String,
    pub cwd: PathBuf,
    pub started_at_unix_ms: u128,
    pub duration_ms: u128,
    pub exit_code: i32,
    pub adapter_name: String,
    pub raw_path: PathBuf,
    pub compact_path: PathBuf,
    pub channel: String,
    pub target_agent: String,
    pub invoked_as_shim: Option<String>,
    pub wf_core_version: String,
}

impl RunMeta {
    pub fn raw_dir(&self) -> &Path {
        self.raw_path.parent().unwrap_or_else(|| Path::new("."))
    }
}

/// Budgets used by adapters to decide how much output to keep.
#[derive(Debug, Clone, Copy)]
pub struct OutputBudget {
    pub max_lines: usize,
    pub max_bytes: usize,
    pub failure_max_lines: usize,
    pub per_group_limit: usize,
}

impl Default for OutputBudget {
    fn default() -> Self {
        Self {
            max_lines: 120,
            max_bytes: 12_000,
            failure_max_lines: 200,
            per_group_limit: 20,
        }
    }
}

/// Result of compacting a single run with a specific adapter.
#[derive(Debug, Clone)]
pub struct CompactResult {
    pub adapter_name: String,
    pub compacted: bool,
    pub summary: String,
    pub stdout: String,
    pub stderr: String,
    pub raw_id: String,
    pub raw_path: PathBuf,
    pub compact_path: PathBuf,
    pub original_stdout_bytes: usize,
    pub original_stderr_bytes: usize,
    pub compact_stdout_bytes: usize,
    pub compact_stderr_bytes: usize,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub estimated_tokens_saved: isize,
    pub savings_pct: f64,
    pub high_signal_count: usize,
    pub warnings: Vec<String>,
}

impl CompactResult {
    pub fn passthrough(
        adapter_name: &str,
        raw_id: &str,
        raw_path: &Path,
        compact_path: &Path,
        run: &RawRun,
        tokens_before: usize,
    ) -> Self {
        let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
        Self {
            adapter_name: adapter_name.to_string(),
            compacted: false,
            summary: String::new(),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            raw_id: raw_id.to_string(),
            raw_path: raw_path.to_path_buf(),
            compact_path: compact_path.to_path_buf(),
            original_stdout_bytes: run.stdout.len(),
            original_stderr_bytes: run.stderr.len(),
            compact_stdout_bytes: stdout.len(),
            compact_stderr_bytes: stderr.len(),
            estimated_tokens_before: tokens_before,
            estimated_tokens_after: tokens_before,
            estimated_tokens_saved: 0,
            savings_pct: 0.0,
            high_signal_count: 0,
            warnings: Vec::new(),
        }
    }
}

/// Adapter trait. Implementors classify and compact command output.
pub trait CommandAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn kind(&self) -> CommandKind;
    fn matches(&self, ast: &CommandAst) -> bool;
    fn compact(
        &self,
        ast: &CommandAst,
        run: &RawRun,
        meta: &RunMeta,
        budget: OutputBudget,
    ) -> CompactResult;

    /// Optional: adapters can suggest an explicit rewrite of the command.
    fn rewrite_args(&self, _ast: &CommandAst) -> Option<CommandAst> {
        None
    }
}
