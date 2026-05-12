use crate::proxy::adapter::{CommandAdapter, CompactResult, OutputBudget, RawRun, RunMeta};
use crate::proxy::command_ast::{normalize_program, CommandAst, CommandKind};

use super::common::AdapterBuild;

/// Adapter for test-runner commands: pytest/cargo test/go test/jest/vitest/etc.
pub struct TestsAdapter;

impl CommandAdapter for TestsAdapter {
    fn name(&self) -> &'static str {
        "tests"
    }

    fn kind(&self) -> CommandKind {
        CommandKind::Test
    }

    fn matches(&self, ast: &CommandAst) -> bool {
        if ast.detected_kind == CommandKind::Test {
            return true;
        }
        let program = normalize_program(&ast.program);
        matches!(
            program.as_str(),
            "pytest" | "jest" | "vitest" | "mocha" | "playwright" | "ava" | "cypress"
        )
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
        let runner = detect_runner(ast);
        let counts = parse_test_counts(&stdout_text, &stderr_text, runner);
        let mut build = AdapterBuild::new("tests");

        let header = match (run.exit_code, &counts) {
            (0, Some(c)) => format!(
                "PASS {} ({} passed{}{}{})",
                ast.original_command,
                c.passed,
                if c.failed > 0 {
                    format!(", {} failed", c.failed)
                } else {
                    String::new()
                },
                if c.skipped > 0 {
                    format!(", {} skipped", c.skipped)
                } else {
                    String::new()
                },
                duration_suffix(&c.duration),
            ),
            (0, None) => format!("PASS {}", ast.original_command),
            (_, Some(c)) => format!(
                "FAIL {} (exit: {}, {} failed, {} passed{}{})",
                ast.original_command,
                run.exit_code,
                c.failed,
                c.passed,
                if c.skipped > 0 {
                    format!(", {} skipped", c.skipped)
                } else {
                    String::new()
                },
                duration_suffix(&c.duration),
            ),
            (_, None) => format!("FAIL {} (exit: {})", ast.original_command, run.exit_code),
        };
        build.push_line(header);

        let cap = if run.exit_code != 0 {
            budget.failure_max_lines
        } else {
            (budget.max_lines / 3).max(20)
        };
        let failures = collect_failures(&stdout_text, &stderr_text, runner, cap);
        if !failures.failure_block.is_empty() {
            build.push_line("");
            build.push_line("failures:");
            build.push_block(&failures.failure_block);
            build.high_signal_count = failures.failure_count;
        }

        if let Some(suggestion) = rerun_suggestion(runner, &failures.failure_names) {
            build.push_line("");
            build.push_line("next:");
            build.push_line(suggestion);
        }

        let compacted = !ast.has_shell_syntax
            && (run.exit_code != 0
                || stdout_text.lines().count() + stderr_text.lines().count() > 8);
        build.finish(run, meta, compacted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runner {
    Pytest,
    CargoTest,
    GoTest,
    Jest,
    Vitest,
    NodeUnknown,
    Other,
}

fn detect_runner(ast: &CommandAst) -> Runner {
    let normalized = normalize_program(&ast.program);
    match normalized.as_str() {
        "pytest" => Runner::Pytest,
        "python" | "python3" if ast.args.iter().any(|a| a == "pytest") => Runner::Pytest,
        "cargo" if ast.args.first().map(|s| s.as_str()) == Some("test") => Runner::CargoTest,
        "cargo" if ast.args.first().map(|s| s.as_str()) == Some("nextest") => Runner::CargoTest,
        "go" if ast.args.first().map(|s| s.as_str()) == Some("test") => Runner::GoTest,
        "jest" => Runner::Jest,
        "vitest" => Runner::Vitest,
        "npx" => match ast.args.first().map(|s| s.as_str()) {
            Some("jest") => Runner::Jest,
            Some("vitest") => Runner::Vitest,
            _ => Runner::NodeUnknown,
        },
        "npm" | "pnpm" | "yarn" | "bun" => Runner::NodeUnknown,
        _ => Runner::Other,
    }
}

#[derive(Debug, Clone, Default)]
struct Counts {
    passed: usize,
    failed: usize,
    skipped: usize,
    duration: Option<String>,
}

fn duration_suffix(value: &Option<String>) -> String {
    match value {
        Some(d) => format!(", {d}"),
        None => String::new(),
    }
}

fn parse_test_counts(stdout: &str, stderr: &str, runner: Runner) -> Option<Counts> {
    let mut counts = Counts::default();
    let combined = format!("{stdout}\n{stderr}");
    match runner {
        Runner::Pytest => {
            for line in combined.lines().rev() {
                if line.contains("passed") || line.contains("failed") || line.contains("error") {
                    counts.passed = extract_number_before(line, "passed").unwrap_or(0);
                    counts.failed = extract_number_before(line, "failed").unwrap_or(0)
                        + extract_number_before(line, "error").unwrap_or(0);
                    counts.skipped = extract_number_before(line, "skipped").unwrap_or(0);
                    counts.duration = extract_pytest_duration(line);
                    if counts.passed + counts.failed + counts.skipped > 0 {
                        return Some(counts);
                    }
                }
            }
        }
        Runner::CargoTest => {
            for line in combined.lines() {
                if line.starts_with("test result:") {
                    counts.passed = extract_number_before(line, "passed").unwrap_or(0);
                    counts.failed = extract_number_before(line, "failed").unwrap_or(0);
                    counts.skipped = extract_number_before(line, "ignored").unwrap_or(0);
                    return Some(counts);
                }
            }
        }
        Runner::GoTest => {
            for line in combined.lines() {
                if line.starts_with("PASS") || line.starts_with("ok ") {
                    counts.passed += 1;
                } else if line.starts_with("FAIL") || line.starts_with("--- FAIL") {
                    counts.failed += 1;
                } else if line.starts_with("--- SKIP") {
                    counts.skipped += 1;
                }
            }
            if counts.passed + counts.failed + counts.skipped > 0 {
                return Some(counts);
            }
        }
        Runner::Jest | Runner::Vitest | Runner::NodeUnknown => {
            for line in combined.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("Tests:") {
                    counts.failed = extract_number_before(rest, "failed").unwrap_or(0);
                    counts.passed = extract_number_before(rest, "passed").unwrap_or(0);
                    counts.skipped = extract_number_before(rest, "skipped").unwrap_or(0);
                    if counts.passed + counts.failed + counts.skipped > 0 {
                        return Some(counts);
                    }
                }
            }
        }
        Runner::Other => {}
    }
    None
}

fn extract_number_before(line: &str, term: &str) -> Option<usize> {
    let idx = line.to_ascii_lowercase().find(term)?;
    let prefix = &line[..idx];
    let tokens: Vec<&str> = prefix.split_whitespace().collect();
    tokens.iter().rev().find_map(|t| t.parse::<usize>().ok())
}

fn extract_pytest_duration(line: &str) -> Option<String> {
    // Pytest summary ends with `in 12.34s` or `in 1.2 seconds`.
    let lower = line.to_ascii_lowercase();
    let idx = lower.rfind(" in ")?;
    let tail = line[idx + 4..]
        .trim_end_matches('=')
        .trim()
        .trim_end_matches('.');
    Some(tail.to_string())
}

#[derive(Debug, Default)]
struct Failures {
    failure_block: String,
    failure_count: usize,
    failure_names: Vec<String>,
}

fn collect_failures(stdout: &str, stderr: &str, runner: Runner, cap: usize) -> Failures {
    let combined = format!("{stdout}\n{stderr}");
    let mut block = String::new();
    let mut names: Vec<String> = Vec::new();
    let mut lines_emitted = 0usize;
    let lines: Vec<&str> = combined.lines().collect();

    let emit = |buf: &mut String, line: &str, emitted: &mut usize| {
        if *emitted < cap {
            buf.push_str(line);
            buf.push('\n');
            *emitted += 1;
        }
    };

    match runner {
        Runner::Pytest => {
            let mut in_failures = false;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("FAILED ") || trimmed.starts_with("ERROR ") {
                    if let Some(name) = trimmed.split_whitespace().nth(1) {
                        names.push(name.to_string());
                    }
                    emit(&mut block, line, &mut lines_emitted);
                }
                if trimmed.starts_with("=") && trimmed.contains("FAILURES") {
                    in_failures = true;
                    continue;
                }
                if in_failures {
                    if trimmed.starts_with("=") && trimmed.contains("short test summary") {
                        in_failures = false;
                        continue;
                    }
                    emit(&mut block, line, &mut lines_emitted);
                }
            }
        }
        Runner::CargoTest => {
            let mut in_failures = false;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed == "failures:" {
                    in_failures = true;
                    emit(&mut block, line, &mut lines_emitted);
                    continue;
                }
                if in_failures && trimmed.starts_with("test result:") {
                    in_failures = false;
                }
                if in_failures {
                    if let Some(rest) = trimmed.strip_prefix("---- ") {
                        if let Some(name) = rest.split_whitespace().next() {
                            names.push(name.to_string());
                        }
                    }
                    emit(&mut block, line, &mut lines_emitted);
                }
            }
        }
        Runner::GoTest => {
            for (index, line) in lines.iter().enumerate() {
                if line.starts_with("--- FAIL:") {
                    if let Some(name) = line.split_whitespace().nth(2) {
                        names.push(name.trim().trim_end_matches(':').to_string());
                    }
                    // Include the failure header and a small window of context.
                    for offset in 0..6.min(lines.len() - index) {
                        emit(&mut block, lines[index + offset], &mut lines_emitted);
                    }
                }
            }
        }
        Runner::Jest | Runner::Vitest | Runner::NodeUnknown => {
            let mut buffering = false;
            for line in &lines {
                let trimmed = line.trim_start();
                if trimmed.starts_with("FAIL ") || trimmed.starts_with("✗ ") {
                    buffering = true;
                    if let Some(name) = trimmed.split_whitespace().nth(1) {
                        names.push(name.to_string());
                    }
                }
                if buffering {
                    emit(&mut block, line, &mut lines_emitted);
                    if trimmed.starts_with("Tests:") {
                        buffering = false;
                    }
                }
            }
        }
        Runner::Other => {}
    }

    Failures {
        failure_block: block,
        failure_count: names.len(),
        failure_names: names,
    }
}

