# Hook-Equivalent Terminal Policy

Windsurf behavior is configured through global rules, workflows, and skills.
Devin for Terminal has native lifecycle hooks. `wf_core` installs the same
terminal policy into Windsurf stable, Windsurf Next, and Devin global homes.

## Installed Pieces

- `~/.codeium/<channel>/memories/global_rules.md` contains the always-on terminal policy.
- `~/.codeium/<channel>/windsurf/workflows/wf-core-hooks.md` gives the manual hook workflow.
- `~/.codeium/<channel>/windsurf/workflows/wf-core-token-saving-shell.md` gives the command wrapper workflow.
- `~/.codeium/<channel>/wf-core/wf-core(.exe)` provides `hook`, `rewrite`, `run`, and `gain` commands.
- `%APPDATA%\devin\config.json` or `~/.config/devin/config.json` contains the Devin `PreToolUse` hook.
- `%APPDATA%\devin\wf-core\devin-hooks.v1.json` or `~/.config/devin/wf-core/devin-hooks.v1.json` records the managed hook artifact.

## Commands

```bash
wf-core hook instructions
wf-core hook list --target all --channel both
wf-core hook install --target all --channel both --source-root <repo>
```

## Runtime Rule

Before noisy terminal commands, run `wf-core run -- <command>`. For shell syntax,
run `wf-core run --shell -- "<command>"`. Never capture or print credentials.
