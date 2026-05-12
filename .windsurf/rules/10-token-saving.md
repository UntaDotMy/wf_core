---
description: "Save Windsurf context tokens before noisy terminal output is produced"
trigger: always_on
---

# Token-Saving Command Rule

- Token saving means reducing command-output context, not storing auth tokens.
- Never store or reveal auth tokens, API keys, cookies, session secrets, or credentials.
- Real automatic saving requires wf-core shim/PATH proxy mode; rules alone do not intercept terminal output.
- Before noisy terminal work, run `wf-core doctor --proxy --channel next` or `wf-core shim doctor --channel next` to verify proxy readiness.
- Run test, build, lint, log, status, search, Docker, Kubernetes, Terraform, package-manager, or CI-style commands through the wf-core proxy. If shim mode is active, call the command normally. If shim mode is not active, use `wf-core run -- <command>`.
- If `wf-core` is not on PATH, use the installed binary from the active agent
  home, such as `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe` or
  `%APPDATA%\devin\wf-core\wf-core.exe` on Windows.
- If shim mode is not active and a command has shell syntax, use `wf-core run --shell -- "<command>"`.
- Use `wf-core rewrite "<command>"` to check whether a wrapper is recommended.
- The proxy preserves the exit code, saves raw output under `~/.codeium/<channel>/wf-core/raw-output/`, and reports estimated token savings with `wf-core gain`.
