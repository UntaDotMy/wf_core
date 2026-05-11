---
description: "Save Windsurf context tokens before noisy terminal output is produced"
trigger: always_on
---

# Token-Saving Command Rule

- Token saving means reducing command-output context, not storing auth tokens.
- Never store or reveal auth tokens, API keys, cookies, session secrets, or credentials.
- Before test, build, lint, log, status, search, Docker, Kubernetes, Terraform, package-manager, or CI-style commands, use the Rust-native `wf-core run -- <command>`.
- If `wf-core` is not on PATH, use the installed binary from the active agent
  home, such as `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe` or
  `%APPDATA%\devin\wf-core\wf-core.exe` on Windows.
- If a command has shell syntax, use `wf-core run --shell -- "<command>"`.
- Use `wf-core rewrite "<command>"` to check whether a wrapper is recommended.
- The wrapper preserves the exit code and saves raw output under `~/.codeium/<channel>/wf-core/raw-output/`.
