use std::env;
use std::path::PathBuf;

use crate::AppError;

use super::adapter::{CompactResult, OutputBudget, RawRun, RunMeta};
use super::command_ast::{build_ast, CommandAst};
use super::event_log;
use super::raw_store::{
    find_raw_dir, new_raw_id, now_unix_ms, raw_store_root, save_run, write_compact,
    write_meta_json, ProxyTarget, RawRunSummary,
};
use super::registry::default_registry;
use super::render::strip_ansi;
use super::safety::{is_interactive_command, redact_secrets};
use super::shell::execute_command_with_env;
use super::token_meter::TokenMeter;

const PROXY_RECURSION_ENV: &str = "WF_CORE_PROXY_ACTIVE";

/// User-facing options for `wf-core run`.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub channel: String,
    pub target: ProxyTarget,
    pub shell_mode: bool,
    pub full: bool,
    pub no_compact: bool,
    pub no_raw: bool,
    pub no_redact: bool,
    pub as_json: bool,
    pub list_adapters: bool,
    pub forced_adapter: Option<String>,
    pub budget: OutputBudget,
    pub invoked_as_shim: Option<String>,
    pub executable_override: Option<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            channel: "next".to_string(),
            target: ProxyTarget::Windsurf,
            shell_mode: false,
            full: false,
            no_compact: false,
            no_raw: false,
            no_redact: false,
            as_json: false,
            list_adapters: false,
            forced_adapter: None,
            budget: OutputBudget::default(),
            invoked_as_shim: None,
            executable_override: None,
        }
    }
}

#[derive(Debug)]
pub struct RunReport {
    pub exit_code: i32,
    pub ast: CommandAst,
    pub result: CompactResult,
}

