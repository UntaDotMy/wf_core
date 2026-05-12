use std::path::{Path, PathBuf};

/// Logical classification of a command. Used by adapters and the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Test,
    Git,
    Search,
    FileRead,
    FileList,
    Build,
    Lint,
    Logs,
    PackageManager,
    Infra,
    Unknown,
}

impl CommandKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CommandKind::Test => "Test",
            CommandKind::Git => "Git",
            CommandKind::Search => "Search",
            CommandKind::FileRead => "FileRead",
            CommandKind::FileList => "FileList",
            CommandKind::Build => "Build",
            CommandKind::Lint => "Lint",
            CommandKind::Logs => "Logs",
            CommandKind::PackageManager => "PackageManager",
            CommandKind::Infra => "Infra",
            CommandKind::Unknown => "Unknown",
        }
    }
}

/// Parsed representation of an incoming command.
#[derive(Debug, Clone)]
pub struct CommandAst {
    pub original_command: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub shell_mode: bool,
    pub shell_wrapped: bool,
    pub detected_kind: CommandKind,
    pub has_shell_syntax: bool,
    pub invoked_as_shim: Option<String>,
}

const SHELL_MARKERS: &[&str] = &["|", "&&", "||", ";", ">", "<", "$(", "`"];

const SHELL_WRAPPERS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "cmd",
    "powershell",
    "pwsh",
];

const ENV_WRAPPERS: &[&str] = &[
    "env", "time", "nice", "ionice", "stdbuf", "unbuffer", "sudo", "doas",
];

/// Strip a leading suffix like ".exe"/".cmd"/".ps1" from a program name for matching.
pub fn normalize_program(value: &str) -> String {
    let base = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_ascii_lowercase();
    base.trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .trim_end_matches(".ps1")
        .to_string()
}

pub fn has_shell_syntax(command_text: &str) -> bool {
    SHELL_MARKERS
        .iter()
        .any(|marker| command_text.contains(marker))
}

fn classify_from(program: &str, args: &[String], has_shell: bool) -> CommandKind {
    let normalized = normalize_program(program);
    classify_normalized(&normalized, args, has_shell)
}

fn classify_normalized(program: &str, args: &[String], has_shell: bool) -> CommandKind {
    if has_shell {
        return CommandKind::Unknown;
    }
    let first_arg = args.first().map(|s| s.as_str()).unwrap_or("");
    match program {
        // Tests
        "pytest" => CommandKind::Test,
        "jest" | "vitest" | "mocha" | "playwright" | "ava" | "cypress" => CommandKind::Test,
        "cargo" => match first_arg {
            "test" | "nextest" => CommandKind::Test,
            "build" | "check" | "run" => CommandKind::Build,
            "clippy" => CommandKind::Lint,
            "fmt" => CommandKind::Lint,
            _ => CommandKind::PackageManager,
        },
        "go" => match first_arg {
            "test" => CommandKind::Test,
            "build" | "install" | "run" => CommandKind::Build,
            "vet" => CommandKind::Lint,
            _ => CommandKind::Unknown,
        },
        "python" | "python3" => {
            if args.iter().any(|a| a == "-m") && args.iter().any(|a| a == "pytest") {
                CommandKind::Test
            } else {
                CommandKind::Unknown
            }
        }
        "npm" | "pnpm" | "yarn" | "bun" => match first_arg {
            "test" => CommandKind::Test,
            "run" => {
                let target = args.get(1).map(|s| s.as_str()).unwrap_or("");
                match target {
                    "test" | "tests" | "test:unit" | "test:integration" | "jest" | "vitest" => {
                        CommandKind::Test
                    }
                    "build" | "compile" | "tsc" => CommandKind::Build,
                    "lint" | "eslint" | "biome" | "ruff" | "fmt" => CommandKind::Lint,
                    _ => CommandKind::PackageManager,
                }
            }
            "build" => CommandKind::Build,
            "lint" => CommandKind::Lint,
            "install" | "ci" | "update" | "add" | "remove" => CommandKind::PackageManager,
            _ => CommandKind::PackageManager,
        },
        "npx" => {
            let target = args.first().map(|s| s.as_str()).unwrap_or("");
            match target {
                "jest" | "vitest" | "mocha" | "playwright" => CommandKind::Test,
                "tsc" => CommandKind::Build,
                "eslint" | "prettier" | "biome" => CommandKind::Lint,
                _ => CommandKind::PackageManager,
            }
        }
        // Git/version control
        "git" => CommandKind::Git,
        "gh" => CommandKind::Git,
        // Search
        "rg" | "grep" | "ag" | "ack" | "ripgrep" => CommandKind::Search,
        // Files
        "cat" | "head" | "tail" | "sed" => CommandKind::FileRead,
        "ls" | "find" | "tree" | "dir" => CommandKind::FileList,
        // Package managers
        "pip" | "pip3" => CommandKind::PackageManager,
        // HTTP clients
        "curl" | "wget" => CommandKind::FileRead,
        // Build/lint
        "tsc" => CommandKind::Build,
        "eslint" | "biome" | "prettier" | "ruff" | "mypy" | "pyright" => CommandKind::Lint,
        "make" => CommandKind::Build,
        "mvn" | "gradle" | "gradlew" | "dotnet" | "javac" => CommandKind::Build,
        // Logs/infra
        "docker" => match first_arg {
            "logs" | "compose" => CommandKind::Logs,
            "build" => CommandKind::Build,
            _ => CommandKind::Logs,
        },
        "docker-compose" => CommandKind::Logs,
        "kubectl" => match first_arg {
            "logs" | "describe" | "get" | "events" | "top" => CommandKind::Logs,
            _ => CommandKind::Infra,
        },
        "journalctl" => CommandKind::Logs,
        "helm" => CommandKind::Infra,
        "terraform" => CommandKind::Infra,
        _ => CommandKind::Unknown,
    }
}

