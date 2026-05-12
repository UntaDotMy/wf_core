---
auto_execution_mode: 1
description: Transparently proxy noisy terminal commands through wf-core for token savings
---

Auto-execution workflow for token saving. Fires when any terminal command is detected.

1. Run `wf-core shim doctor --channel next` to check shim readiness.
2. If the shim directory is active in `PATH`, run the noisy command normally — the shim calls `wf-core dispatch` transparently.
3. If shim mode is not active, use `wf-core run -- <command>`.
4. For pipes, redirects, `&&`, `||`, command substitution, or other shell syntax, use `wf-core run --shell -- "<command>"`.
5. Continue from the compacted output and preserve the exit code.
6. If full output is needed, use the displayed `raw: wf-core raw <raw_id>` recovery command.
7. After terminal-heavy work, run `wf-core gain --since today --channel next`.
8. Never store authentication tokens or secrets. Raw output stays local and may contain what the command printed.
