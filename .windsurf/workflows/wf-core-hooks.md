---
auto_execution_mode: 0
description: Apply wf-core terminal hook discipline in Windsurf and Windsurf Next
---

Use this workflow as the Windsurf hook-equivalent before terminal-heavy work.

1. Identify whether the next terminal command is likely to be noisy.
2. If it is noisy, run `wf-core run -- <command>` before raw output is produced.
3. If it uses shell syntax, run `wf-core run --shell -- "<command>"`.
4. Preserve the wrapped command's exit code and continue from the compacted output.
5. Never print or store authentication tokens, API keys, cookies, or secrets.

Windsurf stable and Windsurf Next are the same product family with separate global homes. Keep this workflow installed in both channel folders.
