# wf_core

Rust-native global installer for Windsurf, Windsurf Next, and Devin for
Terminal rules, skills, workflows, AGENTS guidance, hook-equivalent terminal
discipline, memory/system-map surfaces, and token-saving command output
proxying.

Windsurf stable, Windsurf Next, and Devin for Terminal can read overlapping
Windsurf surfaces, but they also have different global folders. `wf_core`
installs the same managed surface into all supported homes so behavior stays
consistent across local agents.

Important: "save token" means save context/output tokens. `wf_core` never
collects, uploads, or backs up user auth tokens, API keys, cookies, or session
secrets. Raw command output stays local for recovery and may contain whatever
the command printed.

## Global-Only Install

Install writes only to global agent homes, not to an arbitrary user workspace:

| Channel | Global folder |
| --- | --- |
| Windsurf stable | `~/.codeium/windsurf/` |
| Windsurf Next | `~/.codeium/windsurf-next/` |
| Windsurf Insiders | `~/.codeium/windsurf-insiders/` |
| Devin for Terminal | `~/.config/devin/` or `%APPDATA%\devin\` |

| Surface | Global Windsurf path |
| --- | --- |
| Instructions/rules | `~/.codeium/<channel>/memories/global_rules.md` |
| Skills | `~/.codeium/<channel>/skills/<name>/SKILL.md` |
| Workflows | `~/.codeium/<channel>/windsurf/workflows/wf-core-*.md` |
| AGENTS guidance copy | `~/.codeium/<channel>/wf-core/AGENTS.md` |
| Rust binary | `~/.codeium/<channel>/wf-core/wf-core(.exe)` |
| Native command shims | `~/.codeium/<channel>/wf-core/shims/` |
| Install manifest | `~/.codeium/<channel>/wf-core/manifest.tsv` |
| Raw output and analytics | `~/.codeium/<channel>/wf-core/raw-output/`, `gain/events.jsonl` |

| Surface | Global Devin path |
| --- | --- |
| Skills | `%APPDATA%\devin\skills\<name>\SKILL.md` or `~/.config/devin/skills/<name>/SKILL.md` |
| Custom agents | `%APPDATA%\devin\agents\<name>\AGENT.md` or `~/.config/devin/agents/<name>/AGENT.md` |
| Config imports/hooks | `%APPDATA%\devin\config.json` or `~/.config/devin/config.json` |
| Hook artifact and binary | `%APPDATA%\devin\wf-core\` or `~/.config/devin/wf-core/` |
| Shared memory/system map | `%APPDATA%\wf-core\memories\` or `~/.local/share/wf-core/memories/` |

## Quick Install

Prerequisites:

- Git, for cloning the repository.
- Rust/Cargo, for building the native `wf-core` binary from source.

Clone once:

```bash
git clone https://github.com/UntaDotMy/wf_core.git
cd wf_core
```

Windows PowerShell:

```powershell
.\install.ps1 -Target all -Channel both
& "$env:APPDATA\devin\wf-core\wf-core.exe" doctor --target all --channel both
```

macOS, Linux, WSL, or Git Bash:

```bash
./install.sh --target all --channel both
~/.config/devin/wf-core/wf-core doctor --target all --channel both
```

Windows CMD:

```bat
install.cmd -Target all -Channel both
%APPDATA%\devin\wf-core\wf-core.exe doctor --target all --channel both
```

The installer builds the Rust binary with Cargo, copies that binary globally,
and installs global instructions, skills, workflows, Devin agents, Devin hooks,
memory support, and hook-equivalent terminal policy. Restart Windsurf, Windsurf
Next, and Devin for Terminal after install so global surfaces refresh.

Successful install shows:

- Windsurf stable/next: `skills: 13/13`
- Devin: `skills: 13/13`, `agents: 13/13`, and `config hooks/imports: yes`

If `wf-core` is not on `PATH`, use the installed binary path shown by the
installer.

## Native Token-Saving Proxy

Real savings happen only when `wf-core` sits between the agent and the noisy
command before raw output reaches model context. Rules and workflows guide the
agent; the native proxy/shim does the interception.

Automatic shim mode:

```bash
wf-core shim install --channel next
eval "$(wf-core shell init --channel next)"
wf-core doctor --proxy --channel next
```

After activation, common noisy commands are intercepted automatically:

```bash
cargo test --workspace
pytest -q
git diff
rg "CompactResult" .
docker logs api
```

Explicit fallback works everywhere:

```bash
wf-core run -- cargo test --workspace
wf-core run -- pytest -q
wf-core run -- git diff
wf-core run -- rg "foo" .
wf-core run --shell -- "npm test 2>&1 | tee test.log"
```

Recovery and analytics:

```bash
wf-core raw <raw_id>
wf-core raw list --limit 20
wf-core replay <raw_id>
wf-core gain --channel next
wf-core discover --channel next
wf-core rewrite "cargo test --workspace"
```

Compact output always includes `raw: wf-core raw <raw_id>` when output is
compacted, plus estimated before/after token savings. Estimated tokens use a
local approximation; do not claim savings that are not measured in gain events.

Installed global binaries:

- Windsurf Next Windows: `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe`
- Windsurf stable Windows: `%USERPROFILE%\.codeium\windsurf\wf-core\wf-core.exe`
- Devin Windows: `%APPDATA%\devin\wf-core\wf-core.exe`
- Windsurf Next macOS/Linux: `~/.codeium/windsurf-next/wf-core/wf-core`
- Windsurf stable macOS/Linux: `~/.codeium/windsurf/wf-core/wf-core`
- Devin macOS/Linux: `~/.config/devin/wf-core/wf-core`

## Managed Skill Inventory

The global install ships 13 specialist skills:

- `backend-and-data-architecture`
- `cloud-and-devops-expert`
- `git-expert`
- `memory-status-reporter`
- `mobile-development-life-cycle`
- `preserve-existing-flow`
- `qa-and-automation-engineer`
- `reviewer`
- `security-and-compliance-auditor`
- `software-development-life-cycle`
- `ui-design-systems-and-responsive-interfaces`
- `ux-research-and-experience-strategy`
- `web-development-life-cycle`

## Hooks In Windsurf And Devin Terms

Windsurf uses global rules, workflows, and skills for agent behavior. `wf_core`
installs a hook-equivalent terminal policy through global rules and the
`wf-core-hooks` / `wf-core-token-saving-shell` workflows. The Rust CLI also
provides `wf-core hook instructions`, `wf-core hook list`, and
`wf-core hook install` for refreshing and inspecting that surface.

Devin for Terminal also receives global skills and matching custom agents in its
native config home plus a managed `PreToolUse` hook in `config.json`. The hook
blocks noisy raw `exec` commands with `Rerun that as: wf-core run -- ...` before
output reaches context.

## Core Commands

```bash
wf-core install --target all --channel both --source-root <repo>
wf-core update --target all --channel both
wf-core status --target all --channel both
wf-core verify --target all --channel both  # existence, checksum, hooks, and stale-file checks
wf-core doctor --target all --channel both
wf-core flow start --target-file src/main.rs --target-function run
wf-core flow check
wf-core review gates check --repo-root .
wf-core git-workflow commit-message --repo-root . --test-result "cargo test --locked passed"
wf-core git-workflow pr-body --repo-root . --test-result "cargo test --locked passed"
wf-core hook list --channel both
wf-core memory status --repo-root .
wf-core memory system-map refresh --repo-root .
wf-core memory system-map verify --repo-root .
wf-core shim install --channel next
wf-core shell init --channel next
wf-core doctor --proxy --channel next
wf-core discover --channel next
wf-core hook instructions
wf-core rewrite "pytest -q"
wf-core run -- pytest -q
wf-core gain --channel next
```

## Native Flow, Review, And Git Workflow

- `flow start|check|finish` writes preserve-existing-flow evidence under the selected global channel by default: `~/.codeium/<channel>/wf-core/memories/workspaces/<workspace-slug>/flow/flow-check.json`.
- `review pre-pr|pre-commit|gates check` runs deterministic local gates and can emit `--format markdown|compact|json`; `review hosted check` writes Markdown and JSON artifacts.
- `git-workflow commit-message|pr-body|lint-message|preflight` renders professional commit/PR text, lints rendered text, and blocks preflight on unsafe branch/diff state.
- `verify` reads `manifest.tsv`, recomputes checksums for managed standalone files, and reports stale files left in managed skill/workflow/bundle surfaces.

## Native Memory And System Map

- `memory remember|recall|status` stores workspace-scoped notes, research-cache entries, and mistake records under the shared wf-core data home.
- `memory system-map refresh|show|verify` generates and verifies a stale-detectable `SYSTEM_MAP.md` with top-level areas, key files, Rust command handlers, managed skills, and validation commands.
- The system map includes an inventory fingerprint so `verify` can detect when repository files changed and the map must be refreshed.

## Repository Layout

```text
Cargo.toml
src/main.rs
AGENTS.md
.windsurf/
  global_rules.md
  rules/
  skills/
  workflows/wf-core-*.md
docs/
templates/
install.sh
install.ps1
install.cmd
```

## Validation

```bash
cargo fmt --check
cargo test --locked
cargo build --release --locked
./target/release/wf-core rewrite "git status --short"
./target/release/wf-core flow start --target-file src/main.rs --output /tmp/wf-flow.json
./target/release/wf-core review gates check --repo-root . --format compact
./target/release/wf-core memory system-map refresh --repo-root .
./target/release/wf-core memory system-map verify --repo-root .
```
