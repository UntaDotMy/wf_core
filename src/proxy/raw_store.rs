use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppError;

use super::adapter::{CompactResult, RawRun, RunMeta};

const RAW_ID_HEX_LEN: usize = 8;

/// Build the raw store root for the given proxy target.
///
/// Selection order: `WF_CORE_HOME` env wins (gives `$WF_CORE_HOME/raw-output`),
/// otherwise Devin gets `<devin_home>/wf-core/raw-output`, otherwise the
/// Windsurf channel home is used.
pub fn raw_store_root(channel: &str, target: ProxyTarget) -> Result<PathBuf, AppError> {
    if let Some(forced) = forced_root_from_env()? {
        return Ok(forced.join("raw-output"));
    }
    match target {
        ProxyTarget::Windsurf => Ok(crate::channel_home(channel)?
            .join("wf-core")
            .join("raw-output")),
        ProxyTarget::Devin => Ok(crate::devin_home()?.join("wf-core").join("raw-output")),
    }
}

fn forced_root_from_env() -> Result<Option<PathBuf>, AppError> {
    if let Ok(value) = std::env::var("WF_CORE_HOME") {
        if !value.trim().is_empty() {
            return Ok(Some(PathBuf::from(value)));
        }
    }
    Ok(None)
}

/// Logical agent home used for raw storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyTarget {
    Windsurf,
    Devin,
}

impl ProxyTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyTarget::Windsurf => "windsurf",
            ProxyTarget::Devin => "devin",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "devin" | "Devin" | "devin-local" => ProxyTarget::Devin,
            _ => ProxyTarget::Windsurf,
        }
    }
}

/// Build a new raw run identifier: `YYYYMMDD-HHMMSS-<hex>`.
pub fn new_raw_id(seed: &str, now_ms: u128) -> String {
    let secs = (now_ms / 1000) as i64;
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(secs);
    let hash = short_hash(format!("{seed}-{now_ms}").as_bytes(), RAW_ID_HEX_LEN);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{hash}")
}

/// Persist raw stdout/stderr + meta + compact for a single run.
pub fn save_run(
    raw_root: &Path,
    raw_id: &str,
    command: &str,
    argv: &[String],
    shell_mode: bool,
    run: &RawRun,
) -> Result<(PathBuf, PathBuf, PathBuf), AppError> {
    let dir = raw_dir(raw_root, raw_id);
    fs::create_dir_all(&dir)?;
    let stdout_path = dir.join("stdout.log");
    let stderr_path = dir.join("stderr.log");
    fs::write(&stdout_path, &run.stdout)?;
    fs::write(&stderr_path, &run.stderr)?;
    fs::write(dir.join("command.txt"), command)?;
    let args_json = argv
        .iter()
        .map(|a| crate::json_string(a))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        dir.join("args.json"),
        format!(
            "{{\n  \"shellMode\": {},\n  \"argv\": [{}]\n}}\n",
            shell_mode, args_json
        ),
    )?;
    let compact_path = dir.join("compact.txt");
    Ok((stdout_path, compact_path, dir))
}

/// Read the argv list and shell_mode flag previously saved by `save_run`.
pub fn load_args(args_path: &Path) -> Result<(Vec<String>, bool), AppError> {
    let content = fs::read_to_string(args_path)?;
    let shell_mode = crate::json_bool_field(&content, "shellMode").unwrap_or(false);
    let argv = parse_string_array(&content, "argv");
    Ok((argv, shell_mode))
}

fn parse_string_array(content: &str, field: &str) -> Vec<String> {
    let needle = format!("\"{field}\"");
    let Some(start) = content.find(&needle) else {
        return Vec::new();
    };
    let Some(open) = content[start..].find('[') else {
        return Vec::new();
    };
    let open_idx = start + open + 1;
    let Some(close_rel) = content[open_idx..].find(']') else {
        return Vec::new();
    };
    let body = &content[open_idx..open_idx + close_rel];
    let mut values = Vec::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut item = String::new();
            while let Some(next) = chars.next() {
                if next == '\\' {
                    if let Some(after) = chars.next() {
                        match after {
                            '"' => item.push('"'),
                            '\\' => item.push('\\'),
                            '/' => item.push('/'),
                            'n' => item.push('\n'),
                            't' => item.push('\t'),
                            'r' => item.push('\r'),
                            'b' => item.push('\u{08}'),
                            'f' => item.push('\u{0c}'),
                            _ => item.push(after),
                        }
                    }
                } else if next == '"' {
                    break;
                } else {
                    item.push(next);
                }
            }
            values.push(item);
        }
    }
    values
}

