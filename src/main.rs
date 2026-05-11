use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MANAGED_START: &str = "<!-- wf-core managed:start -->";
const MANAGED_END: &str = "<!-- wf-core managed:end -->";
const DEFAULT_CHANNELS: &[&str] = &["stable", "next"];
const DEFAULT_TARGET: &str = "all";

const SKILL_NAMES: &[&str] = &[
    "backend-and-data-architecture",
    "cloud-and-devops-expert",
    "git-expert",
    "memory-status-reporter",
    "mobile-development-life-cycle",
    "preserve-existing-flow",
    "qa-and-automation-engineer",
    "reviewer",
    "security-and-compliance-auditor",
    "software-development-life-cycle",
    "ui-design-systems-and-responsive-interfaces",
    "ux-research-and-experience-strategy",
    "web-development-life-cycle",
];

const WORKFLOW_NAMES: &[&str] = &[
    "wf-core-finish.md",
    "wf-core-hooks.md",
    "wf-core-install-check.md",
    "wf-core-review.md",
    "wf-core-start.md",
    "wf-core-token-saving-shell.md",
];
const OLD_MANAGED_WORKFLOW_NAMES: &[&str] = &[
    "finish.md",
    "install-check.md",
    "start.md",
    "token-saving-shell.md",
];
const MANIFEST_FILE_NAME: &str = "manifest.tsv";

const NOISY_ROOT_COMMANDS: &[&str] = &[
    "bun",
    "cargo",
    "docker",
    "docker-compose",
    "dotnet",
    "go",
    "gradle",
    "gradlew",
    "helm",
    "java",
    "jest",
    "kubectl",
    "make",
    "mvn",
    "node",
    "npm",
    "pnpm",
    "pytest",
    "terraform",
    "tsc",
    "yarn",
];

const NOISY_GIT_SUBCOMMANDS: &[&str] = &["diff", "grep", "log", "show", "status"];
const SHELL_MARKERS: &[&str] = &["|", "&&", "||", ";", ">", "<", "$(", "`"];
const HIGH_SIGNAL_TERMS: &[&str] = &[
    "error",
    "failed",
    "failure",
    "fatal",
    "panic",
    "traceback",
    "exception",
    "assert",
    "warning",
    "denied",
    "cannot",
    "not found",
    "unresolved",
    "expected",
    "actual",
    "timeout",
    "segmentation",
];

#[derive(Debug)]
struct AppError {
    message: String,
}

impl AppError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::new(error.to_string())
    }
}

#[derive(Debug)]
struct InstallSummary {
    messages: Vec<String>,
}

#[derive(Debug)]
struct Compaction {
    rendered: String,
    compacted: bool,
    raw_bytes: usize,
    compacted_bytes: usize,
    saved_bytes: usize,
    high_signal_count: usize,
    raw_path: PathBuf,
}

#[derive(Debug)]
struct VerifyItem {
    surface: String,
    path: PathBuf,
    ok: bool,
}

#[derive(Default)]
struct GainEvent {
    raw_bytes: usize,
    compacted_bytes: usize,
    saved_bytes: usize,
    compacted: bool,
}

#[derive(Debug, Clone)]
struct ManifestEntry {
    relative_path: PathBuf,
    checksum: String,
    size: u64,
}

#[derive(Debug)]
struct ReviewFinding {
    severity: String,
    message: String,
}

#[derive(Debug)]
struct ReviewReport {
    ok: bool,
    repo_root: PathBuf,
    branch: String,
    changed_files: Vec<String>,
    findings: Vec<ReviewFinding>,
}

fn main() {
    let exit_code = match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("wf-core failed: {}", error.message);
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(arguments: Vec<String>) -> Result<i32, AppError> {
    if arguments.is_empty() {
        print_help();
        return Ok(1);
    }
    match arguments[0].as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(0)
        }
        "version" | "--version" | "-V" => {
            println!("wf-core {VERSION}");
            Ok(0)
        }
        "install" => command_install(&arguments[1..]),
        "update" => command_update(&arguments[1..]),
        "status" => command_status(&arguments[1..]),
        "verify" => command_verify(&arguments[1..]),
        "doctor" => command_doctor(&arguments[1..]),
        "flow" => command_flow(&arguments[1..]),
        "review" => command_review(&arguments[1..]),
        "git-workflow" => command_git_workflow(&arguments[1..]),
        "workflow" => command_workflow(&arguments[1..]),
        "memory" => command_memory(&arguments[1..]),
        "hook" => command_hook(&arguments[1..]),
        "devin-hook" => command_devin_hook(&arguments[1..]),
        "rewrite" => command_rewrite(&arguments[1..]),
        "run" => command_run(&arguments[1..]),
        "gain" => command_gain(&arguments[1..]),
        "instructions" => {
            print_instructions();
            Ok(0)
        }
        "uninstall" => command_uninstall(&arguments[1..]),
        other => Err(AppError::new(format!("unknown command: {other}"))),
    }
}

fn print_help() {
    println!(
        "wf-core {VERSION}

Rust-native managed Windsurf/Windsurf Next global installer and token-saving command wrapper.

Usage:
  wf-core install [--target windsurf|devin|all] [--channel stable|next|insiders|both] [--source-root PATH]
  wf-core update [--target windsurf|devin|all] [--channel stable|next|insiders|both]
  wf-core status [--target windsurf|devin|all] [--channel stable|next|insiders|both]
  wf-core verify [--target windsurf|devin|all] [--channel stable|next|insiders|both] [--json]
  wf-core doctor [--target windsurf|devin|all] [--channel stable|next|insiders|both]
  wf-core flow start|check|finish [options]
  wf-core review pre-pr|pre-commit|gates check|hosted check [options]
  wf-core git-workflow commit-message|pr-body|lint-message|preflight [options]
  wf-core workflow start|cockpit|finish [options]
  wf-core memory status|remember|recall|system-map [options]
  wf-core hook install|list|instructions [--target windsurf|devin|all] [--channel stable|next|insiders|both]
  wf-core rewrite \"<command>\"
  wf-core run [--channel next] [--max-lines N] [--max-bytes N] [--shell] -- <command>
  wf-core gain [--channel next] [--json]
  wf-core instructions
  wf-core uninstall --yes [--channel stable|next|insiders|both]

Install is global-only. It writes to ~/.codeium/windsurf* and Devin's global config home, not the current workspace."
    );
}

fn command_install(arguments: &[String]) -> Result<i32, AppError> {
    let target = flag_value(arguments, "--target").unwrap_or_else(|| DEFAULT_TARGET.to_string());
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "both".to_string());
    let source_root = resolve_source_root(flag_value(arguments, "--source-root"))?;
    let targets = expand_targets(&target)?;
    let mut summary = InstallSummary {
        messages: Vec::new(),
    };

    if targets.contains(&"windsurf".to_string()) {
        for channel_name in expand_channels(&channel)? {
            install_global_channel(&channel_name, &source_root, &mut summary)?;
        }
    }
    if targets.contains(&"devin".to_string()) {
        install_devin_global(&source_root, &mut summary)?;
    }

    println!("wf-core global install complete");
    for message in summary.messages {
        println!("  - {message}");
    }
    println!("Restart Windsurf/Windsurf Next and Devin for Terminal so global surfaces refresh.");
    Ok(0)
}

fn command_update(arguments: &[String]) -> Result<i32, AppError> {
    let target = flag_value(arguments, "--target").unwrap_or_else(|| DEFAULT_TARGET.to_string());
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "both".to_string());
    let source_root = match flag_value(arguments, "--source-root") {
        Some(path) => PathBuf::from(path).canonicalize()?,
        None => {
            let targets = expand_targets(&target)?;
            if targets.contains(&"windsurf".to_string()) {
                let channels = expand_channels(&channel)?;
                let first = channels
                    .first()
                    .ok_or_else(|| AppError::new("no channel selected"))?;
                install_source_from_metadata(first)?
            } else {
                install_source_from_devin_metadata()?
            }
        }
    };
    let install_args = vec![
        "--target".to_string(),
        target,
        "--channel".to_string(),
        channel,
        "--source-root".to_string(),
        display_path(&source_root),
    ];
    command_install(&install_args)
}

fn command_status(arguments: &[String]) -> Result<i32, AppError> {
    let target = flag_value(arguments, "--target").unwrap_or_else(|| DEFAULT_TARGET.to_string());
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "both".to_string());
    let targets = expand_targets(&target)?;
    println!("wf-core status");
    println!("  Version: {VERSION}");
    println!("  Runtime: rust");
    println!("  Install mode: global-only");
    println!();
    if targets.contains(&"windsurf".to_string()) {
        println!("Windsurf global channels:");
        for channel_name in expand_channels(&channel)? {
            let home = channel_home(&channel_name)?;
            let skills = count_installed_skills(&home.join("skills"));
            let workflows = count_installed_workflows(&home);
            let binary = installed_binary_path(&channel_name)?;
            let global_rules = home.join("memories").join("global_rules.md");
            println!("  {channel_name}:");
            println!("    home: {}", display_path(&home));
            println!("    skills: {skills}/{}", SKILL_NAMES.len());
            println!("    workflows: {workflows}/{}", WORKFLOW_NAMES.len());
            println!(
                "    global rules: {}",
                if global_rules.exists() { "yes" } else { "no" }
            );
            println!(
                "    Rust binary: {}",
                if binary.exists() {
                    display_path(&binary)
                } else {
                    "not installed".to_string()
                }
            );
        }
    }
    if targets.contains(&"devin".to_string()) {
        print_devin_status()?;
    }
    Ok(0)
}

fn command_verify(arguments: &[String]) -> Result<i32, AppError> {
    let target = flag_value(arguments, "--target").unwrap_or_else(|| DEFAULT_TARGET.to_string());
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "both".to_string());
    let as_json = has_flag(arguments, "--json");
    let targets = expand_targets(&target)?;
    let mut items = Vec::new();
    if targets.contains(&"windsurf".to_string()) {
        for channel_name in expand_channels(&channel)? {
            items.extend(verify_global_channel(&channel_name)?);
        }
    }
    if targets.contains(&"devin".to_string()) {
        items.extend(verify_devin_global()?);
    }
    let ok = items.iter().all(|item| item.ok);
    if as_json {
        print_verify_json(ok, &items);
    } else {
        println!("wf-core verify (global-only, rust)");
        for item in &items {
            let status = if item.ok {
                "ok"
            } else if item.surface.contains("checksum") {
                "mismatch"
            } else if item.surface.contains("stale managed file") {
                "stale"
            } else {
                "missing"
            };
            println!(
                "  [{status}] {}: {}",
                item.surface,
                display_path(&item.path)
            );
        }
    }
    Ok(if ok { 0 } else { 1 })
}

fn print_devin_status() -> Result<(), AppError> {
    let home = devin_home()?;
    let skills = count_installed_skills(&home.join("skills"));
    let agents = count_installed_agents(&home.join("agents"));
    let binary = devin_binary_path()?;
    let config = home.join("config.json");
    println!("Devin for Terminal:");
    println!("  home: {}", display_path(&home));
    println!("  skills: {skills}/{}", SKILL_NAMES.len());
    println!("  agents: {agents}/{}", SKILL_NAMES.len());
    println!(
        "  config hooks/imports: {}",
        if config.exists()
            && fs::read_to_string(&config)
                .map(|content| content.contains("wf-core") && content.contains("read_config_from"))
                .unwrap_or(false)
        {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "  Rust binary: {}",
        if binary.exists() {
            display_path(&binary)
        } else {
            "not installed".to_string()
        }
    );
    Ok(())
}

fn command_doctor(arguments: &[String]) -> Result<i32, AppError> {
    let status_code = command_status(arguments)?;
    println!();
    let verify_code = command_verify(arguments)?;
    if status_code == 0 && verify_code == 0 {
        println!();
        println!("Doctor: Rust-native global Windsurf and Devin surfaces are ready.");
        Ok(0)
    } else {
        eprintln!("Doctor: one or more global surfaces are missing.");
        Ok(1)
    }
}

fn command_flow(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core flow start|check|finish [options]",
        ));
    }
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "next".to_string());
    let workspace_root = workspace_root_from_flag(arguments)?;
    let artifact_path = flow_artifact_path(&channel, &workspace_root)?;
    match arguments[0].as_str() {
        "start" => {
            let task =
                flag_value(arguments, "--task").unwrap_or_else(|| "wf-core task".to_string());
            let target_file = flag_value(arguments, "--target-file")
                .ok_or_else(|| AppError::new("flow start requires --target-file <path>"))?;
            let target_function = flag_value(arguments, "--target-function").unwrap_or_default();
            let path = requested_flow_artifact_path(arguments, artifact_path, "--output");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &path,
                render_flow_template(&task, &target_file, &target_function),
            )?;
            if has_flag(arguments, "--json") {
                println!(
                    "{{\"created\":true,\"path\":{},\"schema\":1}}",
                    json_string(&display_path(&path))
                );
            } else {
                println!("flow check started at {}", display_path(&path));
            }
            Ok(0)
        }
        "check" => {
            let path = requested_flow_artifact_path(arguments, artifact_path, "--artifact");
            let content = fs::read_to_string(&path)?;
            let findings = validate_flow_content(&content);
            if findings.is_empty() {
                if has_flag(arguments, "--json") {
                    println!(
                        "{{\"valid\":true,\"path\":{},\"schema\":1}}",
                        json_string(&display_path(&path))
                    );
                } else {
                    println!("flow check valid: {}", display_path(&path));
                }
                Ok(0)
            } else {
                if has_flag(arguments, "--json") {
                    println!("{}", render_findings_json(false, &path, &findings));
                } else {
                    eprintln!("flow check failed: {}", display_path(&path));
                    for finding in findings {
                        eprintln!("  - {finding}");
                    }
                }
                Ok(1)
            }
        }
        "finish" => {
            let path = requested_flow_artifact_path(arguments, artifact_path, "--artifact");
            let content = fs::read_to_string(&path)?;
            let findings = validate_flow_content(&content);
            if findings.is_empty() {
                let marker = path.with_file_name("flow-finish.txt");
                fs::write(&marker, format!("finished_at={}\n", now_millis() / 1000))?;
                if has_flag(arguments, "--json") {
                    println!(
                        "{{\"valid\":true,\"finished\":true,\"path\":{},\"schema\":1}}",
                        json_string(&display_path(&path))
                    );
                } else {
                    println!("flow check finished: {}", display_path(&path));
                }
                Ok(0)
            } else {
                if has_flag(arguments, "--json") {
                    println!("{}", render_findings_json(false, &path, &findings));
                } else {
                    eprintln!("flow finish blocked by invalid evidence");
                    for finding in findings {
                        eprintln!("  - {finding}");
                    }
                }
                Ok(1)
            }
        }
        other => Err(AppError::new(format!("unknown flow command: {other}"))),
    }
}