/// Build an AST from a vector of command tokens.
pub fn build_ast(
    command_args: &[String],
    cwd: PathBuf,
    shell_mode: bool,
    invoked_as_shim: Option<String>,
) -> CommandAst {
    let original_command = command_args.join(" ");
    let has_shell = shell_mode || has_shell_syntax(&original_command);

    let mut program = command_args.first().cloned().unwrap_or_default();
    let mut args: Vec<String> = command_args.iter().skip(1).cloned().collect();

    // Unwrap leading shell wrappers like `bash -lc "real command"` so classification
    // can see the real intent. This is best-effort and only handles -c forms.
    let mut shell_wrapped = false;
    if !shell_mode {
        let norm = normalize_program(&program);
        if SHELL_WRAPPERS.contains(&norm.as_str()) {
            if let Some(idx) = args
                .iter()
                .position(|a| a == "-c" || a == "-lc" || a == "/C" || a == "-Command")
            {
                if let Some(inner) = args.get(idx + 1) {
                    let inner = inner.clone();
                    if !has_shell_syntax(&inner) {
                        let tokens: Vec<String> =
                            inner.split_whitespace().map(|s| s.to_string()).collect();
                        if let Some(first) = tokens.first() {
                            program = first.clone();
                            args = tokens.iter().skip(1).cloned().collect();
                            shell_wrapped = true;
                        }
                    }
                }
            }
        }
    }

    // Unwrap leading env/time/sudo wrappers.
    while let Some(norm) = (!shell_mode).then(|| normalize_program(&program)) {
        if !ENV_WRAPPERS.contains(&norm.as_str()) {
            break;
        }
        // Skip flags and KEY=VALUE assignments after env-like prefix.
        let mut idx = 0usize;
        while let Some(token) = args.get(idx) {
            if token == "--" {
                idx += 1;
                break;
            }
            if (norm == "env" || norm == "sudo" || norm == "doas") && token.contains('=') {
                idx += 1;
                continue;
            }
            if token.starts_with('-') {
                idx += 1;
                continue;
            }
            break;
        }
        if let Some(next) = args.get(idx).cloned() {
            program = next;
            args = args.iter().skip(idx + 1).cloned().collect();
            continue;
        }
        break;
    }

    let kind = classify_from(&program, &args, has_shell);

    CommandAst {
        original_command,
        program,
        args,
        cwd,
        shell_mode,
        shell_wrapped,
        detected_kind: kind,
        has_shell_syntax: has_shell,
        invoked_as_shim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ast(tokens: &[&str]) -> CommandAst {
        let args: Vec<String> = tokens.iter().map(|s| s.to_string()).collect();
        build_ast(&args, PathBuf::from("/tmp"), false, None)
    }

    #[test]
    fn cargo_test_classifies_as_test() {
        let a = ast(&["cargo", "test", "--workspace"]);
        assert_eq!(a.detected_kind, CommandKind::Test);
        assert_eq!(a.program, "cargo");
    }

    #[test]
    fn pytest_classifies_as_test() {
        let a = ast(&["pytest", "tests", "-q"]);
        assert_eq!(a.detected_kind, CommandKind::Test);
    }

    #[test]
    fn git_diff_classifies_as_git() {
        let a = ast(&["git", "diff", "--cached"]);
        assert_eq!(a.detected_kind, CommandKind::Git);
    }

    #[test]
    fn rg_classifies_as_search() {
        let a = ast(&["rg", "foo", "."]);
        assert_eq!(a.detected_kind, CommandKind::Search);
    }

    #[test]
    fn cat_classifies_as_file_read() {
        let a = ast(&["cat", "src/main.rs"]);
        assert_eq!(a.detected_kind, CommandKind::FileRead);
    }

    #[test]
    fn ls_classifies_as_file_list() {
        let a = ast(&["ls", "-R"]);
        assert_eq!(a.detected_kind, CommandKind::FileList);
    }

    #[test]
    fn docker_logs_classifies_as_logs() {
        let a = ast(&["docker", "logs", "api"]);
        assert_eq!(a.detected_kind, CommandKind::Logs);
    }

    #[test]
    fn kubectl_logs_classifies_as_logs() {
        let a = ast(&["kubectl", "logs", "deploy/api"]);
        assert_eq!(a.detected_kind, CommandKind::Logs);
    }

    #[test]
    fn terraform_plan_classifies_as_infra() {
        let a = ast(&["terraform", "plan"]);
        assert_eq!(a.detected_kind, CommandKind::Infra);
    }

    #[test]
    fn tsc_classifies_as_build() {
        let a = ast(&["tsc", "--noEmit"]);
        assert_eq!(a.detected_kind, CommandKind::Build);
    }

    #[test]
    fn eslint_classifies_as_lint() {
        let a = ast(&["eslint", "."]);
        assert_eq!(a.detected_kind, CommandKind::Lint);
    }

    #[test]
    fn cargo_build_classifies_as_build() {
        let a = ast(&["cargo", "build", "--release"]);
        assert_eq!(a.detected_kind, CommandKind::Build);
    }

    #[test]
    fn cargo_clippy_classifies_as_lint() {
        let a = ast(&["cargo", "clippy"]);
        assert_eq!(a.detected_kind, CommandKind::Lint);
    }

    #[test]
    fn env_prefix_is_unwrapped() {
        let a = ast(&["env", "RUST_BACKTRACE=1", "cargo", "test"]);
        assert_eq!(a.program, "cargo");
        assert_eq!(a.detected_kind, CommandKind::Test);
    }

    #[test]
    fn bash_lc_is_unwrapped_when_no_shell_syntax() {
        let a = ast(&["bash", "-lc", "pytest tests"]);
        assert!(a.shell_wrapped);
        assert_eq!(a.program, "pytest");
        assert_eq!(a.detected_kind, CommandKind::Test);
    }

    #[test]
    fn shell_pipe_marks_shell_syntax() {
        let a = ast(&["sh", "-c", "npm test 2>&1 | tee out.log"]);
        assert!(a.has_shell_syntax);
    }

    #[test]
    fn time_prefix_is_unwrapped() {
        let a = ast(&["time", "go", "test", "./..."]);
        assert_eq!(a.program, "go");
        assert_eq!(a.detected_kind, CommandKind::Test);
    }
}