fn rerun_suggestion(runner: Runner, failures: &[String]) -> Option<String> {
    if failures.is_empty() {
        return None;
    }
    let unique: Vec<&str> = failures
        .iter()
        .map(|s| s.as_str())
        .take(8)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    match runner {
        Runner::Pytest => Some(format!("pytest {} -q", unique.join(" "))),
        Runner::CargoTest => Some(format!("cargo test -- --exact {}", unique.join(" "))),
        Runner::GoTest => Some(format!("go test -run '{}'", unique.join("|"))),
        Runner::Jest => Some(format!("jest -t {}", unique.join(" "))),
        Runner::Vitest => Some(format!("vitest -t {}", unique.join(" "))),
        Runner::NodeUnknown | Runner::Other => None,
    }
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
            adapter_name: "tests".to_string(),
            raw_path: PathBuf::from("/tmp/raw"),
            compact_path: PathBuf::from("/tmp/compact"),
            channel: "next".to_string(),
            target_agent: "windsurf".to_string(),
            invoked_as_shim: None,
            wf_core_version: "test".to_string(),
        }
    }

    #[test]
    fn matches_pytest_and_cargo_test() {
        let ast = build_ast(
            &["pytest".to_string(), "-q".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        assert!(TestsAdapter.matches(&ast));
        let ast2 = build_ast(
            &["cargo".to_string(), "test".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        assert!(TestsAdapter.matches(&ast2));
        let ast3 = build_ast(
            &["echo".to_string(), "hi".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        assert!(!TestsAdapter.matches(&ast3));
    }

    #[test]
    fn pytest_failure_extracts_counts_and_failing_tests() {
        let stdout = b"\
=================================== FAILURES ===================================
________________________ test_create_user _________________________

>       assert response.status_code == 201
E       AssertionError: expected 201, got 500
tests/api/test_users.py:88: AssertionError
=========================== short test summary info ============================
FAILED tests/api/test_users.py::test_create_user - AssertionError
FAILED tests/auth/test_token.py::test_expired_token - ValueError
============================ 2 failed, 143 passed, 4 skipped in 12.82s =========
";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 1,
            duration_ms: 12_820,
        };
        let ast = build_ast(
            &["pytest".to_string(), "tests".to_string(), "-q".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let result = TestsAdapter.compact(&ast, &run, &meta(), OutputBudget::default());
        assert!(result.compacted);
        assert!(result
            .summary
            .contains("FAIL pytest tests -q (exit: 1, 2 failed, 143 passed"));
        assert!(result.summary.contains("test_create_user"));
        assert!(result.summary.contains("test_expired_token"));
        assert!(result.summary.contains("next:"));
        assert!(result.summary.contains("pytest "));
    }

    #[test]
    fn cargo_test_success_extracts_counts() {
        let stdout = b"\nrunning 5 tests\ntest a ... ok\ntest b ... ok\ntest c ... ok\ntest d ... ok\ntest e ... ok\n\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 0,
            duration_ms: 100,
        };
        let ast = build_ast(
            &["cargo".to_string(), "test".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let result = TestsAdapter.compact(&ast, &run, &meta(), OutputBudget::default());
        assert!(result.summary.starts_with("PASS cargo test (5 passed"));
    }

    #[test]
    fn go_test_failure_extracts_test_name() {
        let stdout = b"=== RUN   TestFoo\n--- FAIL: TestFoo (0.01s)\n    foo_test.go:12: expected 1 got 2\nFAIL\nexit status 1\nFAIL    example.com/pkg 0.01s\n";
        let run = RawRun {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code: 1,
            duration_ms: 10,
        };
        let ast = build_ast(
            &["go".to_string(), "test".to_string(), "./...".to_string()],
            PathBuf::from("/tmp"),
            false,
            None,
        );
        let result = TestsAdapter.compact(&ast, &run, &meta(), OutputBudget::default());
        assert!(result.summary.contains("FAIL go test ./..."));
        assert!(result.summary.contains("TestFoo"));
        assert!(result.summary.contains("foo_test.go:12"));
    }
}
