# wf-core Agent Rules

These rules define the managed Windsurf/Windsurf Next behavior for this repo.

## Native Command Routing - Must Follow First

When a `wf-core` command owns the job, use it instead of recreating the
behavior with raw shell, generic search, or ad hoc instructions.

Token-saving rule: prevent noisy command output from entering the agent context.
Do not run a raw noisy command first and compact afterward. Route through
`wf-core run -- <command>` before output is produced.

Use the wrapper for tests, builds, lints, logs, broad search, status, Docker,
Kubernetes, Terraform, package managers, and CI-style commands.

If `wf-core` is not on PATH, use the installed binary path from the active agent
home, such as `%USERPROFILE%\.codeium\windsurf-next\wf-core\wf-core.exe` or
`%APPDATA%\devin\wf-core\wf-core.exe` on Windows.

If a command contains pipes, redirects, `&&`, `||`, or command substitution, use:

```bash
wf-core run --shell -- "<command>"
```

If unsure, run:

```bash
wf-core rewrite "<command>"
```

## Secret Safety

- Token saving means context-output tokens only.
- Never store, print, scrape, or back up authentication tokens, API keys,
  cookies, session secrets, or credentials.
- Raw command output is saved locally under `~/.codeium/<channel>/wf-core/raw-output/`.
  Do not run commands that print secrets unless the user explicitly asks and
  approves the local risk.

## Skill Routing

Use one primary skill when the task clearly fits:

- `reviewer`: reviews, audits, quality gates, production-readiness checks
- `software-development-life-cycle`: broad planning, sequencing, architecture framing
- `preserve-existing-flow`: pre-edit behavior ownership tracing
- `web-development-life-cycle`: web app architecture, browser behavior, performance, SEO
- `mobile-development-life-cycle`: Android/iOS lifecycle, permissions, offline sync
- `backend-and-data-architecture`: APIs, databases, services, queues, caching
- `cloud-and-devops-expert`: CI/CD, IaC, containers, rollout, operations
- `qa-and-automation-engineer`: test strategy, E2E, regression proof
- `security-and-compliance-auditor`: threat modeling, secrets, auth, compliance
- `ui-design-systems-and-responsive-interfaces`: UI, accessibility, responsive systems
- `ux-research-and-experience-strategy`: UX research, journeys, decision quality
- `git-expert`: branch, commit, PR, merge, conflict, and recovery workflows
- `memory-status-reporter`: memory summaries and learning/mistake status

Do not load multiple skills for simple tasks. Use `reviewer` as a final gate
only when risk or the user request justifies it.

## Preserve Existing Flow

Before changing existing source behavior, identify entry point, producer, source
of truth, state/storage/queue owner, side-effect owner, consumers, cleanup or
recovery path, edit boundary, validation needed, and validation evidence.

Do not patch the first suspicious branch until the owner path is understood.

Use the native Rust flow surface for the artifact lifecycle:

```bash
wf-core flow start --target-file <path> --target-function <name>
wf-core flow check
wf-core flow finish
```

Before final closeout, prefer native review and git workflow gates:

```bash
wf-core review gates check --repo-root .
wf-core git-workflow commit-message --repo-root . --test-result "<result>"
wf-core git-workflow pr-body --repo-root . --test-result "<result>"
wf-core git-workflow lint-message <file>
```

Global installs must pass `wf-core verify --channel both`; this includes manifest
checksum validation and stale managed-file detection.

For Devin-local parity, install and verify with `--target all`:

```bash
wf-core install --target all --channel both --source-root <repo>
wf-core verify --target all --channel both
wf-core hook list --target all --channel both
```

Use native shared memory and system-map commands before broad rediscovery:

```bash
wf-core memory status --repo-root .
wf-core memory system-map refresh --repo-root .
wf-core memory system-map verify --repo-root .
```

## Completion Loop

Follow this loop until the task is production-ready:

```text
ALIGN -> RESEARCH -> PLAN -> IMPLEMENT -> TEST -> FIX -> VERIFY -> REVIEW -> RECONCILE
```

Stop early only for missing permission, ambiguous requirements, or an explicit
out-of-scope boundary. Final answers must state what was changed, what was
validated, and any remaining risk.
