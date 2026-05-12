use std::fs;
use std::path::{Path, PathBuf};

use crate::AppError;

use super::adapter::CompactResult;
use super::command_ast::CommandAst;
use super::raw_store::{raw_store_root, ProxyTarget};

/// Path to the gain events JSONL file for the chosen target.
pub fn gain_events_path(channel: &str, target: ProxyTarget) -> Result<PathBuf, AppError> {
    let root_parent = raw_store_root(channel, target)?;
    let proxy_root = root_parent.parent().unwrap_or(&root_parent).to_path_buf();
    Ok(proxy_root.join("gain").join("events.jsonl"))
}

/// Record a single proxy run as a JSONL event (schemaVersion=2).
pub fn record_event(
    channel: &str,
    target: ProxyTarget,
    ast: &CommandAst,
    result: &CompactResult,
    exit_code: i32,
    started_at_unix_ms: u128,
    duration_ms: u128,
    invoked_as_shim: Option<&str>,
) -> Result<(), AppError> {
    let path = gain_events_path(channel, target)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let event = format!(
        "{{\"schemaVersion\":2,\"timestampUnixMs\":{ts},\"rawId\":{raw_id},\"command\":{command},\"cwd\":{cwd},\"exitCode\":{exit_code},\"adapterName\":{adapter},\"kind\":{kind},\"compacted\":{compacted},\"stdoutBytesBefore\":{stdout_before},\"stderrBytesBefore\":{stderr_before},\"stdoutBytesAfter\":{stdout_after},\"stderrBytesAfter\":{stderr_after},\"estimatedTokensBefore\":{tokens_before},\"estimatedTokensAfter\":{tokens_after},\"estimatedTokensSaved\":{tokens_saved},\"savingsPct\":{savings_pct:.4},\"durationMs\":{duration},\"invokedAsShim\":{shim}}}\n",
        ts = started_at_unix_ms,
        raw_id = crate::json_string(&result.raw_id),
        command = crate::json_string(&ast.original_command),
        cwd = crate::json_string(&crate::display_path(&ast.cwd)),
        exit_code = exit_code,
        adapter = crate::json_string(&result.adapter_name),
        kind = crate::json_string(ast.detected_kind.as_str()),
        compacted = result.compacted,
        stdout_before = result.original_stdout_bytes,
        stderr_before = result.original_stderr_bytes,
        stdout_after = result.compact_stdout_bytes,
        stderr_after = result.compact_stderr_bytes,
        tokens_before = result.estimated_tokens_before,
        tokens_after = result.estimated_tokens_after,
        tokens_saved = result.estimated_tokens_saved,
        savings_pct = result.savings_pct,
        duration = duration_ms,
        shim = match invoked_as_shim {
            Some(value) => crate::json_string(value),
            None => "null".to_string(),
        },
    );
    crate::append_text(&path, &event)?;
    Ok(())
}

/// A parsed proxy event used by `wf-core gain` and `wf-core discover`.
#[derive(Debug, Clone, Default)]
pub struct GainEventV2 {
    pub schema_version: u32,
    pub timestamp_unix_ms: u128,
    pub raw_id: String,
    pub command: String,
    pub cwd: String,
    pub exit_code: i32,
    pub adapter_name: String,
    pub kind: String,
    pub compacted: bool,
    pub stdout_bytes_before: usize,
    pub stderr_bytes_before: usize,
    pub stdout_bytes_after: usize,
    pub stderr_bytes_after: usize,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub estimated_tokens_saved: isize,
    pub savings_pct: f64,
    pub duration_ms: u128,
    pub invoked_as_shim: Option<String>,
}

/// Load events from disk. Tolerates the legacy v1 JSONL shape produced by
/// earlier versions of wf-core: bytes-only events are upgraded to estimated
/// tokens using the same chars/4 approximation.
pub fn read_events(path: &Path) -> Result<Vec<GainEventV2>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for line in content.lines() {
        if let Some(event) = parse_event(line) {
            events.push(event);
        }
    }
    Ok(events)
}

