---
description: "Apply wf-core hook-equivalent terminal policy before noisy commands"
trigger: always_on
---

# Terminal Hook Policy

Windsurf sessions must treat this as the pre-terminal hook policy:

- Before noisy test, build, lint, status, log, broad search, package-manager, Docker, Kubernetes, Terraform, or CI-style commands, run `wf-core run -- <command>` instead of the raw command.
- For shell syntax such as pipes, redirects, `&&`, `||`, command substitution, or semicolons, run `wf-core run --shell -- "<command>"`.
- Use `wf-core hook instructions` or `wf-core rewrite "<command>"` when unsure.
- Continue from the compacted output and preserve the command exit code.
- Never capture, store, or display authentication tokens, API keys, cookies, session secrets, or credentials.
