---
auto_execution_mode: 0
description: Apply wf-core terminal hook discipline in Windsurf and Windsurf Next
---

Use this workflow as the Windsurf hook-equivalent before terminal-heavy work.

1. Identify whether the next terminal command is likely to be noisy.
2. Run `wf-core doctor --proxy --channel next` or `wf-core shim doctor --channel next` before terminal-heavy work.
3. If shim mode is active, run the command normally and let the managed shim intercept it.
4. If shim mode is not active, run `wf-core run -- <command>` before raw output is produced.
5. If explicit fallback uses shell syntax, run `wf-core run --shell -- "<command>"`.
6. Preserve the wrapped command's exit code and continue from the compacted output.
7. Use `wf-core raw <raw_id>` if full local output is needed.
8. Never print or store authentication tokens, API keys, cookies, or secrets.

Windsurf stable and Windsurf Next are the same product family with separate global homes. Keep this workflow installed in both channel folders.