pub fn write_compact(compact_path: &Path, content: &str) -> Result<(), AppError> {
    if let Some(parent) = compact_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(compact_path, content)?;
    Ok(())
}

pub fn write_meta_json(
    meta_path: &Path,
    meta: &RunMeta,
    result: &CompactResult,
) -> Result<(), AppError> {
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = format!(
        "{{\n  \"schemaVersion\": 2,\n  \"rawId\": {},\n  \"command\": {},\n  \"cwd\": {},\n  \"exitCode\": {},\n  \"adapterName\": {},\n  \"startedAtUnixMs\": {},\n  \"durationMs\": {},\n  \"stdoutBytes\": {},\n  \"stderrBytes\": {},\n  \"compactStdoutBytes\": {},\n  \"compactStderrBytes\": {},\n  \"estimatedTokensBefore\": {},\n  \"estimatedTokensAfter\": {},\n  \"estimatedTokensSaved\": {},\n  \"savingsPct\": {:.4},\n  \"compacted\": {},\n  \"wfCoreVersion\": {},\n  \"channel\": {},\n  \"targetAgent\": {},\n  \"invokedAsShim\": {}\n}}\n",
        crate::json_string(&meta.raw_id),
        crate::json_string(&meta.command),
        crate::json_string(&crate::display_path(&meta.cwd)),
        meta.exit_code,
        crate::json_string(&meta.adapter_name),
        meta.started_at_unix_ms,
        meta.duration_ms,
        result.original_stdout_bytes,
        result.original_stderr_bytes,
        result.compact_stdout_bytes,
        result.compact_stderr_bytes,
        result.estimated_tokens_before,
        result.estimated_tokens_after,
        result.estimated_tokens_saved,
        result.savings_pct,
        result.compacted,
        crate::json_string(&meta.wf_core_version),
        crate::json_string(&meta.channel),
        crate::json_string(&meta.target_agent),
        match &meta.invoked_as_shim {
            Some(value) => crate::json_string(value),
            None => "null".to_string(),
        },
    );
    fs::write(meta_path, json)?;
    Ok(())
}

/// Directory layout: `<root>/YYYY-MM-DD/<raw_id>/`.
pub fn raw_dir(root: &Path, raw_id: &str) -> PathBuf {
    let date = raw_id
        .split_once('-')
        .map(|(d, _)| d.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    // Reformat YYYYMMDD -> YYYY-MM-DD for friendly grouping.
    let pretty = if date.len() == 8 {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..8])
    } else {
        date
    };
    root.join(pretty).join(raw_id)
}

/// Look up the directory for an existing raw_id by scanning the date layout.
pub fn find_raw_dir(root: &Path, raw_id: &str) -> Result<PathBuf, AppError> {
    if !root.exists() {
        return Err(AppError::new(format!(
            "raw store {} does not exist",
            crate::display_path(root)
        )));
    }
    let candidate = raw_dir(root, raw_id);
    if candidate.exists() {
        return Ok(candidate);
    }
    // Fall back to scanning in case raw_id was produced with an unusual date prefix.
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let inner = path.join(raw_id);
        if inner.exists() {
            return Ok(inner);
        }
    }
    Err(AppError::new(format!("raw run {raw_id} not found")))
}

/// Describe a single raw run for `wf-core raw list`.
#[derive(Debug, Clone)]
pub struct RawRunSummary {
    pub raw_id: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub started_at_unix_ms: u128,
    pub path: PathBuf,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

/// List raw runs newest-first, scanning at most `limit` matching ids.
pub fn list_runs(root: &Path, limit: usize) -> Result<Vec<RawRunSummary>, AppError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<PathBuf> = Vec::new();
    for date_entry in fs::read_dir(root)? {
        let date_entry = date_entry?;
        if !date_entry.path().is_dir() {
            continue;
        }
        for inner in fs::read_dir(date_entry.path())? {
            let inner = inner?;
            if inner.path().is_dir() {
                entries.push(inner.path());
            }
        }
    }
    entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut summaries = Vec::new();
    for path in entries.into_iter().take(limit) {
        let raw_id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let command = fs::read_to_string(path.join("command.txt")).unwrap_or_default();
        let (exit_code, started_at_unix_ms) = read_meta_fields(&path.join("meta.json"));
        let stdout_bytes = fs::metadata(path.join("stdout.log"))
            .map(|m| m.len())
            .unwrap_or(0);
        let stderr_bytes = fs::metadata(path.join("stderr.log"))
            .map(|m| m.len())
            .unwrap_or(0);
        summaries.push(RawRunSummary {
            raw_id,
            command,
            exit_code,
            started_at_unix_ms,
            path,
            stdout_bytes,
            stderr_bytes,
        });
    }
    Ok(summaries)
}

