# Memory And System Map

`wf_core` keeps agent memory in a shared data home so Windsurf, Windsurf Next,
and Devin-local sessions can use the same evidence.

## Paths

| Surface | Windows | macOS/Linux |
| --- | --- | --- |
| Shared memory root | `%APPDATA%\wf-core\memories\` | `~/.local/share/wf-core/memories/` |
| Workspace memory | `<root>\workspaces\<workspace-slug>\` | `<root>/workspaces/<workspace-slug>/` |
| System map | `<workspace>\SYSTEM_MAP.md` | `<workspace>/SYSTEM_MAP.md` |

Set `WF_CORE_HOME` to override the shared data home for tests or isolated
sessions.

## Commands

```bash
wf-core memory status --repo-root .
wf-core memory remember --repo-root . --kind notes --key decision --text "Use native Rust only."
wf-core memory recall --repo-root . --kind notes
wf-core memory system-map refresh --repo-root .
wf-core memory system-map show --repo-root .
wf-core memory system-map verify --repo-root .
```

`system-map refresh` records top-level areas, key files, Rust command handlers,
managed skills, validation commands, and an inventory fingerprint. `system-map
verify` recomputes the fingerprint and exits non-zero when the map is stale.