/// Run a command end-to-end through the proxy pipeline.
pub fn run_proxy(command_args: &[String], options: RunOptions) -> Result<RunReport, AppError> {
    if command_args.is_empty() {
        return Err(AppError::new("no command provided"));
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let ast = build_ast(
        command_args,
        cwd.clone(),
        options.shell_mode,
        options.invoked_as_shim.clone(),
    );
    let started_at_unix_ms = now_unix_ms();

    let registry = default_registry();
    let effective_command = if let Some(executable) = &options.executable_override {
        let mut args = Vec::with_capacity(command_args.len());
        args.push(executable.to_string_lossy().to_string());
        args.extend(command_args.iter().skip(1).cloned());
        args
    } else {
        command_args.to_vec()
    };
    let output = execute_command_with_env(
        &effective_command,
        options.shell_mode,
        [
            (PROXY_RECURSION_ENV, "1"),
            ("WF_CORE_CHANNEL", options.channel.as_str()),
        ],
    )?;
    let duration_ms = now_unix_ms().saturating_sub(started_at_unix_ms);
    let exit_code = output.status.code().unwrap_or(1);
    let run = RawRun {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
        duration_ms,
    };

    let raw_id = new_raw_id(&ast.original_command, started_at_unix_ms);
    let mut adapter_name_for_meta = "passthrough".to_string();

    // Decide where to write raw output.
    let raw_root = raw_store_root(&options.channel, options.target)?;
    let (raw_path, compact_path, dir) = if options.no_raw {
        let stub = raw_root.join("disabled").join(&raw_id);
        (stub.join("stdout.log"), stub.join("compact.txt"), stub)
    } else {
        save_run(
            &raw_root,
            &raw_id,
            &ast.original_command,
            command_args,
            options.shell_mode,
            &run,
        )?
    };
    let meta_path = dir.join("meta.json");

    let tokens_before =
        TokenMeter::estimate_bytes(&run.stdout) + TokenMeter::estimate_bytes(&run.stderr);

    // Resolve adapter or passthrough.
    let result = if options.full || options.no_compact {
        let mut result = CompactResult::passthrough(
            "full",
            &raw_id,
            &raw_path,
            &compact_path,
            &run,
            tokens_before,
        );
        result.compacted = false;
        result
    } else if is_interactive_command(&ast.program, &ast.args) {
        let mut result = CompactResult::passthrough(
            "interactive",
            &raw_id,
            &raw_path,
            &compact_path,
            &run,
            tokens_before,
        );
        result
            .warnings
            .push("interactive command detected; not compacting".to_string());
        result.compacted = false;
        result
    } else {
        let adapter = match &options.forced_adapter {
            Some(name) => registry
                .by_name(name)
                .ok_or_else(|| AppError::new(format!("unknown adapter: {name}")))?,
            None => registry.pick(&ast),
        };
        let meta = RunMeta {
            raw_id: raw_id.clone(),
            command: ast.original_command.clone(),
            cwd: cwd.clone(),
            started_at_unix_ms,
            duration_ms,
            exit_code,
            adapter_name: adapter.name().to_string(),
            raw_path: raw_path.clone(),
            compact_path: compact_path.clone(),
            channel: options.channel.clone(),
            target_agent: options.target.as_str().to_string(),
            invoked_as_shim: options.invoked_as_shim.clone(),
            wf_core_version: crate::VERSION.to_string(),
        };
        adapter_name_for_meta = adapter.name().to_string();
        adapter.compact(&ast, &run, &meta, options.budget)
    };

    // Optionally redact possible secrets in the compact text. Raw output is
    // never altered: it is local recovery data. Adapters that embed the full
    // compact body into `summary` (and leave stdout/stderr empty) still get
    // redacted because we run the same scrub on `summary`.
    let mut final_result = result;

    // Strip ANSI escape sequences from all output fields so the compact text
    // is clean before redaction and writing.
    final_result.summary = strip_ansi(&final_result.summary);
    final_result.stdout = strip_ansi(&final_result.stdout);
    final_result.stderr = strip_ansi(&final_result.stderr);

    // Re-count tokens after stripping ANSI so savings reflect the
    // compact text the model actually sees.
    let tokens_after = TokenMeter::estimate_text(&final_result.summary)
        + TokenMeter::estimate_text(&final_result.stdout)
        + TokenMeter::estimate_text(&final_result.stderr);
    final_result.estimated_tokens_after = tokens_after;
    final_result.estimated_tokens_saved =
        final_result.estimated_tokens_before as isize - tokens_after as isize;
    final_result.savings_pct = if final_result.estimated_tokens_before > 0 {
        (final_result.estimated_tokens_saved as f64 / final_result.estimated_tokens_before as f64)
            * 100.0
    } else {
        0.0
    };

    if !options.no_redact {
        let (redacted_summary, _) = redact_secrets(&final_result.summary);
        let (redacted_stdout, _) = redact_secrets(&final_result.stdout);
        let (redacted_stderr, _) = redact_secrets(&final_result.stderr);
        final_result.summary = redacted_summary;
        final_result.stdout = redacted_stdout;
        final_result.stderr = redacted_stderr;
    }

    if !options.no_raw {
        let mut content = String::new();
        if !final_result.summary.is_empty() {
            content.push_str(&final_result.summary);
            content.push('\n');
        }
        if !final_result.stdout.is_empty() {
            content.push_str(&final_result.stdout);
            if !final_result.stdout.ends_with('\n') {
                content.push('\n');
            }
        }
        if !final_result.stderr.is_empty() {
            content.push_str("[stderr]\n");
            content.push_str(&final_result.stderr);
        }
        write_compact(&compact_path, &content)?;
        let meta = RunMeta {
            raw_id: raw_id.clone(),
            command: ast.original_command.clone(),
            cwd: cwd.clone(),
            started_at_unix_ms,
            duration_ms,
            exit_code,
            adapter_name: adapter_name_for_meta.clone(),
            raw_path: raw_path.clone(),
            compact_path: compact_path.clone(),
            channel: options.channel.clone(),
            target_agent: options.target.as_str().to_string(),
            invoked_as_shim: options.invoked_as_shim.clone(),
            wf_core_version: crate::VERSION.to_string(),
        };
        write_meta_json(&meta_path, &meta, &final_result)?;
    }

    event_log::record_event(
        &options.channel,
        options.target,
        &ast,
        &final_result,
        exit_code,
        started_at_unix_ms,
        duration_ms,
        options.invoked_as_shim.as_deref(),
    )?;

    Ok(RunReport {
        exit_code,
        ast,
        result: final_result,
    })
}

/// List raw runs in the configured store, newest first.
pub fn list_raw_runs(
    channel: &str,
    target: ProxyTarget,
    limit: usize,
) -> Result<Vec<RawRunSummary>, AppError> {
    let root = raw_store_root(channel, target)?;
    super::raw_store::list_runs(&root, limit)
}

/// Print the raw recovery contents for a single raw_id.
pub fn print_raw(channel: &str, target: ProxyTarget, raw_id: &str) -> Result<i32, AppError> {
    let root = raw_store_root(channel, target)?;
    let dir = find_raw_dir(&root, raw_id)?;
    println!("[wf-core] raw run {raw_id}");
    println!("path: {}", crate::display_path(&dir));
    let stdout = std::fs::read_to_string(dir.join("stdout.log")).unwrap_or_default();
    let stderr = std::fs::read_to_string(dir.join("stderr.log")).unwrap_or_default();
    if !stdout.is_empty() {
        println!("\n[stdout]");
        print!("{stdout}");
        if !stdout.ends_with('\n') {
            println!();
        }
    }
    if !stderr.is_empty() {
        println!("\n[stderr]");
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
    }
    Ok(0)
}

pub fn print_raw_path(channel: &str, target: ProxyTarget, raw_id: &str) -> Result<i32, AppError> {
    let root = raw_store_root(channel, target)?;
    let dir = find_raw_dir(&root, raw_id)?;
    println!("{}", crate::display_path(&dir));
    Ok(0)
}

pub fn prevent_recursion_active() -> bool {
    matches!(env::var(PROXY_RECURSION_ENV).ok().as_deref(), Some("1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_for_test(scratch: &std::path::Path) -> RunOptions {
        env::set_var("WF_CORE_HOME", scratch);
        RunOptions {
            channel: "next".to_string(),
            ..RunOptions::default()
        }
    }

    #[test]
    fn end_to_end_run_records_raw_and_event() {
        let scratch = env::temp_dir().join(format!("wf-core-run-test-{}", now_unix_ms()));
        let opts = opts_for_test(&scratch);
        let report = run_proxy(&vec!["true".to_string()], opts).unwrap();
        assert_eq!(report.exit_code, 0);
        assert!(report.result.raw_path.exists());
        let events = event_log::read_events(
            &event_log::gain_events_path("next", ProxyTarget::Windsurf).unwrap(),
        )
        .unwrap();
        assert!(!events.is_empty());
        let _ = std::fs::remove_dir_all(&scratch);
        env::remove_var("WF_CORE_HOME");
    }

    #[test]
    fn json_redact_default_strips_secret_lines() {
        let scratch = env::temp_dir().join(format!("wf-core-run-redact-{}", now_unix_ms()));
        env::set_var("WF_CORE_HOME", &scratch);
        let opts = RunOptions {
            channel: "next".to_string(),
            shell_mode: true,
            ..RunOptions::default()
        };
        let report = run_proxy(
            &vec!["printf 'GITHUB_TOKEN=ghp_abcdef\\nok\\n'".to_string()],
            opts,
        )
        .unwrap();
        assert!(report.result.stdout.contains("[redacted possible secret"));
        let _ = std::fs::remove_dir_all(&scratch);
        env::remove_var("WF_CORE_HOME");
    }

    #[test]
    fn redact_default_strips_secrets_from_compacted_summary() {
        let scratch = env::temp_dir().join(format!("wf-core-redact-summary-{}", now_unix_ms()));
        env::set_var("WF_CORE_HOME", &scratch);
        let opts = RunOptions {
            channel: "next".to_string(),
            shell_mode: true,
            ..RunOptions::default()
        };
        // Force compaction by emitting more lines than the default max_lines (120).
        let cmd = "for i in $(seq 1 200); do echo line $i; done; echo GITHUB_TOKEN=ghp_supersecret"
            .to_string();
        let report = run_proxy(&vec![cmd], opts).unwrap();
        assert!(report.result.compacted, "expected output to be compacted");
        assert!(
            report.result.summary.contains("[redacted possible secret"),
            "summary should be redacted, was:\n{}",
            report.result.summary
        );
        assert!(
            !report.result.summary.contains("ghp_supersecret"),
            "secret token leaked in compact summary:\n{}",
            report.result.summary
        );
        let _ = std::fs::remove_dir_all(&scratch);
        env::remove_var("WF_CORE_HOME");
    }
}
