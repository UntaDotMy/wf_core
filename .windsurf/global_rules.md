# wf-core Global Rules for Windsurf

These rules install into `~/.codeium/<channel>/memories/global_rules.md` for
Windsurf stable and Windsurf Next. The two apps are the same product family but
use different global homes: `~/.codeium/windsurf/` and
`~/.codeium/windsurf-next/`.

## Token-Saving Rule

- Token saving means reducing noisy terminal output that enters the agent context.
- Never store, print, scrape, or collect authentication tokens, API keys, cookies, or secrets.
- Real automatic saving comes from wf-core-managed shims/proxy intercepting output before it reaches context; rules alone do not intercept terminal output.
- At session start before noisy terminal work, verify proxy mode with `wf-core doctor --proxy --channel next` or `wf-core shim doctor --channel next`.
- Run noisy commands through the wf-core proxy. If shim mode is active, call the command normally. If shim mode is not active, use `wf-core run -- <command>`.
- If `wf-core` is not on PATH, use the installed binary path from the active
  agent home, such as `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe`
  or `%APPDATA%\devin\wf-core\wf-core.exe` on Windows.
- If shim mode is not active and the command uses pipes, redirects, `&&`, `||`, or shell syntax, use `wf-core run --shell -- "<command>"`.
- Use `wf-core rewrite "<command>"` or `wf-core hook instructions` when unsure whether the wrapper is useful.

## Hook-Equivalent Terminal Rule

- Treat this rule as the pre-terminal hook discipline for Windsurf sessions.
- Run noisy commands through wf-core before raw output is produced: native shim when active, explicit `wf-core run --` otherwise.
- Continue from the compacted output and preserve the command exit code.
- Use the `wf-core-hooks` workflow for terminal-heavy work.

## Routing Rule

- Use the most specific global skill from `~/.codeium/<channel>/skills/<name>/SKILL.md` for domain work.
- Use `preserve-existing-flow` before changing existing behavior, state, queues, handlers, protocols, or side effects.
- Use `reviewer` for explicit reviews, production-readiness checks, security/quality gates, and final validation.
- Use `software-development-life-cycle` for broad planning, sequencing, or cross-domain coordination.

## Delivery Rule

- Translate the user request into a working brief before implementation.
- Keep changes small and reversible.
- Validate with the narrowest proving command first, then widen only when needed.
- Finish by reconciling every explicit user requirement against evidence.
