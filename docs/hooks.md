# Hook And Proxy Terminal Policy

Windsurf behavior is configured through global rules, workflows, and skills.
Devin for Terminal has native lifecycle hooks. `wf_core` installs the same
terminal policy into Windsurf stable, Windsurf Next, and Devin global homes.

Windsurf does not provide a guaranteed native PreToolUse hook in this repo.
Windsurf automation is enforced through wf-core-managed shims plus global
rules/workflows. Devin also has PreToolUse hook support.

## Installed Pieces

- `~/.codeium/<channel>/memories/global_rules.md` contains the always-on terminal policy.
- `~/.codeium/<channel>/windsurf/workflows/wf-core-hooks.md` gives the manual hook workflow.
- `~/.codeium/<channel>/windsurf/workflows/wf-core-token-saving-shell.md` gives the command wrapper workflow.
- `~/.codeium/<channel>/wf-core/wf-core(.exe)` provides `shim`, `dispatch`, `shell init`, `rewrite`, `run`, `raw`, `replay`, `gain`, and `discover` commands.
- `~/.codeium/<channel>/wf-core/shims/` contains managed native shims for noisy commands.
- `%APPDATA%\devin\config.json` or `~/.config/devin/config.json` contains the Devin `PreToolUse` hook.
- `%APPDATA%\devin\wf-core\devin-hooks.v1.json` or `~/.config/devin/wf-core/devin-hooks.v1.json` records the managed hook artifact.

## Commands

```bash
wf-core hook instructions
wf-core hook list --target all --channel both
wf-core hook install --target all --channel both --source-root <repo>
wf-core shim install --target all --channel both
wf-core shell init --channel next
wf-core doctor --proxy --channel next
```

## Runtime Rule

Run noisy commands through the wf-core proxy. If shim mode is active, call the
command normally and let the shim intercept it. If shim mode is not active, use
`wf-core run -- <command>` before raw output is produced. For shell syntax, run
`wf-core run --shell -- "<command>"`.

Never capture or print credentials. Compact output redacts likely secrets, but
raw recovery logs are full local command output.
