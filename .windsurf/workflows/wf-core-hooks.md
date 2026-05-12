---
auto_execution_mode: 1
description: Transparently auto-rewrite noisy terminal commands through wf-core proxy compaction
---

Transparent auto-rewrite workflow — fires automatically without model interaction.

1. Before any terminal command, check if wf-core shim mode is active (`wf-core shim doctor --channel next`).
2. If shim mode is active, run the command normally — the shim intercepts it transparently.
3. If shim mode is not active, wrap noisy commands with `wf-core run -- <command>`.
4. For shell syntax (pipes, redirects, `&&`, `||`), use `wf-core run --shell -- "<command>"`.
5. Always preserve the exit code and continue from the compacted output.
6. If full output is needed, use `wf-core raw <raw_id>` from the compact output line.
7. Never retry a blocked command — the proxy result is authoritative.
8. Never print or store credentials. Compact output redacts likely secrets.