fn parse_event(line: &str) -> Option<GainEventV2> {
    let schema_version = crate::json_number_field(line, "schemaVersion").unwrap_or(1) as u32;
    let compacted = crate::json_bool_field(line, "compacted").unwrap_or(false);
    // v2 events use millisecond timestamps; the legacy v1 `time` field is in
    // seconds since epoch, so multiply by 1000 when falling back.
    let timestamp_unix_ms = crate::json_number_field(line, "timestampUnixMs")
        .map(|value| value as u128)
        .or_else(|| crate::json_number_field(line, "time").map(|value| (value as u128) * 1000))
        .unwrap_or(0);
    let raw_id = crate::json_string_field(line, "rawId").unwrap_or_default();
    let command = crate::json_string_field(line, "command").unwrap_or_default();
    let cwd = crate::json_string_field(line, "cwd").unwrap_or_default();
    let exit_code = crate::json_number_field(line, "exitCode").unwrap_or(0) as i32;
    let adapter_name =
        crate::json_string_field(line, "adapterName").unwrap_or_else(|| "generic".to_string());
    let kind = crate::json_string_field(line, "kind").unwrap_or_else(|| "Unknown".to_string());

    let stdout_bytes_before = crate::json_number_field(line, "stdoutBytesBefore")
        .or_else(|| crate::json_number_field(line, "rawBytes"))
        .unwrap_or(0);
    let stderr_bytes_before = crate::json_number_field(line, "stderrBytesBefore").unwrap_or(0);
    let stdout_bytes_after = crate::json_number_field(line, "stdoutBytesAfter")
        .or_else(|| crate::json_number_field(line, "compactedBytes"))
        .unwrap_or(0);
    let stderr_bytes_after = crate::json_number_field(line, "stderrBytesAfter").unwrap_or(0);

    let estimated_tokens_before = crate::json_number_field(line, "estimatedTokensBefore")
        .unwrap_or((stdout_bytes_before + stderr_bytes_before + 3) / 4);
    let estimated_tokens_after = crate::json_number_field(line, "estimatedTokensAfter")
        .unwrap_or((stdout_bytes_after + stderr_bytes_after + 3) / 4);
    let estimated_tokens_saved = estimated_tokens_before as isize - estimated_tokens_after as isize;
    let savings_pct = if estimated_tokens_before > 0 {
        (estimated_tokens_saved as f64 / estimated_tokens_before as f64) * 100.0
    } else {
        0.0
    };
    let duration_ms = crate::json_number_field(line, "durationMs").unwrap_or(0) as u128;
    let invoked_as_shim = crate::json_string_field(line, "invokedAsShim");

    Some(GainEventV2 {
        schema_version,
        timestamp_unix_ms,
        raw_id,
        command,
        cwd,
        exit_code,
        adapter_name,
        kind,
        compacted,
        stdout_bytes_before,
        stderr_bytes_before,
        stdout_bytes_after,
        stderr_bytes_after,
        estimated_tokens_before,
        estimated_tokens_after,
        estimated_tokens_saved,
        savings_pct,
        duration_ms,
        invoked_as_shim,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn parses_v2_event() {
        let line = "{\"schemaVersion\":2,\"timestampUnixMs\":42,\"rawId\":\"id\",\"command\":\"echo hi\",\"cwd\":\"/x\",\"exitCode\":0,\"adapterName\":\"tests\",\"kind\":\"Test\",\"compacted\":true,\"stdoutBytesBefore\":1000,\"stderrBytesBefore\":50,\"stdoutBytesAfter\":80,\"stderrBytesAfter\":5,\"estimatedTokensBefore\":300,\"estimatedTokensAfter\":21,\"estimatedTokensSaved\":279,\"savingsPct\":93.0,\"durationMs\":11,\"invokedAsShim\":null}";
        let event = parse_event(line).unwrap();
        assert_eq!(event.schema_version, 2);
        assert_eq!(event.adapter_name, "tests");
        assert_eq!(event.estimated_tokens_before, 300);
        assert!(event.compacted);
    }

    #[test]
    fn parses_legacy_v1_event_and_upgrades_tokens() {
        let line = "{\"time\":1,\"command\":\"echo hi\",\"exitCode\":0,\"compacted\":true,\"rawBytes\":400,\"compactedBytes\":40,\"savedBytes\":360,\"highSignalCount\":3,\"rawPath\":\"/tmp/raw\"}";
        let event = parse_event(line).unwrap();
        assert_eq!(event.schema_version, 1);
        assert_eq!(event.estimated_tokens_before, 100); // 400/4
        assert_eq!(event.estimated_tokens_after, 10);
        assert_eq!(event.estimated_tokens_saved, 90);
    }

    #[test]
    fn upgrades_legacy_v1_timestamp_from_seconds_to_milliseconds() {
        let line = "{\"time\":1700000000,\"command\":\"echo hi\",\"exitCode\":0,\"compacted\":true,\"rawBytes\":400,\"compactedBytes\":40,\"savedBytes\":360,\"highSignalCount\":3,\"rawPath\":\"/tmp/raw\"}";
        let event = parse_event(line).unwrap();
        assert_eq!(event.timestamp_unix_ms, 1_700_000_000_000);
    }

    #[test]
    fn read_events_handles_missing_file() {
        let scratch = env::temp_dir().join(format!(
            "wf-core-events-{}",
            super::super::raw_store::now_unix_ms()
        ));
        let path = scratch.join("events.jsonl");
        assert!(read_events(&path).unwrap().is_empty());
        let _ = fs::remove_dir_all(&scratch);
    }
}