fn read_meta_fields(meta_path: &Path) -> (Option<i32>, u128) {
    let Ok(content) = fs::read_to_string(meta_path) else {
        return (None, 0);
    };
    let exit_code = crate::json_number_field(&content, "exitCode").map(|v| v as i32);
    let started_at = crate::json_number_field(&content, "startedAtUnixMs").unwrap_or(0) as u128;
    (exit_code, started_at)
}

/// Remove raw runs older than the cutoff (millis since epoch). Returns the
/// number of run directories deleted.
pub fn prune_older_than(root: &Path, cutoff_unix_ms: u128) -> Result<usize, AppError> {
    if !root.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for date_entry in fs::read_dir(root)? {
        let date_entry = date_entry?;
        if !date_entry.path().is_dir() {
            continue;
        }
        for inner in fs::read_dir(date_entry.path())? {
            let inner = inner?;
            let path = inner.path();
            if !path.is_dir() {
                continue;
            }
            let started = read_meta_fields(&path.join("meta.json")).1;
            let modified = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let ts = if started > 0 { started } else { modified };
            if ts > 0 && ts < cutoff_unix_ms {
                fs::remove_dir_all(&path).ok();
                removed += 1;
            }
        }
        // Best-effort cleanup of empty date directories.
        if fs::read_dir(date_entry.path())
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
        {
            fs::remove_dir(date_entry.path()).ok();
        }
    }
    Ok(removed)
}

/// Translate a duration string like `30d` or `12h` into milliseconds. Returns
/// `None` if the value cannot be parsed.
pub fn parse_duration_ms(value: &str) -> Option<u128> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value == "today" {
        return Some(24 * 60 * 60 * 1000);
    }
    let (num, unit) = value.split_at(value.len() - 1);
    let unit = unit.to_ascii_lowercase();
    let amount: u128 = num.parse().ok()?;
    let factor: u128 = match unit.as_str() {
        "s" => 1_000,
        "m" => 60 * 1_000,
        "h" => 60 * 60 * 1_000,
        "d" => 24 * 60 * 60 * 1_000,
        "w" => 7 * 24 * 60 * 60 * 1_000,
        _ => return None,
    };
    Some(amount * factor)
}

pub fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Encode a YYYY-MM-DD timestamp into year/month/day/hour/minute/second.
fn unix_to_ymdhms(secs: i64) -> (u32, u32, u32, u32, u32, u32) {
    // Algorithm adapted from Howard Hinnant's `civil_from_days`.
    let days = secs.div_euclid(86_400);
    let mut secs_of_day = secs.rem_euclid(86_400);
    let h = (secs_of_day / 3600) as u32;
    secs_of_day %= 3600;
    let mi = (secs_of_day / 60) as u32;
    let s = (secs_of_day % 60) as u32;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe + era * 400) as i64;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (y + i64::from(month <= 2)) as u32;
    (year, month, d, h, mi, s)
}

fn short_hash(bytes: &[u8], length: usize) -> String {
    let mut hash: u64 = 1_469_598_103_934_665_603;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    let hex = format!("{hash:016x}");
    hex.chars().take(length).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn new_raw_id_uses_yyyymmdd_prefix() {
        // 2026-05-12 00:30:44 UTC -> seconds = 1778575844
        let raw_id = new_raw_id("seed", 1_778_575_844_000);
        assert!(raw_id.starts_with("20260512-"));
        assert_eq!(raw_id.chars().filter(|c| *c == '-').count(), 2);
    }

    #[test]
    fn parse_duration_handles_common_units() {
        assert_eq!(parse_duration_ms("30s"), Some(30_000));
        assert_eq!(parse_duration_ms("1h"), Some(3_600_000));
        assert_eq!(parse_duration_ms("7d"), Some(7 * 86_400_000));
        assert_eq!(parse_duration_ms("today"), Some(86_400_000));
        assert!(parse_duration_ms("bogus").is_none());
    }

    #[test]
    fn save_and_list_roundtrip() {
        let scratch = env::temp_dir().join(format!("wf-core-raw-test-{}", now_unix_ms()));
        let root = scratch.join("raw-output");
        let raw_id = new_raw_id("seed", now_unix_ms());
        let run = RawRun {
            stdout: b"hello\n".to_vec(),
            stderr: b"warn\n".to_vec(),
            exit_code: 0,
            duration_ms: 5,
        };
        save_run(
            &root,
            &raw_id,
            "echo hello",
            &["echo".to_string(), "hello".to_string()],
            false,
            &run,
        )
        .unwrap();
        let runs = list_runs(&root, 5).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].raw_id, raw_id);
        let _ = fs::remove_dir_all(&scratch);
    }
}