fn command_review(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return run_review_gate(arguments);
    }
    match arguments[0].as_str() {
        "pre-pr" | "pre-commit" => run_review_gate(&arguments[1..]),
        "gates" if arguments.get(1).map(String::as_str) == Some("check") => {
            run_review_gate(&arguments[2..])
        }
        "hosted" if arguments.get(1).map(String::as_str) == Some("check") => {
            let repo_root = repo_root_from_flag(&arguments[2..])?;
            let out_dir = flag_value(&arguments[2..], "--out-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo_root.join("wf-core-review-artifacts"));
            fs::create_dir_all(&out_dir)?;
            let report = build_review_report(&repo_root)?;
            fs::write(out_dir.join("check.md"), render_review_markdown(&report))?;
            fs::write(out_dir.join("check.json"), render_review_json(&report))?;
            println!(
                "hosted review artifacts written to {}",
                display_path(&out_dir)
            );
            Ok(if report.ok { 0 } else { 1 })
        }
        other => Err(AppError::new(format!("unknown review command: {other}"))),
    }
}

fn command_git_workflow(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core git-workflow commit-message|pr-body|lint-message|preflight",
        ));
    }
    match arguments[0].as_str() {
        "commit-message" => {
            let repo_root = repo_root_from_flag(&arguments[1..])?;
            let test_result = flag_value(&arguments[1..], "--test-result")
                .unwrap_or_else(|| "Not provided".to_string());
            println!("{}", render_commit_message(&repo_root, &test_result)?);
            Ok(0)
        }
        "pr-body" => {
            let repo_root = repo_root_from_flag(&arguments[1..])?;
            let test_result = flag_value(&arguments[1..], "--test-result")
                .unwrap_or_else(|| "Not provided".to_string());
            println!("{}", render_pr_body(&repo_root, &test_result)?);
            Ok(0)
        }
        "lint-message" => {
            let path = arguments
                .get(1)
                .ok_or_else(|| AppError::new("lint-message requires a file path"))?;
            let content = fs::read_to_string(path)?;
            let findings = lint_message_text(&content);
            if findings.is_empty() {
                println!("message lint passed");
                Ok(0)
            } else {
                eprintln!("message lint failed");
                for finding in findings {
                    eprintln!("  - {finding}");
                }
                Ok(1)
            }
        }
        "preflight" => run_git_preflight(&arguments[1..]),
        other => Err(AppError::new(format!(
            "unknown git-workflow command: {other}"
        ))),
    }
}

fn command_workflow(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core workflow start|cockpit|finish [options]",
        ));
    }
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "next".to_string());
    let workspace_root = workspace_root_from_flag(arguments)?;
    let state_path = workflow_state_path(&channel, &workspace_root)?;
    match arguments[0].as_str() {
        "start" => {
            if let Some(parent) = state_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let request =
                flag_value(arguments, "--request").unwrap_or_else(|| "No request recorded".into());
            fs::write(
                &state_path,
                format!(
                    "stage=start\nrequest={}\nworkspace={}\nupdated_at={}\n",
                    request,
                    display_path(&workspace_root),
                    now_millis() / 1000
                ),
            )?;
            println!("workflow started: {}", display_path(&state_path));
            Ok(0)
        }
        "cockpit" => {
            if state_path.exists() {
                print!("{}", fs::read_to_string(&state_path)?);
            } else {
                println!("workflow state not found: {}", display_path(&state_path));
            }
            Ok(0)
        }
        "finish" => {
            let review_code =
                run_review_gate(&["--repo-root".to_string(), display_path(&workspace_root)])?;
            if review_code == 0 {
                fs::write(
                    &state_path,
                    format!(
                        "stage=finish\nworkspace={}\nupdated_at={}\n",
                        display_path(&workspace_root),
                        now_millis() / 1000
                    ),
                )?;
            }
            Ok(review_code)
        }
        other => Err(AppError::new(format!("unknown workflow command: {other}"))),
    }
}

fn command_memory(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core memory status|remember|recall|system-map [options]",
        ));
    }
    match arguments[0].as_str() {
        "status" => {
            let repo_root = repo_root_from_flag(&arguments[1..])?;
            let memory_root = workspace_memory_root(&repo_root)?;
            let system_map = system_map_path(&repo_root)?;
            println!("wf-core memory");
            println!("  root: {}", display_path(&wf_core_data_home()?));
            println!("  workspace: {}", display_path(&memory_root));
            println!(
                "  system map: {}",
                if system_map.exists() {
                    display_path(&system_map)
                } else {
                    "missing".to_string()
                }
            );
            println!(
                "  notes: {}",
                count_files_with_extension(&memory_root.join("notes"), "md")
            );
            println!(
                "  research cache: {}",
                count_files_with_extension(&memory_root.join("research-cache"), "md")
            );
            Ok(0)
        }
        "remember" => {
            let repo_root = repo_root_from_flag(&arguments[1..])?;
            let kind = flag_value(&arguments[1..], "--kind").unwrap_or_else(|| "notes".to_string());
            let text = flag_value(&arguments[1..], "--text")
                .ok_or_else(|| AppError::new("memory remember requires --text <value>"))?;
            let key = flag_value(&arguments[1..], "--key").unwrap_or_else(|| slugify(&text));
            let path = memory_note_path(&repo_root, &kind, &key)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &path,
                format!(
                    "# {key}\n\n- recorded_at: {}\n- kind: {kind}\n\n{}\n",
                    now_millis() / 1000,
                    text.trim()
                ),
            )?;
            println!("memory recorded: {}", display_path(&path));
            Ok(0)
        }
        "recall" => {
            let repo_root = repo_root_from_flag(&arguments[1..])?;
            let kind = flag_value(&arguments[1..], "--kind").unwrap_or_else(|| "notes".to_string());
            let dir = workspace_memory_root(&repo_root)?.join(kind);
            if !dir.exists() {
                println!("no memory entries: {}", display_path(&dir));
                return Ok(0);
            }
            for path in list_files_with_extension(&dir, "md")? {
                println!("--- {}", display_path(&path));
                print!("{}", fs::read_to_string(path)?);
                println!();
            }
            Ok(0)
        }
        "system-map" => command_system_map(&arguments[1..]),
        other => Err(AppError::new(format!("unknown memory command: {other}"))),
    }
}

fn command_system_map(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core memory system-map refresh|show|verify [--repo-root PATH]",
        ));
    }
    let repo_root = repo_root_from_flag(arguments)?;
    let path = system_map_path(&repo_root)?;
    match arguments[0].as_str() {
        "refresh" => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let map = render_system_map(&repo_root)?;
            fs::write(&path, map)?;
            println!("system map refreshed: {}", display_path(&path));
            Ok(0)
        }
        "show" => {
            print!("{}", fs::read_to_string(&path)?);
            Ok(0)
        }
        "verify" => {
            let content = fs::read_to_string(&path)?;
            let expected = repo_inventory_fingerprint(&repo_root)?;
            if content.contains(&format!("inventory_fingerprint: {expected}")) {
                println!("system map current: {}", display_path(&path));
                Ok(0)
            } else {
                eprintln!("system map stale: {}", display_path(&path));
                Ok(1)
            }
        }
        other => Err(AppError::new(format!(
            "unknown system-map command: {other}"
        ))),
    }
}

fn command_hook(arguments: &[String]) -> Result<i32, AppError> {
    if arguments.is_empty() || arguments[0] == "instructions" {
        print_hook_instructions();
        return Ok(0);
    }
    match arguments[0].as_str() {
        "install" => {
            let target = flag_value(&arguments[1..], "--target")
                .unwrap_or_else(|| DEFAULT_TARGET.to_string());
            let channel =
                flag_value(&arguments[1..], "--channel").unwrap_or_else(|| "both".to_string());
            let source_root = resolve_source_root(flag_value(&arguments[1..], "--source-root"))?;
            let targets = expand_targets(&target)?;
            let mut summary = InstallSummary {
                messages: Vec::new(),
            };
            if targets.contains(&"windsurf".to_string()) {
                for channel_name in expand_channels(&channel)? {
                    install_global_channel(&channel_name, &source_root, &mut summary)?;
                }
            }
            if targets.contains(&"devin".to_string()) {
                install_devin_global(&source_root, &mut summary)?;
            }
            println!("wf-core hook-equivalent global policy installed");
            for message in summary.messages {
                println!("  - {message}");
            }
            Ok(0)
        }
        "list" | "show" => {
            let target = flag_value(&arguments[1..], "--target")
                .unwrap_or_else(|| DEFAULT_TARGET.to_string());
            let channel =
                flag_value(&arguments[1..], "--channel").unwrap_or_else(|| "both".to_string());
            let targets = expand_targets(&target)?;
            println!("wf-core hook-equivalent policy");
            if targets.contains(&"windsurf".to_string()) {
                for channel_name in expand_channels(&channel)? {
                    let home = channel_home(&channel_name)?;
                    let global_rules = home.join("memories").join("global_rules.md");
                    let hooks_workflow = home
                        .join("windsurf")
                        .join("workflows")
                        .join("wf-core-hooks.md");
                    let token_workflow = home
                        .join("windsurf")
                        .join("workflows")
                        .join("wf-core-token-saving-shell.md");
                    println!("  {channel_name}:");
                    println!(
                        "    global terminal rule: {}",
                        if global_rules.exists() {
                            "installed"
                        } else {
                            "missing"
                        }
                    );
                    println!(
                        "    wf-core-hooks workflow: {}",
                        if hooks_workflow.exists() {
                            "installed"
                        } else {
                            "missing"
                        }
                    );
                    println!(
                        "    token-saving workflow: {}",
                        if token_workflow.exists() {
                            "installed"
                        } else {
                            "missing"
                        }
                    );
                }
            }
            if targets.contains(&"devin".to_string()) {
                let home = devin_home()?;
                let config = home.join("config.json");
                let hook_artifact = home.join("wf-core").join("devin-hooks.v1.json");
                println!("  devin:");
                println!(
                    "    global config hook: {}",
                    if config.exists()
                        && fs::read_to_string(&config)
                            .map(|content| content.contains("wf-core"))
                            .unwrap_or(false)
                    {
                        "installed"
                    } else {
                        "missing"
                    }
                );
                println!(
                    "    hook artifact: {}",
                    if hook_artifact.exists() {
                        "installed"
                    } else {
                        "missing"
                    }
                );
            }
            Ok(0)
        }
        other => Err(AppError::new(format!(
            "unknown hook command: {other}. Use hook install, hook list, or hook instructions"
        ))),
    }
}

fn command_devin_hook(arguments: &[String]) -> Result<i32, AppError> {
    let hook_kind = arguments
        .first()
        .map(String::as_str)
        .unwrap_or("pre-tool-use");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    match hook_kind {
        "pre-tool-use" => {
            let command = json_string_field(&input, "command").unwrap_or_default();
            if command.trim().is_empty()
                || command.contains("wf-core run")
                || command.contains("wf-core.exe\" run")
            {
                return Ok(0);
            }
            let (supported, _) = is_supported_noisy_command(&command);
            if supported {
                let wrapper = current_wrapper_command()?;
                let rerun = if requires_shell(&command) {
                    format!("{wrapper} run --shell -- {}", quote_arg(&command))
                } else {
                    format!("{wrapper} run -- {command}")
                };
                println!(
                    "{{\"decision\":\"block\",\"reason\":{}}}",
                    json_string(&format!("Rerun that as: {rerun}"))
                );
            }
            Ok(0)
        }
        _ => Ok(0),
    }
}

fn command_rewrite(arguments: &[String]) -> Result<i32, AppError> {
    let command_text = arguments.join(" ").trim().to_string();
    if command_text.is_empty() {
        return Err(AppError::new("Usage: wf-core rewrite \"<command>\""));
    }
    let (supported, reason) = is_supported_noisy_command(&command_text);
    if !supported {
        return Err(AppError::new(reason));
    }
    let wrapper = current_wrapper_command()?;
    if requires_shell(&command_text) {
        println!("{wrapper} run --shell -- {}", quote_arg(&command_text));
    } else {
        println!("{wrapper} run -- {command_text}");
    }
    Ok(0)
}

