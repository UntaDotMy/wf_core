use std::collections::HashSet;

const HIGH_SIGNAL_TERMS: &[&str] = &[
    "error",
    "errored",
    "failed",
    "failure",
    "fatal",
    "panic",
    "traceback",
    "exception",
    "assert",
    "assertion",
    "warning",
    "denied",
    "cannot",
    "not found",
    "unresolved",
    "expected",
    "actual",
    "timeout",
    "segmentation",
    "killed",
    "oom",
    "out of memory",
    "segfault",
    "permission denied",
];

/// True if this line contains a typical error/warning/failure signal.
pub fn is_high_signal(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    HIGH_SIGNAL_TERMS.iter().any(|term| lowered.contains(term)) || looks_like_file_line(&lowered)
}

/// Heuristic for file:line:col style references.
pub fn looks_like_file_line(line: &str) -> bool {
    // Quick reject for empty.
    if line.is_empty() {
        return false;
    }
    // Look for ":NUMBER" at any boundary which usually signals file:line.
    let mut chars = line.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == ':' && idx + 1 < line.len() {
            let rest = &line[idx + 1..];
            let first = rest.chars().next().unwrap_or(' ');
            if first.is_ascii_digit() {
                // Make sure there's at least 3 chars before the colon, e.g. .py / .rs / .ts
                let before = &line[..idx];
                if before.len() >= 3
                    && (before.contains('.') || before.contains('/') || before.contains('\\'))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Wrap a long body of text into a head/tail snapshot constrained by line and byte budget.
pub fn head_tail_snapshot(text: &str, max_lines: usize, max_bytes: usize) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return (String::new(), 0);
    }
    let mut head_count = max_lines.min(lines.len());
    let mut tail_count = 0usize;
    if lines.len() > max_lines {
        head_count = (max_lines * 6 / 10).max(1);
        tail_count = max_lines.saturating_sub(head_count);
    }
    let mut output = String::new();
    let mut included = 0usize;
    for line in lines.iter().take(head_count) {
        output.push_str(line);
        output.push('\n');
        included += 1;
        if output.len() > max_bytes {
            break;
        }
    }
    if tail_count > 0 {
        let skip = lines.len().saturating_sub(tail_count);
        output.push_str(&format!(
            "... ({} lines elided)\n",
            skip.saturating_sub(head_count)
        ));
        for line in lines.iter().skip(skip) {
            output.push_str(line);
            output.push('\n');
            included += 1;
            if output.len() > max_bytes {
                break;
            }
        }
    } else if lines.len() > head_count {
        output.push_str(&format!(
            "... ({} lines elided)\n",
            lines.len() - head_count
        ));
    }
    (output, included)
}

/// Deduplicate consecutive identical lines and return the reduced text plus the
/// number of duplicates collapsed (for reporting).
pub fn collapse_repeats(text: &str, per_group_limit: usize) -> (String, usize) {
    let mut output = String::new();
    let mut previous: Option<String> = None;
    let mut repeat_count = 0usize;
    let mut collapsed_total = 0usize;
    for line in text.lines() {
        match &previous {
            Some(prev) if prev == line => {
                repeat_count += 1;
                if repeat_count < per_group_limit {
                    output.push_str(line);
                    output.push('\n');
                } else if repeat_count == per_group_limit {
                    output.push_str("... (repeating line collapsed)\n");
                    collapsed_total += 1;
                } else {
                    collapsed_total += 1;
                }
            }
            _ => {
                previous = Some(line.to_string());
                repeat_count = 0;
                output.push_str(line);
                output.push('\n');
            }
        }
    }
    (output, collapsed_total)
}

/// Render a single line that always includes the raw recovery id, and an
/// estimated savings note.
pub fn raw_recovery_line(raw_id: &str) -> String {
    format!("raw: wf-core raw {raw_id}")
}

pub fn savings_line(tokens_saved: isize, savings_pct: f64) -> String {
    let pct = if savings_pct.is_finite() {
        savings_pct
    } else {
        0.0
    };
    format!(
        "saved: {} tokens estimated ({:.1}%)",
        format_signed_count(tokens_saved),
        pct
    )
}

pub fn format_signed_count(value: isize) -> String {
    let mut absolute = value.unsigned_abs() as u128;
    if absolute == 0 {
        return "0".to_string();
    }
    let mut groups: Vec<String> = Vec::new();
    while absolute > 0 {
        let chunk = (absolute % 1000) as u32;
        groups.push(format!("{chunk:03}"));
        absolute /= 1000;
    }
    let mut joined = groups.into_iter().rev().collect::<Vec<_>>().join(",");
    // Trim leading zeros from the first group.
    while joined.starts_with('0') && joined.len() > 1 && joined.chars().nth(1) != Some(',') {
        joined.remove(0);
    }
    if value < 0 {
        joined.insert(0, '-');
    }
    joined
}

/// Deduplicate lines while preserving order.
pub fn dedupe_lines(text: &str) -> String {
    let mut seen: HashSet<String> = HashSet::new();
    let mut output = String::new();
    for line in text.lines() {
        if seen.insert(line.to_string()) {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_signal_terms_are_detected() {
        assert!(is_high_signal("ERROR: thing exploded"));
        assert!(is_high_signal("AssertionError: nope"));
        assert!(is_high_signal("traceback (most recent call last)"));
        assert!(!is_high_signal("ok 12 ms passed"));
    }

    #[test]
    fn file_line_pattern_is_detected() {
        assert!(is_high_signal("src/main.rs:42:11 error[E0001]"));
        assert!(is_high_signal("tests/test_api.py:88: AssertionError"));
        assert!(!is_high_signal("12:34"));
    }

    #[test]
    fn format_signed_count_uses_thousands_separators() {
        assert_eq!(format_signed_count(0), "0");
        assert_eq!(format_signed_count(42), "42");
        assert_eq!(format_signed_count(1234), "1,234");
        assert_eq!(format_signed_count(-1_234_567), "-1,234,567");
    }

    #[test]
    fn head_tail_snapshot_caps_lines() {
        let text = (0..200)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (snapshot, _) = head_tail_snapshot(&text, 30, 10_000);
        assert!(snapshot.contains("line 0"));
        assert!(snapshot.contains("line 199"));
        assert!(snapshot.contains("lines elided"));
    }

    #[test]
    fn collapse_repeats_squashes_runs() {
        let text = "a\nb\nb\nb\nb\nb\nc\n";
        let (collapsed, count) = collapse_repeats(text, 2);
        assert!(collapsed.contains("collapsed"));
        assert!(count >= 1);
        assert!(collapsed.contains('c'));
    }
}
