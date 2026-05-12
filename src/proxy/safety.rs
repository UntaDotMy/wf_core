/// Patterns indicating an interactive program that must never be wrapped in
/// the proxy pipeline (which captures output and prevents user interaction).
const INTERACTIVE_PROGRAMS: &[&str] = &[
    "vim", "vi", "nano", "emacs", "less", "more", "top", "htop", "btop", "ssh", "sftp", "scp",
    "mysql", "psql", "sqlite3", "python", "python3", "node", "irb", "pry", "ruby",
];

/// Subcommand-level interactive guards: `(program, subcommand fragment)`.
const INTERACTIVE_PATTERNS: &[(&str, &str)] = &[
    ("cargo", "watch"),
    ("npm", "run dev"),
    ("yarn", "dev"),
    ("pnpm", "dev"),
    ("docker", "exec -it"),
    ("docker", "run -it"),
    ("kubectl", "exec -it"),
];

/// True if running this command through the proxy would break interactive UX.
pub fn is_interactive_command(program: &str, args: &[String]) -> bool {
    let lower = program.to_ascii_lowercase();
    if INTERACTIVE_PROGRAMS.contains(&lower.as_str()) && args.is_empty() {
        return true;
    }
    if lower == "python" || lower == "python3" {
        let has_real_args = args
            .iter()
            .any(|a| !a.starts_with('-') || a == "-c" || a == "-m");
        if !has_real_args {
            return true;
        }
    }
    if lower == "node" {
        let has_real_args = args.iter().any(|a| !a.starts_with('-'));
        if !has_real_args {
            return true;
        }
    }
    let joined = format!("{} {}", lower, args.join(" "));
    for (p, fragment) in INTERACTIVE_PATTERNS {
        if lower == *p && joined.contains(fragment) {
            return true;
        }
    }
    // Docker/kubectl `-i`/`-t` style invocations.
    if (lower == "docker" || lower == "kubectl")
        && args.iter().any(|a| a == "-it" || a == "--tty" || a == "-t")
    {
        return true;
    }
    false
}

/// Commands that are destructive enough that replay must require an opt-in.
const DESTRUCTIVE_PROGRAMS: &[&str] = &["rm", "del", "rmdir", "shred", "mkfs", "dd"];

const DESTRUCTIVE_PATTERNS: &[(&str, &str)] = &[
    ("git", "reset --hard"),
    ("git", "clean -fd"),
    ("git", "push --force"),
    ("docker", "rm"),
    ("docker", "system prune"),
    ("kubectl", "delete"),
    ("terraform", "apply"),
    ("terraform", "destroy"),
    ("npm", "publish"),
    ("yarn", "publish"),
    ("pnpm", "publish"),
    ("cargo", "publish"),
];

/// True if this command would mutate state in a way that should not be replayed
/// without explicit `--allow-risky`.
pub fn is_destructive(program: &str, args: &[String]) -> bool {
    let lower = program.to_ascii_lowercase();
    if DESTRUCTIVE_PROGRAMS.contains(&lower.as_str()) {
        return true;
    }
    let joined = format!("{} {}", lower, args.join(" "));
    for (p, fragment) in DESTRUCTIVE_PATTERNS {
        if lower == *p && joined.contains(fragment) {
            return true;
        }
    }
    // `mv -f` over potentially existing path counts as destructive.
    if lower == "mv" && args.iter().any(|a| a == "-f" || a == "--force") {
        return true;
    }
    false
}

/// Patterns that suggest a likely credential and should be redacted in compact output.
/// Each entry is a case-insensitive substring marker.
pub const SECRET_MARKERS: &[&str] = &[
    "api_key=",
    "secret=",
    "token=",
    "password=",
    "passwd=",
    "private key",
    "begin rsa private key",
    "begin ed25519 private key",
    "begin openssh private key",
    "aws_secret_access_key",
    "aws_session_token",
    "github_token",
    "npm_token",
    "authorization:",
    "bearer ey",
];

/// Heuristic: a long string of base64-looking characters likely contains an entropy-bearing secret.
pub fn looks_like_high_entropy(line: &str) -> bool {
    let mut run = 0usize;
    let mut best = 0usize;
    for ch in line.chars() {
        if ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '_' || ch == '-' {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best >= 32
}

/// Replace likely-secret lines with a placeholder. Returns the redacted text
/// and the number of lines redacted.
pub fn redact_secrets(text: &str) -> (String, usize) {
    let mut output = String::new();
    let mut redacted = 0usize;
    for line in text.lines() {
        if line_looks_like_secret(line) {
            output.push_str("[redacted possible secret; see local raw output if necessary]\n");
            redacted += 1;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    (output, redacted)
}

fn line_looks_like_secret(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    for marker in SECRET_MARKERS {
        if lower.contains(marker) {
            return true;
        }
    }
    if line.contains("===") || line.contains("---") {
        return false;
    }
    // Long base64-ish runs combined with key-looking words.
    if (lower.contains("token") || lower.contains("key") || lower.contains("secret"))
        && looks_like_high_entropy(line)
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_python_without_args_is_blocked() {
        assert!(is_interactive_command("python", &[]));
        assert!(is_interactive_command("vim", &[]));
        assert!(!is_interactive_command(
            "python",
            &["-m".into(), "pytest".into()]
        ));
    }

    #[test]
    fn npm_dev_is_interactive() {
        assert!(is_interactive_command("npm", &["run".into(), "dev".into()]));
    }

    #[test]
    fn destructive_rm_is_flagged() {
        assert!(is_destructive("rm", &["-rf".into(), "/tmp/x".into()]));
        assert!(is_destructive(
            "git",
            &["reset".into(), "--hard".into(), "HEAD".into()]
        ));
    }

    #[test]
    fn redaction_replaces_marker_lines() {
        let text = "GITHUB_TOKEN=ghp_abcdefgh\nok line\nAuthorization: Bearer eyJabc.123\n";
        let (out, count) = redact_secrets(text);
        assert_eq!(count, 2);
        assert!(out.contains("ok line"));
        assert!(out.contains("[redacted possible secret"));
    }
}