fn command_run(arguments: &[String]) -> Result<i32, AppError> {
    let option_arguments = arguments_before_separator(arguments);
    let channel = flag_value(option_arguments, "--channel").unwrap_or_else(|| "next".to_string());
    let max_lines = flag_value(option_arguments, "--max-lines")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(120);
    let max_bytes = flag_value(option_arguments, "--max-bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(12_000);
    let shell_mode = has_flag(option_arguments, "--shell");
    let command_args = positional_after_options(arguments);
    if command_args.is_empty() {
        return Err(AppError::new(
            "Usage: wf-core run [--shell] -- <command> [args...]",
        ));
    }

    let started = now_millis();
    let output = execute_command(&command_args, shell_mode)?;
    let elapsed_ms = now_millis().saturating_sub(started);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw_text = render_raw_output(&stdout, &stderr);
    let display_command = command_args.join(" ");
    let raw_path = save_raw_output(&channel, &display_command, &raw_text)?;
    let compaction = compact_output(
        &display_command,
        output.status.code().unwrap_or(1),
        &stdout,
        &stderr,
        raw_path,
        max_lines,
        max_bytes,
        elapsed_ms,
    );
    record_gain_event(
        &channel,
        &display_command,
        output.status.code().unwrap_or(1),
        &compaction,
    )?;

    if compaction.compacted {
        print!("{}", compaction.rendered);
        if !compaction.rendered.ends_with('\n') {
            println!();
        }
    } else {
        if !stdout.is_empty() {
            print!("{stdout}");
            if !stdout.ends_with('\n') {
                println!();
            }
        }
        if !stderr.is_empty() {
            eprint!("{stderr}");
            if !stderr.ends_with('\n') {
                eprintln!();
            }
        }
        println!(
            "[wf-core] raw output saved at {}",
            display_path(&compaction.raw_path)
        );
    }

    Ok(clamp_exit_code(output.status.code().unwrap_or(1)))
}

fn command_gain(arguments: &[String]) -> Result<i32, AppError> {
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "next".to_string());
    let as_json = has_flag(arguments, "--json");
    let events = read_gain_events(&channel)?;
    let mut total_raw = 0usize;
    let mut total_compacted = 0usize;
    let mut total_saved = 0usize;
    let mut compacted_events = 0usize;
    for event in &events {
        total_raw += event.raw_bytes;
        total_compacted += event.compacted_bytes;
        total_saved += event.saved_bytes;
        if event.compacted {
            compacted_events += 1;
        }
    }
    let savings = savings_percent(total_raw, total_saved);
    if as_json {
        println!(
            "{{\n  \"channel\": {},\n  \"events\": {},\n  \"compactedEvents\": {},\n  \"rawBytes\": {},\n  \"compactedBytes\": {},\n  \"savedBytes\": {},\n  \"savingsPercent\": {:.2}\n}}",
            json_string(&channel),
            events.len(),
            compacted_events,
            total_raw,
            total_compacted,
            total_saved,
            savings
        );
    } else {
        println!("wf-core token savings");
        println!("  Channel: {channel}");
        println!("  Events: {}", events.len());
        println!("  Compacted events: {compacted_events}");
        println!("  Raw bytes: {total_raw}");
        println!("  Compacted bytes: {total_compacted}");
        println!("  Saved bytes: {total_saved} ({savings:.2}%)");
    }
    Ok(0)
}

fn command_uninstall(arguments: &[String]) -> Result<i32, AppError> {
    if !has_flag(arguments, "--yes") {
        return Err(AppError::new("refusing to uninstall without --yes"));
    }
    let target = flag_value(arguments, "--target").unwrap_or_else(|| DEFAULT_TARGET.to_string());
    let channel = flag_value(arguments, "--channel").unwrap_or_else(|| "both".to_string());
    let targets = expand_targets(&target)?;
    if targets.contains(&"windsurf".to_string()) {
        for channel_name in expand_channels(&channel)? {
            uninstall_global_channel(&channel_name)?;
        }
    }
    if targets.contains(&"devin".to_string()) {
        uninstall_devin_global()?;
    }
    println!("wf-core global uninstall complete");
    Ok(0)
}

fn install_global_channel(
    channel: &str,
    source_root: &Path,
    summary: &mut InstallSummary,
) -> Result<(), AppError> {
    let source_windsurf = source_root.join(".windsurf");
    if !source_windsurf.exists() {
        return Err(AppError::new(format!(
            "source .windsurf directory not found: {}",
            display_path(&source_windsurf)
        )));
    }

    let home = channel_home(channel)?;
    for skill in SKILL_NAMES {
        sync_tree_delta(
            &source_windsurf.join("skills").join(skill),
            &home.join("skills").join(skill),
        )?;
    }
    copy_named_files(
        &source_windsurf.join("workflows"),
        &home.join("windsurf").join("workflows"),
        WORKFLOW_NAMES,
    )?;
    let global_rules = build_global_instruction_bundle(source_root, &source_windsurf)?;
    update_managed_block(
        &home.join("memories").join("global_rules.md"),
        &global_rules,
    )?;

    let cli_dir = home.join("wf-core");
    fs::create_dir_all(&cli_dir)?;
    let current_exe = env::current_exe()?;
    let installed_binary = installed_binary_path(channel)?;
    fs::copy(&current_exe, &installed_binary)?;
    let agents_source = source_root.join("AGENTS.md");
    if agents_source.exists() {
        fs::copy(&agents_source, cli_dir.join("AGENTS.md"))?;
    }
    install_bundle(source_root, &cli_dir.join("bundle"))?;
    write_install_metadata(channel, &cli_dir, source_root, &home)?;
    remove_known_stale_workflows(&home)?;
    remove_unlisted_wf_core_workflows(&home)?;
    let manifest = collect_installed_manifest(channel, &home)?;
    remove_stale_from_manifest(&home, &manifest)?;
    write_manifest(&home, &manifest)?;

    summary.messages.push(format!(
        "{channel}: installed global skills, workflows, global rules, and Rust binary at {}",
        display_path(&installed_binary)
    ));
    Ok(())
}

fn install_devin_global(source_root: &Path, summary: &mut InstallSummary) -> Result<(), AppError> {
    let source_windsurf = source_root.join(".windsurf");
    if !source_windsurf.exists() {
        return Err(AppError::new(format!(
            "source .windsurf directory not found: {}",
            display_path(&source_windsurf)
        )));
    }
    let home = devin_home()?;
    fs::create_dir_all(&home)?;
    for skill in SKILL_NAMES {
        sync_tree_delta(
            &source_windsurf.join("skills").join(skill),
            &home.join("skills").join(skill),
        )?;
    }
    install_devin_agents(&source_windsurf, &home)?;
    let cli_dir = home.join("wf-core");
    fs::create_dir_all(&cli_dir)?;
    let current_exe = env::current_exe()?;
    let installed_binary = devin_binary_path()?;
    fs::copy(&current_exe, &installed_binary)?;
    let agents_source = source_root.join("AGENTS.md");
    if agents_source.exists() {
        fs::copy(&agents_source, cli_dir.join("AGENTS.md"))?;
    }
    install_bundle(source_root, &cli_dir.join("bundle"))?;
    write_devin_install_metadata(&cli_dir, source_root, &home)?;
    write_devin_hook_artifact(&home, &installed_binary)?;
    update_devin_config(&home, &installed_binary)?;
    let manifest = collect_devin_manifest(&home)?;
    remove_stale_from_manifest(&home, &manifest)?;
    write_manifest(&home, &manifest)?;
    summary.messages.push(format!(
        "devin: installed global skills, hooks, config import, and Rust binary at {}",
        display_path(&installed_binary)
    ));
    Ok(())
}

fn install_devin_agents(source_windsurf: &Path, home: &Path) -> Result<(), AppError> {
    for skill in SKILL_NAMES {
        let skill_dir = source_windsurf.join("skills").join(skill);
        let agent_dir = home.join("agents").join(skill);
        let agent_path = agent_dir.join("AGENT.md");
        let profile = load_devin_agent_profile(&skill_dir)?;
        fs::create_dir_all(&agent_dir)?;
        fs::write(&agent_path, render_devin_agent(skill, &profile))?;
    }
    Ok(())
}

#[derive(Debug)]
struct DevinAgentProfile {
    description: String,
    prompt: String,
}

fn load_devin_agent_profile(skill_dir: &Path) -> Result<DevinAgentProfile, AppError> {
    let skill_markdown = read_text_if_exists(&skill_dir.join("SKILL.md"))?;
    let agent_config = read_text_if_exists(&skill_dir.join("agents").join("openai.yaml"))?;
    let description = yaml_scalar_field(&agent_config, "short_description")
        .or_else(|| yaml_scalar_field(&skill_markdown, "description"))
        .unwrap_or_else(|| "wf-core specialist agent".to_string());
    let prompt = yaml_scalar_field(&agent_config, "default_prompt").unwrap_or_else(|| {
        let body = markdown_body_after_frontmatter(&skill_markdown);
        if body.trim().is_empty() {
            "Use the matching wf-core skill guidance for this specialist task.".to_string()
        } else {
            body.trim().to_string()
        }
    });
    Ok(DevinAgentProfile {
        description,
        prompt,
    })
}

fn render_devin_agent(skill: &str, profile: &DevinAgentProfile) -> String {
    format!(
        "---
name: {skill}
description: {}
allowed-tools:
  - read
  - grep
  - glob
  - exec
---

{}
",
        json_string(&profile.description),
        profile.prompt.trim()
    )
}

fn uninstall_global_channel(channel: &str) -> Result<(), AppError> {
    let home = channel_home(channel)?;
    for skill in SKILL_NAMES {
        remove_dir_if_exists(&home.join("skills").join(skill))?;
        remove_dir_if_exists(&home.join("agents").join(skill))?;
    }
    for workflow in WORKFLOW_NAMES {
        remove_file_if_exists(&home.join("windsurf").join("workflows").join(workflow))?;
    }
    remove_managed_block(&home.join("memories").join("global_rules.md"))?;
    remove_dir_if_exists(&home.join("wf-core"))?;
    Ok(())
}

fn uninstall_devin_global() -> Result<(), AppError> {
    let home = devin_home()?;
    for skill in SKILL_NAMES {
        remove_dir_if_exists(&home.join("skills").join(skill))?;
    }
    remove_dir_if_exists(&home.join("wf-core"))?;
    Ok(())
}

fn verify_global_channel(channel: &str) -> Result<Vec<VerifyItem>, AppError> {
    let home = channel_home(channel)?;
    let mut items = vec![
        verify_item(format!("{channel} home"), home.clone()),
        verify_item(
            format!("{channel} global rules"),
            home.join("memories").join("global_rules.md"),
        ),
        verify_item(
            format!("{channel} workflows"),
            home.join("windsurf").join("workflows"),
        ),
        verify_item(
            format!("{channel} Rust binary"),
            installed_binary_path(channel)?,
        ),
        verify_item(
            format!("{channel} manifest"),
            home.join("wf-core").join(MANIFEST_FILE_NAME),
        ),
    ];
    for skill in SKILL_NAMES {
        items.push(verify_item(
            format!("{channel} skill {skill}"),
            home.join("skills").join(skill).join("SKILL.md"),
        ));
    }
    for entry in read_manifest(&home)? {
        let path = home.join(&entry.relative_path);
        let ok = path.exists()
            && file_checksum(&path)
                .map(|checksum| checksum == entry.checksum)
                .unwrap_or(false);
        items.push(VerifyItem {
            surface: format!("{channel} checksum {}", display_path(&entry.relative_path)),
            path,
            ok,
        });
    }
    for stale in stale_managed_files(&home)? {
        items.push(VerifyItem {
            surface: format!("{channel} stale managed file {}", display_path(&stale)),
            path: home.join(&stale),
            ok: false,
        });
    }
    Ok(items)
}

fn verify_devin_global() -> Result<Vec<VerifyItem>, AppError> {
    let home = devin_home()?;
    let config = home.join("config.json");
    let mut items = vec![
        verify_item("devin home".to_string(), home.clone()),
        verify_item("devin config".to_string(), config.clone()),
        verify_item(
            "devin hook artifact".to_string(),
            home.join("wf-core").join("devin-hooks.v1.json"),
        ),
        verify_item("devin Rust binary".to_string(), devin_binary_path()?),
        verify_item(
            "devin manifest".to_string(),
            home.join("wf-core").join(MANIFEST_FILE_NAME),
        ),
    ];
    let config_ok = config.exists()
        && fs::read_to_string(&config)
            .map(|content| content.contains("wf-core") && content.contains("read_config_from"))
            .unwrap_or(false);
    items.push(VerifyItem {
        surface: "devin config wf-core hooks/imports".to_string(),
        path: config,
        ok: config_ok,
    });
    for skill in SKILL_NAMES {
        items.push(verify_item(
            format!("devin skill {skill}"),
            home.join("skills").join(skill).join("SKILL.md"),
        ));
        items.push(verify_item(
            format!("devin agent {skill}"),
            home.join("agents").join(skill).join("AGENT.md"),
        ));
    }
    for entry in read_manifest(&home)? {
        let path = home.join(&entry.relative_path);
        let ok = path.exists()
            && file_checksum(&path)
                .map(|checksum| checksum == entry.checksum)
                .unwrap_or(false);
        items.push(VerifyItem {
            surface: format!("devin checksum {}", display_path(&entry.relative_path)),
            path,
            ok,
        });
    }
    for stale in stale_managed_files(&home)? {
        items.push(VerifyItem {
            surface: format!("devin stale managed file {}", display_path(&stale)),
            path: home.join(&stale),
            ok: false,
        });
    }
    Ok(items)
}

