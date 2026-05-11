# Devin Local Support

`wf_core` installs a native Devin for Terminal surface in addition to Windsurf
and Windsurf Next.

## Installed Global Files

| Surface | Windows | macOS/Linux |
| --- | --- | --- |
| Skills | `%APPDATA%\devin\skills\<name>\SKILL.md` | `~/.config/devin/skills/<name>/SKILL.md` |
| Custom agents | `%APPDATA%\devin\agents\<name>\AGENT.md` | `~/.config/devin/agents/<name>/AGENT.md` |
| Config | `%APPDATA%\devin\config.json` | `~/.config/devin/config.json` |
| Hook artifact | `%APPDATA%\devin\wf-core\devin-hooks.v1.json` | `~/.config/devin/wf-core/devin-hooks.v1.json` |
| Binary | `%APPDATA%\devin\wf-core\wf-core.exe` | `~/.config/devin/wf-core/wf-core` |

The managed config enables `read_config_from.windsurf`, installs matching
custom agents from each skill's agent metadata, and installs a `PreToolUse` hook
for `exec`. The hook blocks noisy raw commands before output enters Devin
context and returns a `Rerun that as: wf-core run -- ...` reason.

## Commands

```bash
wf-core install --target devin --source-root <repo>
wf-core verify --target devin
wf-core hook list --target devin
```

Use `--target all --channel both` to install or verify Windsurf stable,
Windsurf Next, and Devin together.
