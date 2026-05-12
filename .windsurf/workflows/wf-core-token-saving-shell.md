---
auto_execution_mode: 0
description: Run noisy terminal commands through wf-core proxy/shim compaction
---

Use this workflow before running noisy commands in Windsurf.

1. Run `wf-core shim doctor --channel next` or `wf-core doctor --proxy --channel next`.
2. If the shim directory is active in `PATH`, run the noisy command normally and let the shim call `wf-core dispatch`.
3. If shim mode is not active, use explicit fallback: `wf-core run -- <command>`.
4. If explicit fallback is needed for pipes, redirects, `&&`, `||`, command substitution, or other shell syntax, use `wf-core run --shell -- "<command>"`.
5. Continue from the compacted output and preserve the exit code.
6. If full output is needed, use the displayed `raw: wf-core raw <raw_id>` recovery command.
7. After terminal-heavy work, run `wf-core gain --since today --channel next`.
8. Never store authentication tokens or secrets. Raw output stays local and may contain what the command printed.

Windsurf rules/workflows guide behavior; real automatic saving comes from the
wf-core-managed shim/proxy layer.