fn verify_item(surface: String, path: PathBuf) -> VerifyItem {
    let ok = path.exists();
    VerifyItem { surface, path, ok }
}

fn collect_installed_manifest(channel: &str, home: &Path) -> Result<Vec<ManifestEntry>, AppError> {
    let mut files = Vec::new();
    for skill in SKILL_NAMES {
        collect_files_recursive(home, &home.join("skills").join(skill), &mut files)?;
    }
    for workflow in WORKFLOW_NAMES {
        let path = home.join("windsurf").join("workflows").join(workflow);
        if path.exists() {
            files.push(relative_to(home, &path));
        }
    }
    let binary = installed_binary_path(channel)?;
    if binary.exists() {
        files.push(relative_to(home, &binary));
    }
    for relative in [
        PathBuf::from("wf-core").join("AGENTS.md"),
        PathBuf::from("wf-core").join("install-metadata.json"),
    ] {
        if home.join(&relative).exists() {
            files.push(relative);
        }
    }
    collect_files_recursive(home, &home.join("wf-core").join("bundle"), &mut files)?;
    files.sort();
    files.dedup();

    let mut manifest = Vec::new();
    for relative in files {
        let path = home.join(&relative);
        if path.is_file() {
            manifest.push(ManifestEntry {
                checksum: file_checksum(&path)?,
                size: fs::metadata(&path)?.len(),
                relative_path: relative,
            });
        }
    }
    Ok(manifest)
}

fn collect_devin_manifest(home: &Path) -> Result<Vec<ManifestEntry>, AppError> {
    let mut files = Vec::new();
    for skill in SKILL_NAMES {
        collect_files_recursive(home, &home.join("skills").join(skill), &mut files)?;
        collect_files_recursive(home, &home.join("agents").join(skill), &mut files)?;
    }
    let binary = devin_binary_path()?;
    if binary.exists() {
        files.push(relative_to(home, &binary));
    }
    for relative in [
        PathBuf::from("wf-core").join("AGENTS.md"),
        PathBuf::from("wf-core").join("install-metadata.json"),
        PathBuf::from("wf-core").join("devin-hooks.v1.json"),
    ] {
        if home.join(&relative).exists() {
            files.push(relative);
        }
    }
    collect_files_recursive(home, &home.join("wf-core").join("bundle"), &mut files)?;
    files.sort();
    files.dedup();
    let mut manifest = Vec::new();
    for relative in files {
        let path = home.join(&relative);
        if path.is_file() {
            manifest.push(ManifestEntry {
                checksum: file_checksum(&path)?,
                size: fs::metadata(&path)?.len(),
                relative_path: relative,
            });
        }
    }
    Ok(manifest)
}

fn write_manifest(home: &Path, manifest: &[ManifestEntry]) -> Result<(), AppError> {
    let path = home.join("wf-core").join(MANIFEST_FILE_NAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = String::from("# relative_path\tchecksum\tsize\n");
    for entry in manifest {
        content.push_str(&format!(
            "{}\t{}\t{}\n",
            path_to_manifest(&entry.relative_path),
            entry.checksum,
            entry.size
        ));
    }
    fs::write(path, content)?;
    Ok(())
}

fn read_manifest(home: &Path) -> Result<Vec<ManifestEntry>, AppError> {
    let path = home.join("wf-core").join(MANIFEST_FILE_NAME);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        entries.push(ManifestEntry {
            relative_path: PathBuf::from(parts[0].replace('/', std::path::MAIN_SEPARATOR_STR)),
            checksum: parts[1].to_string(),
            size: parts
                .get(2)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0),
        });
    }
    Ok(entries)
}

fn file_checksum(path: &Path) -> Result<String, AppError> {
    let bytes = fs::read(path)?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

fn remove_stale_from_manifest(home: &Path, current: &[ManifestEntry]) -> Result<(), AppError> {
    let current_paths: BTreeSet<String> = current
        .iter()
        .map(|entry| path_to_manifest(&entry.relative_path))
        .collect();
    for old_entry in read_manifest(home)? {
        let old_key = path_to_manifest(&old_entry.relative_path);
        if current_paths.contains(&old_key)
            || !is_safe_managed_relative_path(&old_entry.relative_path)
        {
            continue;
        }
        let absolute = home.join(&old_entry.relative_path);
        if absolute.is_file() {
            remove_file_if_exists(&absolute)?;
        }
    }
    Ok(())
}

fn stale_managed_files(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let manifest_paths: BTreeSet<String> = read_manifest(home)?
        .into_iter()
        .map(|entry| path_to_manifest(&entry.relative_path))
        .collect();
    let mut scanned = Vec::new();
    for skill in SKILL_NAMES {
        collect_files_recursive(home, &home.join("skills").join(skill), &mut scanned)?;
    }
    for workflow in managed_workflow_files(home)? {
        scanned.push(relative_to(home, &workflow));
    }
    collect_wf_core_files_for_stale_check(home, &mut scanned)?;
    scanned.sort();
    scanned.dedup();
    Ok(scanned
        .into_iter()
        .filter(|relative| !manifest_paths.contains(&path_to_manifest(relative)))
        .filter(|relative| is_safe_managed_relative_path(relative))
        .collect())
}

fn print_verify_json(ok: bool, items: &[VerifyItem]) {
    println!("{{");
    println!("  \"ok\": {ok},");
    println!("  \"results\": [");
    for (index, item) in items.iter().enumerate() {
        let comma = if index + 1 == items.len() { "" } else { "," };
        println!(
            "    {{\"surface\": {}, \"path\": {}, \"ok\": {}}}{comma}",
            json_string(&item.surface),
            json_string(&display_path(&item.path)),
            item.ok
        );
    }
    println!("  ]");
    println!("}}");
}

fn workspace_root_from_flag(arguments: &[String]) -> Result<PathBuf, AppError> {
    let raw = flag_value(arguments, "--workspace-root")
        .or_else(|| flag_value(arguments, "--repo-root"))
        .unwrap_or_else(|| ".".to_string());
    let candidate = PathBuf::from(raw);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        env::current_dir()?.join(candidate)
    };
    Ok(clean_path(&absolute))
}

fn repo_root_from_flag(arguments: &[String]) -> Result<PathBuf, AppError> {
    workspace_root_from_flag(arguments)
}

fn flow_artifact_path(channel: &str, workspace_root: &Path) -> Result<PathBuf, AppError> {
    Ok(channel_home(channel)?
        .join("wf-core")
        .join("memories")
        .join("workspaces")
        .join(workspace_key(workspace_root))
        .join("flow")
        .join("flow-check.json"))
}

fn requested_flow_artifact_path(
    arguments: &[String],
    default_path: PathBuf,
    output_flag: &str,
) -> PathBuf {
    flag_value(arguments, output_flag)
        .or_else(|| flag_value(arguments, "--path"))
        .map(PathBuf::from)
        .unwrap_or(default_path)
}

fn render_flow_template(task: &str, target_file: &str, target_function: &str) -> String {
    format!(
        "{{
  \"version\": 1,
  \"task\": {},
  \"target_file\": {},
  \"target_function\": {},
  \"current_behavior_to_preserve\": \"\",
  \"entry_point\": \"\",
  \"producer\": \"\",
  \"source_of_truth\": \"\",
  \"storage_state_queue_owner\": \"\",
  \"side_effect_owner\": \"\",
  \"consumers\": [],
  \"cleanup_recovery_path\": \"\",
  \"edit_boundary\": \"\",
  \"validation_needed\": [],
  \"validation_evidence\": [],
  \"duplicate_owner_logic\": false,
  \"migration_approved\": false,
  \"docs_only\": false,
  \"formatting_only\": false,
  \"generated_only\": false,
  \"greenfield\": false
}}
",
        json_string(task),
        json_string(&target_file.replace('\\', "/")),
        json_string(target_function)
    )
}

fn validate_flow_content(content: &str) -> Vec<String> {
    let mut findings = Vec::new();
    if json_number_field(content, "version").unwrap_or(0) != 1 {
        findings.push("version must be 1".to_string());
    }
    if json_string_field(content, "target_file")
        .map(|value| is_placeholder(&value))
        .unwrap_or(true)
    {
        findings.push("target_file is required".to_string());
    }
    let exempt = [
        "docs_only",
        "formatting_only",
        "generated_only",
        "greenfield",
    ]
    .iter()
    .any(|field| json_bool_field(content, field).unwrap_or(false));
    if exempt {
        return findings;
    }
    for field in [
        "current_behavior_to_preserve",
        "entry_point",
        "producer",
        "source_of_truth",
        "storage_state_queue_owner",
        "side_effect_owner",
        "cleanup_recovery_path",
        "edit_boundary",
    ] {
        if json_string_field(content, field)
            .map(|value| is_placeholder(&value))
            .unwrap_or(true)
        {
            findings.push(format!("{field} is required for existing-source edits"));
        }
    }
    for field in ["consumers", "validation_needed", "validation_evidence"] {
        if !json_array_has_value(content, field) {
            findings.push(format!("{field} must contain at least one item"));
        }
    }
    if json_bool_field(content, "duplicate_owner_logic").unwrap_or(false)
        && !json_bool_field(content, "migration_approved").unwrap_or(false)
    {
        findings.push("duplicate_owner_logic requires migration_approved evidence".to_string());
    }
    findings
}

fn is_placeholder(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("todo")
}

fn workflow_state_path(channel: &str, workspace_root: &Path) -> Result<PathBuf, AppError> {
    Ok(channel_home(channel)?
        .join("wf-core")
        .join("workflow")
        .join(format!("{}.state", workspace_key(workspace_root))))
}

fn workspace_memory_root(repo_root: &Path) -> Result<PathBuf, AppError> {
    Ok(wf_core_data_home()?
        .join("memories")
        .join("workspaces")
        .join(workspace_key(repo_root)))
}

fn system_map_path(repo_root: &Path) -> Result<PathBuf, AppError> {
    Ok(workspace_memory_root(repo_root)?.join("SYSTEM_MAP.md"))
}

fn memory_note_path(repo_root: &Path, kind: &str, key: &str) -> Result<PathBuf, AppError> {
    Ok(workspace_memory_root(repo_root)?
        .join(slugify(kind))
        .join(format!("{}.md", slugify(key))))
}

fn render_system_map(repo_root: &Path) -> Result<String, AppError> {
    let fingerprint = repo_inventory_fingerprint(repo_root)?;
    let mut files = Vec::new();
    collect_repo_inventory(repo_root, repo_root, &mut files)?;
    files.sort();

    let mut top_dirs = BTreeSet::new();
    for file in &files {
        if let Some(first) = file.components().next() {
            top_dirs.insert(first.as_os_str().to_string_lossy().to_string());
        }
    }

    let mut output = String::new();
    output.push_str("# wf-core System Map\n\n");
    output.push_str(&format!(
        "<!-- inventory_fingerprint: {fingerprint} -->\n\n"
    ));
    output.push_str(&format!("- generated_at: {}\n", now_millis() / 1000));
    output.push_str(&format!("- repository: {}\n", display_path(repo_root)));
    output.push_str(&format!("- files_indexed: {}\n\n", files.len()));

    output.push_str("## Top Level Areas\n\n");
    for dir in top_dirs {
        output.push_str(&format!("- {dir}\n"));
    }

    output.push_str("\n## Key Files\n\n");
    for file in files.iter().filter(|file| is_key_file(file)).take(80) {
        output.push_str(&format!("- {}\n", path_to_manifest(file)));
    }

    output.push_str("\n## Rust Command Surface\n\n");
    let main_rs = repo_root.join("src").join("main.rs");
    if main_rs.exists() {
        let content = fs::read_to_string(main_rs)?;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("fn command_") {
                output.push_str(&format!("- `{}`\n", trimmed.trim_end_matches(" {")));
            }
        }
    } else {
        output.push_str("- Not found\n");
    }

    output.push_str("\n## Managed Skills\n\n");
    let skills_dir = repo_root.join(".windsurf").join("skills");
    if skills_dir.exists() {
        for skill in SKILL_NAMES {
            if skills_dir.join(skill).join("SKILL.md").exists() {
                output.push_str(&format!("- `{skill}`\n"));
            }
        }
    } else {
        output.push_str("- Not found\n");
    }

    output.push_str("\n## Validation Commands\n\n");
    output.push_str("- `cargo fmt --check`\n");
    output.push_str("- `cargo test --locked`\n");
    output.push_str("- `cargo build --release --locked`\n");
    output.push_str("- `wf-core verify --target all --channel both`\n");
    Ok(output)
}

fn repo_inventory_fingerprint(repo_root: &Path) -> Result<String, AppError> {
    let mut files = Vec::new();
    collect_repo_inventory(repo_root, repo_root, &mut files)?;
    files.sort();
    let mut hash: u64 = 0xcbf29ce484222325;
    for relative in files {
        let absolute = repo_root.join(&relative);
        let metadata = fs::metadata(&absolute)?;
        for byte in path_to_manifest(&relative).as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for byte in metadata.len().to_string().as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("{hash:016x}"))
}

