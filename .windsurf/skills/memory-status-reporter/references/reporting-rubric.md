# Memory Reporting Rubric

## Purpose

Use this rubric when turning `wf-core memory status`, `wf-core memory recall`, and current task evidence into a human-readable memory report.

## Source Priority

1. `wf-core memory status --repo-root <repo>` for workspace-level availability and system-map freshness.
2. `wf-core memory recall --repo-root <repo> --kind <kind>` for durable notes, research, mistakes, preferences, and requirements.
3. `wf-core memory system-map show --repo-root <repo>` when architecture context affects the report.
4. Current validation/review evidence from the active task.

## Status Labels

- **Healthy**: useful memory exists, no open mistakes dominate, and system-map evidence is fresh enough for the request.
- **Mixed**: useful memory exists but has open mistakes, stale areas, or unclear evidence.
- **Needs Attention**: important memory is missing, stale, contradictory, or dominated by unresolved mistakes.
- **Quiet**: no relevant memory entries are available for the requested scope.

## Resolution Labels

- **Resolved**: the memory entry or validation evidence includes a fix and proof.
- **Open**: the memory entry still names a blocker, follow-up, or unresolved risk.
- **Unclear**: evidence is too thin to classify confidently.

## Honesty Rule

Label any percentages, brain-growth language, or learning momentum as heuristic reporting aids derived from memory artifacts, not literal cognition measurements.
