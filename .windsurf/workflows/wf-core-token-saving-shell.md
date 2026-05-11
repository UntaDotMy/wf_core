---
auto_execution_mode: 0
description: Run noisy terminal commands through wf-core compaction
---

Use this workflow before running noisy commands in Windsurf.

- Prefer the Rust-native `wf-core run -- <command>` for tests, builds, lints, logs, broad searches, package managers, Docker, Kubernetes, Terraform, and CI-style commands.
- Use `wf-core run --shell -- "<command>"` for pipes, redirects, `&&`, `||`, command substitution, or other shell syntax.
- Use `wf-core rewrite "<command>"` to inspect the recommended wrapper.
- Continue from the compacted output and preserve the exit code.
- Never store authentication tokens or secrets.