fn collect_repo_inventory(
    repo_root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    if should_skip_repo_path(repo_root, directory) {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if should_skip_repo_path(repo_root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_repo_inventory(repo_root, &path, output)?;
        } else if path.is_file() && is_source_like_file(&path) {
            output.push(relative_to(repo_root, &path));
        }
    }
    Ok(())
}

fn should_skip_repo_path(repo_root: &Path, path: &Path) -> bool {
    let relative = relative_to(repo_root, path);
    let value = path_to_manifest(&relative);
    value == ".git"
        || value.starts_with(".git/")
        || value == "target"
        || value.starts_with("target/")
        || value.contains("/node_modules/")
        || value.ends_with("/node_modules")
}

fn is_source_like_file(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "rs" | "toml" | "lock" | "md" | "yml" | "yaml" | "json" | "ps1" | "sh" | "cmd" => true,
        _ => false,
    }
}

fn is_key_file(path: &Path) -> bool {
    let value = path_to_manifest(path);
    value == "Cargo.toml"
        || value == "README.md"
        || value == "AGENTS.md"
        || value.ends_with("/SKILL.md")
        || value.ends_with("validate.yml")
        || value.starts_with("src/")
        || value.starts_with("docs/")
}

fn list_files_with_extension(path: &Path, extension: &str) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    if !path.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn run_review_gate(arguments: &[String]) -> Result<i32, AppError> {
    let repo_root = repo_root_from_flag(arguments)?;
    let format = flag_value(arguments, "--format").unwrap_or_else(|| "markdown".to_string());
    let repo_test_policy =
        flag_value(arguments, "--repo-test-policy").unwrap_or_else(|| "skip".to_string());
    let mut report = build_review_report(&repo_root)?;
    if repo_test_policy != "skip" {
        run_optional_repo_tests(&repo_root, &mut report)?;
    }
    report.ok = report
        .findings
        .iter()
        .all(|finding| finding.severity != "blocking");
    match format.as_str() {
        "json" => println!("{}", render_review_json(&report)),
        "compact" => println!(
            "gate={} blocking={} findings={} changed_files={}",
            if report.ok { "pass" } else { "block" },
            report
                .findings
                .iter()
                .filter(|finding| finding.severity == "blocking")
                .count(),
            report.findings.len(),
            report.changed_files.len()
        ),
        _ => print!("{}", render_review_markdown(&report)),
    }
    Ok(if report.ok { 0 } else { 1 })
}

fn build_review_report(repo_root: &Path) -> Result<ReviewReport, AppError> {
    let mut findings = Vec::new();
    if !repo_root.exists() {
        findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: format!("repo root does not exist: {}", display_path(repo_root)),
        });
    }
    let branch = git_output(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    let changed_files = git_changed_files(repo_root).unwrap_or_default();
    match git_status(repo_root) {
        Ok(status) => {
            if status.lines().any(|line| line.starts_with("??")) {
                findings.push(ReviewFinding {
                    severity: "warning".to_string(),
                    message: "untracked files are present; confirm they are intentional"
                        .to_string(),
                });
            }
        }
        Err(error) => findings.push(ReviewFinding {
            severity: "warning".to_string(),
            message: format!("unable to read git status: {}", error.message),
        }),
    }
    match git_command_status(repo_root, &["diff", "--check"]) {
        Ok(0) => {}
        Ok(_) => findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: "git diff --check reported whitespace or conflict-marker issues".to_string(),
        }),
        Err(error) => findings.push(ReviewFinding {
            severity: "warning".to_string(),
            message: format!("unable to run git diff --check: {}", error.message),
        }),
    }
    let ok = findings
        .iter()
        .all(|finding| finding.severity != "blocking");
    Ok(ReviewReport {
        ok,
        repo_root: repo_root.to_path_buf(),
        branch,
        changed_files,
        findings,
    })
}

fn run_optional_repo_tests(repo_root: &Path, report: &mut ReviewReport) -> Result<(), AppError> {
    if !repo_root.join("Cargo.toml").exists() {
        report.findings.push(ReviewFinding {
            severity: "warning".to_string(),
            message: "repo-test-policy requested tests, but Cargo.toml was not found".to_string(),
        });
        return Ok(());
    }
    match run_command_capture(repo_root, "cargo", &["test", "--locked"]) {
        Ok(output) if output.status.code().unwrap_or(1) == 0 => Ok(()),
        Ok(output) => {
            report.findings.push(ReviewFinding {
                severity: "blocking".to_string(),
                message: format!(
                    "cargo test --locked failed with exit code {}",
                    output.status.code().unwrap_or(1)
                ),
            });
            Ok(())
        }
        Err(error) => {
            report.findings.push(ReviewFinding {
                severity: "blocking".to_string(),
                message: format!("unable to run cargo test --locked: {}", error.message),
            });
            Ok(())
        }
    }
}

fn render_review_markdown(report: &ReviewReport) -> String {
    let mut output = String::new();
    output.push_str("# wf-core Review Gate\n\n");
    output.push_str(&format!(
        "- Gate: {}\n- Repository: {}\n- Branch: {}\n- Changed files: {}\n\n",
        if report.ok { "pass" } else { "block" },
        display_path(&report.repo_root),
        report.branch,
        report.changed_files.len()
    ));
    if report.findings.is_empty() {
        output.push_str("## Findings\n\nNo blocking findings detected.\n\n");
    } else {
        output.push_str("## Findings\n\n");
        for finding in &report.findings {
            output.push_str(&format!("- {}: {}\n", finding.severity, finding.message));
        }
        output.push('\n');
    }
    if !report.changed_files.is_empty() {
        output.push_str("## Changed Files\n\n");
        for file in report.changed_files.iter().take(50) {
            output.push_str(&format!("- {file}\n"));
        }
        if report.changed_files.len() > 50 {
            output.push_str(&format!("- ... {} more\n", report.changed_files.len() - 50));
        }
    }
    output
}

fn render_review_json(report: &ReviewReport) -> String {
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str(&format!("  \"ok\": {},\n", report.ok));
    output.push_str(&format!(
        "  \"gate\": {},\n",
        json_string(if report.ok { "pass" } else { "block" })
    ));
    output.push_str(&format!(
        "  \"repoRoot\": {},\n",
        json_string(&display_path(&report.repo_root))
    ));
    output.push_str(&format!("  \"branch\": {},\n", json_string(&report.branch)));
    output.push_str("  \"changedFiles\": [");
    for (index, file) in report.changed_files.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&json_string(file));
    }
    output.push_str("],\n");
    output.push_str("  \"findings\": [\n");
    for (index, finding) in report.findings.iter().enumerate() {
        let comma = if index + 1 == report.findings.len() {
            ""
        } else {
            ","
        };
        output.push_str(&format!(
            "    {{\"severity\": {}, \"message\": {}}}{comma}\n",
            json_string(&finding.severity),
            json_string(&finding.message)
        ));
    }
    output.push_str("  ]\n}");
    output
}

fn render_commit_message(repo_root: &Path, test_result: &str) -> Result<String, AppError> {
    let files = git_changed_files(repo_root).unwrap_or_default();
    let subject = suggested_subject(&files);
    Ok(format!(
        "{subject}\n\nProblem:\n- Keep the managed Windsurf surfaces aligned with the requested wf-core behavior.\n\nSolution:\n- Update the Rust-native CLI and managed guidance surfaces.\n\nSummary:\n{}\n\nTest Result:\n- {}\n",
        render_file_summary(&files),
        test_result.trim()
    ))
}

fn render_pr_body(repo_root: &Path, test_result: &str) -> Result<String, AppError> {
    let files = git_changed_files(repo_root).unwrap_or_default();
    Ok(format!(
        "## Summary\n{}\n\n## Test Result\n- {}\n\n## Review Checklist\n- [ ] Flow evidence is complete for existing-source edits\n- [ ] Review gates pass\n- [ ] Global install/verify surfaces were checked\n",
        render_file_summary(&files),
        test_result.trim()
    ))
}

fn lint_message_text(content: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let mut lines = content.lines();
    let subject = lines.next().unwrap_or("").trim_end();
    if subject.is_empty() {
        findings.push("subject line is required".to_string());
    }
    if subject.chars().count() > 72 {
        findings.push("subject line should be 72 characters or fewer".to_string());
    }
    let allowed = [
        "feat:",
        "fix:",
        "improve:",
        "add:",
        "docs:",
        "test:",
        "refactor:",
        "chore:",
        "build:",
        "ci:",
    ];
    if !allowed.iter().any(|prefix| subject.starts_with(prefix)) {
        findings.push(
            "subject should start with a conventional prefix such as feat:, fix:, docs:, or chore:"
                .to_string(),
        );
    }
    for (index, line) in content.lines().enumerate() {
        if line.ends_with(' ') || line.ends_with('\t') {
            findings.push(format!("line {} has trailing whitespace", index + 1));
        }
    }
    if content.lines().count() > 1 && !content.contains("Test Result") {
        findings.push("message body should include a Test Result section".to_string());
    }
    findings
}

fn run_git_preflight(arguments: &[String]) -> Result<i32, AppError> {
    let repo_root = repo_root_from_flag(arguments)?;
    let base_ref = flag_value(arguments, "--base-ref").unwrap_or_else(|| "origin/main".to_string());
    let mut report = build_review_report(&repo_root)?;
    if report.branch == "HEAD" || report.branch == "unknown" {
        report.findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: "preflight requires a named git branch".to_string(),
        });
    } else if !valid_branch_name(&report.branch) {
        report.findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: format!("branch should start with feat/, fix/, improve/, add/, docs/, chore/, refactor/, or test/: {}", report.branch),
        });
    }
    if git_command_status(&repo_root, &["rev-parse", "--verify", &base_ref]).unwrap_or(1) != 0 {
        report.findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: format!("base ref is not visible locally: {base_ref}"),
        });
    }
    let status = git_status(&repo_root).unwrap_or_default();
    if !status.trim().is_empty() {
        report.findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: "working tree has uncommitted or untracked files".to_string(),
        });
    }
    if report.changed_files.is_empty() {
        report.findings.push(ReviewFinding {
            severity: "blocking".to_string(),
            message: "no changed files detected for preflight".to_string(),
        });
    }
    report.ok = report
        .findings
        .iter()
        .all(|finding| finding.severity != "blocking");
    print!("{}", render_review_markdown(&report));
    Ok(if report.ok { 0 } else { 1 })
}

