---
auto_execution_mode: 0
description: Verify Rust-native global wf-core installation for Windsurf stable and Windsurf Next
---

Run the global installation check:

```bash
wf-core doctor --channel both
```

If the binary is not on PATH, use the installed global binary:

- Windsurf Next Windows: `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe doctor --channel both`
- Windsurf stable Windows: `%USERPROFILE%\.codeium\windsurf\wf-core\wf-core.exe doctor --channel both`
- macOS/Linux: `~/.codeium/windsurf-next/wf-core/wf-core doctor --channel both`