fn git_changed_files(repo_root: &Path) -> Result<Vec<String>, AppError> {
    let output = git_output(repo_root, &["diff", "--name-only", "HEAD"]).unwrap_or_default();
    let mut files: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect();
    let status = git_status(repo_root).unwrap_or_default();
    for line in status.lines() {
        if line.starts_with("??") {
            let file = line.trim_start_matches("??").trim();
            if !file.is_empty() {
                files.push(file.to_string());
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn git_status(repo_root: &Path) -> Result<String, AppError> {
    git_output(repo_root, &["status", "--porcelain"])
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String, AppError> {
    let output = run_command_capture(repo_root, "git", args)?;
    if !output.status.success() {
        return Err(AppError::new(format!(
            "git {} failed with exit code {}",
            args.join(" "),
            output.status.code().unwrap_or(1)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_command_status(repo_root: &Path, args: &[&str]) -> Result<i32, AppError> {
    let output = run_command_capture(repo_root, "git", args)?;
    Ok(output.status.code().unwrap_or(1))
}

fn run_command_capture(repo_root: &Path, program: &str, args: &[&str]) -> Result<Output, AppError> {
    Ok(Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output()?)
}

fn render_file_summary(files: &[String]) -> String {
    if files.is_empty() {
        return "- No changed files detected.".to_string();
    }
    let mut output = String::new();
    for file in files.iter().take(25) {
        output.push_str(&format!("- {file}\n"));
    }
    if files.len() > 25 {
        output.push_str(&format!("- ... {} more\n", files.len() - 25));
    }
    output.trim_end().to_string()
}

fn suggested_subject(files: &[String]) -> String {
    if files
        .iter()
        .any(|file| file.starts_with("docs/") || file.ends_with(".md"))
    {
        "docs: update wf-core guidance".to_string()
    } else if files.iter().any(|file| file.starts_with("src/")) {
        "feat: extend wf-core native workflow gates".to_string()
    } else {
        "chore: update wf-core managed surfaces".to_string()
    }
}

fn valid_branch_name(branch: &str) -> bool {
    [
        "feat/",
        "fix/",
        "improve/",
        "add/",
        "docs/",
        "chore/",
        "refactor/",
        "test/",
    ]
    .iter()
    .any(|prefix| branch.starts_with(prefix))
}

fn render_findings_json(valid: bool, path: &Path, findings: &[String]) -> String {
    let mut output = String::new();
    output.push_str("{\"valid\":");
    output.push_str(if valid { "true" } else { "false" });
    output.push_str(",\"path\":");
    output.push_str(&json_string(&display_path(path)));
    output.push_str(",\"errors\":[");
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&json_string(finding));
    }
    output.push_str("]}");
    output
}

fn execute_command(command_args: &[String], shell_mode: bool) -> Result<Output, AppError> {
    if shell_mode {
        let command_text = command_args.join(" ");
        let mut command = if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg(command_text);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(command_text);
            cmd
        };
        Ok(command.output()?)
    } else {
        let mut command = Command::new(&command_args[0]);
        if command_args.len() > 1 {
            command.args(&command_args[1..]);
        }
        Ok(command.output()?)
    }
}

fn compact_output(
    command: &str,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    raw_path: PathBuf,
    max_lines: usize,
    max_bytes: usize,
    elapsed_ms: u128,
) -> Compaction {
    let raw = render_raw_output(stdout, stderr);
    let raw_bytes = raw.as_bytes().len();
    let lines: Vec<&str> = raw.lines().collect();
    let high_signal_indexes: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            if is_high_signal(line) {
                Some(index)
            } else {
                None
            }
        })
        .collect();
    let should_compact = raw_bytes > max_bytes || lines.len() > max_lines;
    if !should_compact {
        return Compaction {
            rendered: raw,
            compacted: false,
            raw_bytes,
            compacted_bytes: raw_bytes,
            saved_bytes: 0,
            high_signal_count: high_signal_indexes.len(),
            raw_path,
        };
    }

    let mut selected = HashSet::new();
    for index in high_signal_indexes.iter().take(80) {
        let start = index.saturating_sub(2);
        let end = (*index + 3).min(lines.len());
        for candidate in start..end {
            selected.insert(candidate);
        }
    }
    let mut selected_indexes: Vec<usize> = selected.into_iter().collect();
    selected_indexes.sort_unstable();

    let mut rendered = String::new();
    rendered.push_str("[wf-core] compacted command output\n");
    rendered.push_str(&format!("command: {command}\n"));
    rendered.push_str(&format!("exit code: {exit_code}\n"));
    rendered.push_str(&format!("elapsed: {elapsed_ms} ms\n"));
    rendered.push_str(&format!("raw: {raw_bytes} bytes, {} lines\n", lines.len()));
    rendered.push_str(&format!(
        "high-signal lines: {}\n",
        high_signal_indexes.len()
    ));
    rendered.push_str(&format!("raw output: {}\n\n", display_path(&raw_path)));

    if !selected_indexes.is_empty() {
        rendered.push_str("## High Signal\n");
        let mut seen = HashSet::new();
        for index in selected_indexes {
            let line = lines[index];
            if seen.insert(line) {
                rendered.push_str(line);
                rendered.push('\n');
            }
        }
        rendered.push('\n');
    }

    rendered.push_str("## Head\n");
    for line in lines.iter().take(30) {
        rendered.push_str(line);
        rendered.push('\n');
    }
    rendered.push('\n');

    if lines.len() > 40 {
        rendered.push_str("## Tail\n");
        for line in lines.iter().skip(lines.len().saturating_sub(40)) {
            rendered.push_str(line);
            rendered.push('\n');
        }
    }

    let compacted_bytes = rendered.as_bytes().len();
    Compaction {
        rendered,
        compacted: true,
        raw_bytes,
        compacted_bytes,
        saved_bytes: raw_bytes.saturating_sub(compacted_bytes),
        high_signal_count: high_signal_indexes.len(),
        raw_path,
    }
}

fn render_raw_output(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("[stdout]\n{stdout}\n[stderr]\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) => String::new(),
    }
}

fn save_raw_output(channel: &str, command: &str, raw_text: &str) -> Result<PathBuf, AppError> {
    let output_dir = channel_home(channel)?.join("wf-core").join("raw-output");
    fs::create_dir_all(&output_dir)?;
    let file_name = format!("{}-{}.log", now_millis(), slugify(command));
    let path = output_dir.join(file_name);
    fs::write(&path, raw_text)?;
    Ok(path)
}

fn record_gain_event(
    channel: &str,
    command: &str,
    exit_code: i32,
    compaction: &Compaction,
) -> Result<(), AppError> {
    let event_path = gain_events_path(channel)?;
    if let Some(parent) = event_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let event = format!(
        "{{\"time\":{},\"command\":{},\"exitCode\":{},\"compacted\":{},\"rawBytes\":{},\"compactedBytes\":{},\"savedBytes\":{},\"highSignalCount\":{},\"rawPath\":{}}}\n",
        now_millis() / 1000,
        json_string(command),
        exit_code,
        compaction.compacted,
        compaction.raw_bytes,
        compaction.compacted_bytes,
        compaction.saved_bytes,
        compaction.high_signal_count,
        json_string(&display_path(&compaction.raw_path))
    );
    append_text(&event_path, &event)?;
    Ok(())
}

fn read_gain_events(channel: &str) -> Result<Vec<GainEvent>, AppError> {
    let path = gain_events_path(channel)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for line in content.lines() {
        let event = GainEvent {
            raw_bytes: json_number_field(line, "rawBytes").unwrap_or(0),
            compacted_bytes: json_number_field(line, "compactedBytes").unwrap_or(0),
            saved_bytes: json_number_field(line, "savedBytes").unwrap_or(0),
            compacted: json_bool_field(line, "compacted").unwrap_or(false),
        };
        events.push(event);
    }
    Ok(events)
}

fn gain_events_path(channel: &str) -> Result<PathBuf, AppError> {
    Ok(channel_home(channel)?
        .join("wf-core")
        .join("gain")
        .join("events.jsonl"))
}

fn is_supported_noisy_command(command_text: &str) -> (bool, String) {
    let tokens = split_command_for_detection(command_text);
    if tokens.is_empty() {
        return (false, "empty command".to_string());
    }
    let mut root = Path::new(&tokens[0])
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&tokens[0])
        .to_ascii_lowercase();
    if root.ends_with(".cmd") || root.ends_with(".exe") {
        root = root
            .trim_end_matches(".cmd")
            .trim_end_matches(".exe")
            .to_string();
    }
    if root == "git" {
        if tokens
            .get(1)
            .map(|subcommand| {
                NOISY_GIT_SUBCOMMANDS.contains(&subcommand.to_ascii_lowercase().as_str())
            })
            .unwrap_or(false)
        {
            return (true, "supported git command".to_string());
        }
        return (
            false,
            "git command is not in the noisy command allowlist".to_string(),
        );
    }
    if NOISY_ROOT_COMMANDS.contains(&root.as_str()) {
        return (true, "supported noisy command family".to_string());
    }
    if requires_shell(command_text) {
        return (true, "shell pipeline or compound command".to_string());
    }
    (
        false,
        "command is not recognized as noisy; run it raw or use wf-core run manually".to_string(),
    )
}

fn split_command_for_detection(command_text: &str) -> Vec<String> {
    command_text
        .split_whitespace()
        .map(|value| value.trim_matches('"').trim_matches('\'').to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn requires_shell(command_text: &str) -> bool {
    SHELL_MARKERS
        .iter()
        .any(|marker| command_text.contains(marker))
}

fn is_high_signal(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    HIGH_SIGNAL_TERMS.iter().any(|term| lowered.contains(term))
}

fn resolve_source_root(raw: Option<String>) -> Result<PathBuf, AppError> {
    if let Some(value) = raw {
        return Ok(PathBuf::from(value).canonicalize()?);
    }
    let current_dir = env::current_dir()?;
    if current_dir.join(".windsurf").exists() {
        return Ok(current_dir);
    }
    let exe = env::current_exe()?;
    if let Some(exe_dir) = exe.parent() {
        let bundled = exe_dir.join("bundle");
        if bundled.join(".windsurf").exists() {
            return Ok(bundled);
        }
    }
    Err(AppError::new(
        "unable to resolve source root; pass --source-root <repo>",
    ))
}

fn install_source_from_metadata(channel: &str) -> Result<PathBuf, AppError> {
    let home = channel_home(channel)?;
    let cli_dir = home.join("wf-core");
    let metadata_path = cli_dir.join("install-metadata.json");
    if metadata_path.exists() {
        let metadata = fs::read_to_string(&metadata_path)?;
        if let Some(source_root) = json_string_field(&metadata, "sourceRoot") {
            let candidate = PathBuf::from(source_root);
            if candidate.join(".windsurf").exists() {
                return Ok(candidate.canonicalize()?);
            }
        }
    }
    let bundled = cli_dir.join("bundle");
    if bundled.join(".windsurf").exists() {
        return Ok(bundled);
    }
    Err(AppError::new(format!(
        "unable to resolve update source for {channel}; pass --source-root <repo>"
    )))
}

fn install_source_from_devin_metadata() -> Result<PathBuf, AppError> {
    let home = devin_home()?;
    let cli_dir = home.join("wf-core");
    let metadata_path = cli_dir.join("install-metadata.json");
    if metadata_path.exists() {
        let metadata = fs::read_to_string(&metadata_path)?;
        if let Some(source_root) = json_string_field(&metadata, "sourceRoot") {
            let candidate = PathBuf::from(source_root);
            if candidate.join(".windsurf").exists() {
                return Ok(candidate.canonicalize()?);
            }
        }
    }
    let bundled = cli_dir.join("bundle");
    if bundled.join(".windsurf").exists() {
        return Ok(bundled);
    }
    Err(AppError::new(
        "unable to resolve Devin update source; pass --source-root <repo>",
    ))
}

fn expand_channels(channel: &str) -> Result<Vec<String>, AppError> {
    if channel == "both" {
        return Ok(DEFAULT_CHANNELS
            .iter()
            .map(|value| value.to_string())
            .collect());
    }
    match channel {
        "stable" | "next" | "insiders" | "windsurf" | "windsurf-next" | "windsurf-insiders" => {
            Ok(vec![canonical_channel(channel).to_string()])
        }
        _ => Err(AppError::new(format!("unknown channel: {channel}"))),
    }
}

fn expand_targets(target: &str) -> Result<Vec<String>, AppError> {
    match target {
        "all" | "both" => Ok(vec!["windsurf".to_string(), "devin".to_string()]),
        "windsurf" | "codeium" => Ok(vec!["windsurf".to_string()]),
        "devin" | "devin-local" => Ok(vec!["devin".to_string()]),
        other => Err(AppError::new(format!("unknown target: {other}"))),
    }
}

fn canonical_channel(channel: &str) -> &str {
    match channel {
        "windsurf" => "stable",
        "windsurf-next" => "next",
        "windsurf-insiders" => "insiders",
        other => other,
    }
}

fn channel_directory_name(channel: &str) -> Result<&'static str, AppError> {
    match canonical_channel(channel) {
        "stable" => Ok("windsurf"),
        "next" => Ok("windsurf-next"),
        "insiders" => Ok("windsurf-insiders"),
        other => Err(AppError::new(format!("unknown channel: {other}"))),
    }
}

fn codeium_root() -> Result<PathBuf, AppError> {
    if let Ok(value) = env::var("CODEIUM_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    Ok(home_dir()?.join(".codeium"))
}

fn home_dir() -> Result<PathBuf, AppError> {
    if let Ok(value) = env::var("USERPROFILE") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    if let Ok(value) = env::var("HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    Err(AppError::new("unable to resolve home directory"))
}

fn channel_home(channel: &str) -> Result<PathBuf, AppError> {
    Ok(codeium_root()?.join(channel_directory_name(channel)?))
}

fn devin_home() -> Result<PathBuf, AppError> {
    if let Ok(value) = env::var("DEVIN_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    if cfg!(windows) {
        if let Ok(value) = env::var("APPDATA") {
            if !value.trim().is_empty() {
                return Ok(PathBuf::from(value).join("devin"));
            }
        }
        return Ok(home_dir()?.join("AppData").join("Roaming").join("devin"));
    }
    if let Ok(value) = env::var("XDG_CONFIG_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value).join("devin"));
        }
    }
    Ok(home_dir()?.join(".config").join("devin"))
}

fn wf_core_data_home() -> Result<PathBuf, AppError> {
    if let Ok(value) = env::var("WF_CORE_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    if cfg!(windows) {
        if let Ok(value) = env::var("APPDATA") {
            if !value.trim().is_empty() {
                return Ok(PathBuf::from(value).join("wf-core"));
            }
        }
        return Ok(home_dir()?.join("AppData").join("Roaming").join("wf-core"));
    }
    if let Ok(value) = env::var("XDG_DATA_HOME") {
        if !value.trim().is_empty() {
            return Ok(PathBuf::from(value).join("wf-core"));
        }
    }
    Ok(home_dir()?.join(".local").join("share").join("wf-core"))
}

fn installed_binary_path(channel: &str) -> Result<PathBuf, AppError> {
    let file_name = if cfg!(windows) {
        "wf-core.exe"
    } else {
        "wf-core"
    };
    Ok(channel_home(channel)?.join("wf-core").join(file_name))
}

fn devin_binary_path() -> Result<PathBuf, AppError> {
    let file_name = if cfg!(windows) {
        "wf-core.exe"
    } else {
        "wf-core"
    };
    Ok(devin_home()?.join("wf-core").join(file_name))
}

fn current_wrapper_command() -> Result<String, AppError> {
    Ok(quote_arg(&display_path(&env::current_exe()?)))
}

fn copy_tree_contents(source: &Path, target: &Path) -> Result<(), AppError> {
    if !source.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            fs::create_dir_all(&destination)?;
            copy_tree_contents(&path, &destination)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

fn copy_named_files(source: &Path, target: &Path, names: &[&str]) -> Result<(), AppError> {
    fs::create_dir_all(target)?;
    for name in names {
        let source_path = source.join(name);
        if source_path.exists() {
            fs::copy(&source_path, target.join(name))?;
        }
    }
    Ok(())
}

fn sync_tree_delta(source: &Path, target: &Path) -> Result<(), AppError> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(target)?;
    let mut source_names = BTreeSet::new();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        source_names.insert(file_name.to_string_lossy().to_string());
        let source_path = entry.path();
        let target_path = target.join(&file_name);
        if source_path.is_dir() {
            sync_tree_delta(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_file_if_changed(&source_path, &target_path)?;
        }
    }
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !source_names.contains(&name) {
            remove_path_if_exists(&entry.path())?;
        }
    }
    Ok(())
}

fn copy_file_if_changed(source: &Path, target: &Path) -> Result<(), AppError> {
    let source_bytes = fs::read(source)?;
    if fs::read(target)
        .map(|target_bytes| target_bytes == source_bytes)
        .unwrap_or(false)
    {
        return Ok(());
    }
    fs::write(target, source_bytes)?;
    Ok(())
}

fn collect_files_recursive(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, output)?;
        } else if path.is_file() {
            output.push(relative_to(root, &path));
        }
    }
    Ok(())
}

fn collect_wf_core_files_for_stale_check(
    home: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    let root = home.join("wf-core");
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if name == MANIFEST_FILE_NAME
            || name == "raw-output"
            || name == "gain"
            || name == "memories"
            || name.starts_with("config.backup.")
        {
            continue;
        }
        if path.is_dir() {
            collect_files_recursive(home, &path, output)?;
        } else if path.is_file() {
            output.push(relative_to(home, &path));
        }
    }
    Ok(())
}

fn relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn path_to_manifest(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_safe_managed_relative_path(relative: &Path) -> bool {
    let value = path_to_manifest(relative);
    if value.starts_with("skills/") {
        return true;
    }
    if SKILL_NAMES
        .iter()
        .any(|skill| value.starts_with(&format!("agents/{skill}/")))
    {
        return true;
    }
    if value.starts_with("windsurf/workflows/wf-core-") {
        return true;
    }
    value.starts_with("wf-core/")
        && !value.starts_with("wf-core/raw-output/")
        && !value.starts_with("wf-core/gain/")
        && !value.starts_with("wf-core/memories/")
        && value != format!("wf-core/{MANIFEST_FILE_NAME}")
}

fn update_managed_block(path: &Path, content: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let block = format!("{MANAGED_START}\n{}\n{MANAGED_END}\n", content.trim());
    let updated = replace_or_append_managed_block(&existing, &block);
    fs::write(path, updated)?;
    Ok(())
}

fn remove_managed_block(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(path)?;
    let updated = remove_managed_region(&existing);
    fs::write(path, updated)?;
    Ok(())
}

fn replace_or_append_managed_block(existing: &str, block: &str) -> String {
    if let (Some(start), Some(end)) = (existing.find(MANAGED_START), existing.find(MANAGED_END)) {
        let end_index = end + MANAGED_END.len();
        let mut output = String::new();
        output.push_str(&existing[..start]);
        output.push_str(block);
        output.push_str(existing[end_index..].trim_start_matches(['\r', '\n']));
        return output;
    }
    if existing.trim().is_empty() {
        block.to_string()
    } else {
        format!("{}\n\n{}", existing.trim_end(), block)
    }
}

fn remove_managed_region(existing: &str) -> String {
    if let (Some(start), Some(end)) = (existing.find(MANAGED_START), existing.find(MANAGED_END)) {
        let end_index = end + MANAGED_END.len();
        let mut output = String::new();
        output.push_str(existing[..start].trim_end());
        output.push('\n');
        output.push_str(existing[end_index..].trim_start_matches(['\r', '\n']));
        let trimmed = output.trim();
        return if trimmed.is_empty() {
            String::new()
        } else {
            format!("{trimmed}\n")
        };
    }
    existing.to_string()
}

fn build_global_instruction_bundle(
    source_root: &Path,
    source_windsurf: &Path,
) -> Result<String, AppError> {
    let mut bundle = String::new();
    bundle.push_str("# wf-core Managed Global Instruction Bundle\n\n");
    bundle.push_str(&read_text_if_exists(
        &source_windsurf.join("global_rules.md"),
    )?);

    let rules_dir = source_windsurf.join("rules");
    if rules_dir.exists() {
        let mut rule_files = Vec::new();
        for entry in fs::read_dir(&rules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("md") {
                rule_files.push(path);
            }
        }
        rule_files.sort();
        for path in rule_files {
            bundle.push_str("\n\n---\n\n");
            bundle.push_str(&format!(
                "# Rule Source: {}\n\n",
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("rule.md")
            ));
            bundle.push_str(&strip_frontmatter(&read_text_if_exists(&path)?));
        }
    }

    let agents_path = source_root.join("AGENTS.md");
    if agents_path.exists() {
        bundle.push_str("\n\n---\n\n# AGENTS Guidance\n\n");
        bundle.push_str(&read_text_if_exists(&agents_path)?);
    }
    Ok(bundle)
}

fn read_text_if_exists(path: &Path) -> Result<String, AppError> {
    if path.exists() {
        Ok(fs::read_to_string(path)?)
    } else {
        Ok(String::new())
    }
}

fn strip_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content.to_string();
    }
    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body = Vec::new();
    let mut in_frontmatter = true;
    for line in lines {
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        body.push(line);
    }
    body.join("\n")
}

fn markdown_body_after_frontmatter(content: &str) -> String {
    strip_frontmatter(content)
}

fn yaml_scalar_field(content: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            continue;
        }
        let value = trimmed[prefix.len()..].trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            return decode_basic_quoted_string(value);
        }
        return Some(value.trim_matches('\'').to_string());
    }
    None
}

fn decode_basic_quoted_string(value: &str) -> Option<String> {
    let mut characters = value.chars();
    if characters.next()? != '"' || !value.ends_with('"') {
        return None;
    }
    let inner = &value[1..value.len() - 1];
    let mut output = String::new();
    let mut escaped = false;
    for character in inner.chars() {
        if escaped {
            match character {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    Some(output)
}

fn install_bundle(source_root: &Path, bundle_root: &Path) -> Result<(), AppError> {
    remove_dir_if_exists(bundle_root)?;
    fs::create_dir_all(bundle_root)?;
    for relative in [".windsurf", "docs", "templates"] {
        let source = source_root.join(relative);
        if source.exists() {
            copy_tree_contents(&source, &bundle_root.join(relative))?;
        }
    }
    for file_name in ["README.md", "AGENTS.md"] {
        let source = source_root.join(file_name);
        if source.exists() {
            fs::copy(source, bundle_root.join(file_name))?;
        }
    }
    Ok(())
}

fn write_install_metadata(
    channel: &str,
    cli_dir: &Path,
    source_root: &Path,
    home: &Path,
) -> Result<(), AppError> {
    let payload = format!(
        "{{\n  \"version\": {},\n  \"channel\": {},\n  \"installedAt\": {},\n  \"installMode\": \"global-only\",\n  \"runtime\": \"rust\",\n  \"sourceRoot\": {},\n  \"home\": {}\n}}\n",
        json_string(VERSION),
        json_string(channel),
        now_millis() / 1000,
        json_string(&display_path(source_root)),
        json_string(&display_path(home))
    );
    fs::write(cli_dir.join("install-metadata.json"), payload)?;
    Ok(())
}

fn write_devin_install_metadata(
    cli_dir: &Path,
    source_root: &Path,
    home: &Path,
) -> Result<(), AppError> {
    let payload = format!(
        "{{\n  \"version\": {},\n  \"target\": \"devin\",\n  \"installedAt\": {},\n  \"installMode\": \"global-only\",\n  \"runtime\": \"rust\",\n  \"sourceRoot\": {},\n  \"home\": {}\n}}\n",
        json_string(VERSION),
        now_millis() / 1000,
        json_string(&display_path(source_root)),
        json_string(&display_path(home))
    );
    fs::write(cli_dir.join("install-metadata.json"), payload)?;
    Ok(())
}

fn write_devin_hook_artifact(home: &Path, binary: &Path) -> Result<(), AppError> {
    let hook_path = home.join("wf-core").join("devin-hooks.v1.json");
    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&hook_path, devin_hooks_json(binary))?;
    Ok(())
}

fn update_devin_config(home: &Path, binary: &Path) -> Result<(), AppError> {
    fs::create_dir_all(home)?;
    let config_path = home.join("config.json");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    let mut updated = if existing.trim().is_empty() {
        "{\n  \"version\": 1\n}\n".to_string()
    } else {
        existing.clone()
    };
    if !updated.contains("\"read_config_from\"") {
        updated = insert_top_level_json_field(
            &updated,
            "read_config_from",
            "{ \"windsurf\": true, \"claude\": true, \"cursor\": true }",
        )?;
    }
    updated = ensure_devin_config_hooks(&updated, binary)?;
    if config_path.exists() && updated != existing {
        let backup = home
            .join("wf-core")
            .join(format!("config.backup.{}.json", now_millis() / 1000));
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(backup, existing)?;
    }
    fs::write(config_path, updated)?;
    Ok(())
}

fn insert_top_level_json_field(
    content: &str,
    field: &str,
    value_json: &str,
) -> Result<String, AppError> {
    let Some(index) = content.rfind('}') else {
        return Err(AppError::new("Devin config is not a JSON object"));
    };
    let before = content[..index].trim_end();
    let after = &content[index..];
    let needs_comma = !before.trim_end().ends_with('{');
    let mut output = String::new();
    output.push_str(before);
    if needs_comma {
        output.push_str(",\n");
    } else {
        output.push('\n');
    }
    output.push_str(&format!("  {}: {}\n", json_string(field), value_json));
    output.push_str(after);
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn ensure_devin_config_hooks(content: &str, binary: &Path) -> Result<String, AppError> {
    if content.contains("devin-hook pre-tool-use") {
        return Ok(content.to_string());
    }
    let Some((hooks_start, hooks_end)) = json_object_field_value_span(content, "hooks") else {
        return insert_top_level_json_field(content, "hooks", &devin_hooks_object_json(binary));
    };
    let hooks_value = &content[hooks_start..hooks_end];
    if !hooks_value.trim_start().starts_with('{') {
        return Err(AppError::new(
            "Devin config hooks field must be a JSON object",
        ));
    }
    let entry = devin_pre_tool_use_entry_json(binary);
    let updated_hooks = if let Some((pre_tool_start, pre_tool_end)) =
        json_object_field_value_span(hooks_value, "PreToolUse")
    {
        let pre_tool_value = &hooks_value[pre_tool_start..pre_tool_end];
        if !pre_tool_value.trim_start().starts_with('[') {
            return Err(AppError::new(
                "Devin config hooks.PreToolUse field must be a JSON array",
            ));
        }
        append_json_array_item(hooks_value, pre_tool_start, pre_tool_end, &entry)?
    } else {
        insert_top_level_json_field(
            hooks_value,
            "PreToolUse",
            &format!("[\n      {entry}\n    ]"),
        )?
    };
    let mut output = String::new();
    output.push_str(&content[..hooks_start]);
    output.push_str(&updated_hooks);
    output.push_str(&content[hooks_end..]);
    Ok(output)
}

fn append_json_array_item(
    content: &str,
    array_start: usize,
    array_end: usize,
    item_json: &str,
) -> Result<String, AppError> {
    let array_value = &content[array_start..array_end];
    let close_relative = array_value
        .rfind(']')
        .ok_or_else(|| AppError::new("JSON array is missing closing bracket"))?;
    let close_index = array_start + close_relative;
    let body = &content[array_start + 1..close_index];
    let insertion = if body.trim().is_empty() {
        format!("\n      {item_json}\n    ")
    } else {
        format!(",\n      {item_json}")
    };
    let mut output = String::new();
    output.push_str(&content[..close_index]);
    output.push_str(&insertion);
    output.push_str(&content[close_index..]);
    Ok(output)
}

fn json_object_field_value_span(content: &str, field: &str) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let mut index = 0;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if depth == 1 => {
                let (key, key_end) = parse_json_string_at(content, index)?;
                index = key_end;
                let mut probe = skip_json_whitespace(bytes, index);
                if key == field && bytes.get(probe) == Some(&b':') {
                    probe += 1;
                    let value_start = skip_json_whitespace(bytes, probe);
                    let value_end = json_value_end(content, value_start)?;
                    return Some((value_start, value_end));
                }
            }
            b'"' => {
                let (_, key_end) = parse_json_string_at(content, index)?;
                index = key_end;
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

fn parse_json_string_at(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut output = String::new();
    let mut index = start + 1;
    let mut escaped = false;
    while index < bytes.len() {
        let character = bytes[index] as char;
        if escaped {
            match character {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some((output, index + 1));
        } else {
            output.push(character);
        }
        index += 1;
    }
    None
}

fn skip_json_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .map(|byte| byte.is_ascii_whitespace())
        .unwrap_or(false)
    {
        index += 1;
    }
    index
}

fn json_value_end(content: &str, start: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    match bytes.get(start)? {
        b'"' => parse_json_string_at(content, start).map(|(_, end)| end),
        b'{' | b'[' => {
            let opener = bytes[start];
            let closer = if opener == b'{' { b'}' } else { b']' };
            let mut depth = 0usize;
            let mut index = start;
            while index < bytes.len() {
                match bytes[index] {
                    b'"' => {
                        let (_, end) = parse_json_string_at(content, index)?;
                        index = end;
                    }
                    byte if byte == opener => {
                        depth += 1;
                        index += 1;
                    }
                    byte if byte == closer => {
                        depth = depth.saturating_sub(1);
                        index += 1;
                        if depth == 0 {
                            return Some(index);
                        }
                    }
                    _ => index += 1,
                }
            }
            None
        }
        _ => {
            let mut index = start;
            while index < bytes.len() && !matches!(bytes[index], b',' | b'}' | b']') {
                index += 1;
            }
            Some(index)
        }
    }
}

fn devin_hooks_json(binary: &Path) -> String {
    format!("{}\n", devin_hooks_object_json(binary))
}

fn devin_hooks_object_json(binary: &Path) -> String {
    format!(
        "{{\n    \"PreToolUse\": [\n      {}\n    ]\n  }}",
        devin_pre_tool_use_entry_json(binary)
    )
}

fn devin_pre_tool_use_entry_json(binary: &Path) -> String {
    let command = format!(
        "{} devin-hook pre-tool-use",
        quote_arg(&display_path(binary))
    );
    format!(
        "{{\n        \"matcher\": \"exec\",\n        \"hooks\": [{{ \"type\": \"command\", \"command\": {}, \"timeout\": 5 }}]\n      }}",
        json_string(&command)
    )
}

fn append_text(path: &Path, content: &str) -> Result<(), AppError> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

fn count_installed_skills(path: &Path) -> usize {
    SKILL_NAMES
        .iter()
        .filter(|skill| path.join(skill).join("SKILL.md").exists())
        .count()
}

fn count_installed_agents(path: &Path) -> usize {
    SKILL_NAMES
        .iter()
        .filter(|skill| path.join(skill).join("AGENT.md").exists())
        .count()
}

fn count_installed_workflows(home: &Path) -> usize {
    WORKFLOW_NAMES
        .iter()
        .filter(|workflow| {
            home.join("windsurf")
                .join("workflows")
                .join(workflow)
                .exists()
        })
        .count()
}

fn count_files_with_extension(path: &Path, extension: &str) -> usize {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
        })
        .count()
}

fn remove_file_if_exists(path: &Path) -> Result<(), AppError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<(), AppError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), AppError> {
    if path.is_dir() {
        remove_dir_if_exists(path)
    } else {
        remove_file_if_exists(path)
    }
}

fn remove_known_stale_workflows(home: &Path) -> Result<(), AppError> {
    let workflows = home.join("windsurf").join("workflows");
    for workflow in OLD_MANAGED_WORKFLOW_NAMES {
        let path = workflows.join(workflow);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        if content.contains("wf-core")
            || content.contains("wf_core")
            || content.contains(MANAGED_START)
        {
            remove_file_if_exists(&path)?;
        }
    }
    Ok(())
}

fn remove_unlisted_wf_core_workflows(home: &Path) -> Result<(), AppError> {
    let current: BTreeSet<&str> = WORKFLOW_NAMES.iter().copied().collect();
    for path in managed_workflow_files(home)? {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !current.contains(name) {
            remove_file_if_exists(&path)?;
        }
    }
    Ok(())
}

fn managed_workflow_files(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let workflows = home.join("windsurf").join("workflows");
    if !workflows.exists() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    for entry in fs::read_dir(workflows)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("wf-core-") && name.ends_with(".md") {
            output.push(path);
        }
    }
    Ok(output)
}

fn flag_value(arguments: &[String], name: &str) -> Option<String> {
    for index in 0..arguments.len() {
        let argument = &arguments[index];
        if argument == name {
            return arguments.get(index + 1).cloned();
        }
        if let Some(value) = argument.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn arguments_before_separator(arguments: &[String]) -> &[String] {
    arguments
        .iter()
        .position(|argument| argument == "--")
        .map(|index| &arguments[..index])
        .unwrap_or(arguments)
}

fn has_flag(arguments: &[String], name: &str) -> bool {
    arguments
        .iter()
        .any(|argument| argument == name || argument == &format!("{name}=true"))
}

fn positional_after_options(arguments: &[String]) -> Vec<String> {
    if let Some(index) = arguments.iter().position(|argument| argument == "--") {
        return arguments[index + 1..].to_vec();
    }
    let mut output = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument.starts_with("--channel")
            || argument.starts_with("--max-lines")
            || argument.starts_with("--max-bytes")
        {
            if !argument.contains('=') {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if argument == "--shell" {
            index += 1;
            continue;
        }
        output.extend(arguments[index..].iter().cloned());
        break;
    }
    output
}

fn slugify(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars().take(96) {
        if character.is_ascii_alphanumeric()
            || character == '.'
            || character == '_'
            || character == '-'
        {
            output.push(character);
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "command".to_string()
    } else {
        trimmed
    }
}

fn workspace_key(path: &Path) -> String {
    let raw = path_to_manifest(&clean_path(path));
    let mut output = String::new();
    let mut previous_dash = false;
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed
    }
}

fn clean_path(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned.pop();
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    if cleaned.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        cleaned
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn json_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            other => output.push(other),
        }
    }
    output.push('"');
    output
}

fn json_string_field(content: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\"");
    let field_start = content.find(&pattern)?;
    let after_field = &content[field_start + pattern.len()..];
    let colon = after_field.find(':')?;
    let after_colon = after_field[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for character in after_colon[1..].chars() {
        if escaped {
            match character {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(output);
        } else {
            output.push(character);
        }
    }
    None
}

fn json_array_has_value(content: &str, field: &str) -> bool {
    let pattern = format!("\"{field}\"");
    let Some(field_start) = content.find(&pattern) else {
        return false;
    };
    let after_field = &content[field_start + pattern.len()..];
    let Some(colon) = after_field.find(':') else {
        return false;
    };
    let after_colon = after_field[colon + 1..].trim_start();
    let Some(rest) = after_colon.strip_prefix('[') else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    rest[..end]
        .split(',')
        .any(|item| !item.trim().trim_matches('"').is_empty())
}

fn json_number_field(line: &str, field: &str) -> Option<usize> {
    let pattern = format!("\"{field}\":");
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let digits: String = rest
        .chars()
        .skip_while(|character| character.is_whitespace())
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn json_bool_field(line: &str, field: &str) -> Option<bool> {
    let pattern = format!("\"{field}\":");
    let start = line.find(&pattern)? + pattern.len();
    let rest = line[start..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn savings_percent(raw_bytes: usize, saved_bytes: usize) -> f64 {
    if raw_bytes == 0 {
        0.0
    } else {
        (saved_bytes as f64 / raw_bytes as f64) * 100.0
    }
}

fn quote_arg(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        let escaped = value.replace('\'', "'\\''");
        format!("'{escaped}'")
    }
}

fn clamp_exit_code(value: i32) -> i32 {
    value.clamp(0, 255)
}

fn print_instructions() {
    println!(
        "# wf-core Agent Instructions

Use these rules in Windsurf or Windsurf Next sessions:

1. Token saving means context-output tokens, not authentication tokens.
2. Never store, print, or collect user auth tokens, API keys, cookies, or secrets.
3. Before noisy terminal commands, prefer `wf-core run -- <command>`.
4. If a command has shell syntax, use `wf-core run --shell -- \"<command>\"`.
5. Use `wf-core rewrite \"<command>\"` when unsure whether a wrapper is useful.
6. Use managed global Windsurf skills from `~/.codeium/<channel>/skills/`.
7. Before editing existing behavior, use the preserve-existing-flow skill and record owner-path evidence.
8. Finish by reconciling every explicit user requirement against evidence."
    );
}

fn print_hook_instructions() {
    println!(
        "# wf-core Hook-Equivalent Instructions

Windsurf and Windsurf Next use global rules, workflows, and skills for agent behavior. Treat the installed wf-core global rules as the pre-terminal hook policy.

1. Before noisy test, build, lint, status, log, broad search, package-manager, Docker, Kubernetes, Terraform, or CI-style commands, run `wf-core run -- <command>`.
2. For shell syntax such as pipes, redirects, `&&`, `||`, command substitution, or semicolons, run `wf-core run --shell -- \"<command>\"`.
3. Use `wf-core rewrite \"<command>\"` when unsure whether a command should be wrapped.
4. Continue from the compacted output and preserve the command exit code.
5. Never capture, store, or display authentication tokens, API keys, cookies, session secrets, or credentials.
6. Keep the same policy installed in both `~/.codeium/windsurf/` and `~/.codeium/windsurf-next/`."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_noisy_commands() {
        assert!(is_supported_noisy_command("cargo test --workspace").0);
        assert!(is_supported_noisy_command("git status --short").0);
        assert!(!is_supported_noisy_command("echo hello").0);
    }

    #[test]
    fn compact_output_keeps_high_signal() {
        let stdout = (0..160)
            .map(|index| format!("line {index}"))
            .chain(std::iter::once("ERROR: important failure".to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        let compaction = compact_output(
            "pytest -q",
            1,
            &stdout,
            "",
            PathBuf::from("raw.log"),
            20,
            1000,
            10,
        );
        assert!(compaction.compacted);
        assert!(compaction.rendered.contains("ERROR: important failure"));
        assert!(compaction.rendered.contains("raw.log"));
    }

    #[test]
    fn managed_block_replaces_existing_content() {
        let first = replace_or_append_managed_block("hello", "BLOCK");
        assert_eq!(first, "hello\n\nBLOCK");
        let second = replace_or_append_managed_block(
            "a\n<!-- wf-core managed:start -->\nold\n<!-- wf-core managed:end -->\nb",
            "new\n",
        );
        assert_eq!(second, "a\nnew\nb");
    }

    #[test]
    fn flow_validation_requires_owner_path_unless_exempt() {
        let incomplete = render_flow_template("task", "src/main.rs", "run");
        let findings = validate_flow_content(&incomplete);
        assert!(findings
            .iter()
            .any(|finding| finding.contains("current_behavior_to_preserve")));

        let exempt = incomplete.replace("\"greenfield\": false", "\"greenfield\": true");
        assert!(validate_flow_content(&exempt).is_empty());
    }

    #[test]
    fn file_checksum_changes_with_content() {
        let root = env::temp_dir().join(format!("wf-core-test-{}", now_millis()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("example.txt");
        fs::write(&path, "one").unwrap();
        let first = file_checksum(&path).unwrap();
        fs::write(&path, "two").unwrap();
        let second = file_checksum(&path).unwrap();
        assert_ne!(first, second);
        remove_dir_if_exists(&root).unwrap();
    }

    #[test]
    fn lint_message_accepts_generated_commit_template() {
        let message =
            "feat: extend wf-core native workflow gates\n\nTest Result:\n- cargo test passed\n";
        assert!(lint_message_text(message).is_empty());
    }

    #[test]
    fn json_field_insertion_preserves_existing_devin_config() {
        let updated = insert_top_level_json_field(
            "{\n  \"version\": 1\n}\n",
            "read_config_from",
            "{ \"windsurf\": true }",
        )
        .unwrap();
        assert!(updated.contains("\"version\": 1,"));
        assert!(updated.contains("\"read_config_from\": { \"windsurf\": true }"));
    }

    #[test]
    fn run_options_ignore_child_command_flags() {
        let args = vec![
            "--".to_string(),
            "wf-core".to_string(),
            "verify".to_string(),
            "--channel".to_string(),
            "both".to_string(),
        ];
        assert!(flag_value(arguments_before_separator(&args), "--channel").is_none());
    }

    #[test]
    fn devin_hook_merge_preserves_existing_hooks() {
        let binary = PathBuf::from("C:/Users/example/AppData/Roaming/devin/wf-core/wf-core.exe");
        let existing = "{\n  \"version\": 1,\n  \"hooks\": {\n    \"PreToolUse\": [\n      { \"matcher\": \"read\", \"hooks\": [] }\n    ],\n    \"Stop\": []\n  }\n}\n";
        let updated = ensure_devin_config_hooks(existing, &binary).unwrap();
        assert!(updated.contains("\"matcher\": \"read\""));
        assert!(updated.contains("\"Stop\": []"));
        assert!(updated.contains("devin-hook pre-tool-use"));
    }

    #[test]
    fn devin_agent_profile_uses_openai_agent_prompt() {
        let root = env::temp_dir().join(format!("wf-core-agent-test-{}", now_millis()));
        let skill_dir = root.join("skills").join("reviewer");
        fs::create_dir_all(skill_dir.join("agents")).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: reviewer\ndescription: Review fallback\n---\nFallback body\n",
        )
        .unwrap();
        fs::write(
            skill_dir.join("agents").join("openai.yaml"),
            "interface:\n  short_description: \"Review profile\"\n  default_prompt: \"Review with evidence.\"\n",
        )
        .unwrap();
        let profile = load_devin_agent_profile(&skill_dir).unwrap();
        let rendered = render_devin_agent("reviewer", &profile);
        assert!(rendered.contains("description: \"Review profile\""));
        assert!(rendered.contains("Review with evidence."));
        remove_dir_if_exists(&root).unwrap();
    }

    #[test]
    fn system_map_contains_inventory_fingerprint() {
        let root = env::temp_dir().join(format!("wf-core-map-test-{}", now_millis()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname=\"example\"\n").unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn command_example() {}\n",
        )
        .unwrap();
        let map = render_system_map(&root).unwrap();
        assert!(map.contains("inventory_fingerprint"));
        assert!(map.contains("command_example"));
        remove_dir_if_exists(&root).unwrap();
    }
}
