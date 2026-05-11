---
description: "Route Windsurf work through wf-core skills and workflows"
trigger: always_on
---

# wf-core Routing

- Use managed Windsurf skills from `.windsurf/skills/` when a task clearly belongs to a specialist domain.
- Keep one primary skill responsible for the user-facing answer.
- Use `software-development-life-cycle` for broad sequencing and architecture framing.
- Use `reviewer` only for explicit reviews, audits, release gates, or final production-readiness checks.
- Use `preserve-existing-flow` before changing existing source behavior or ownership.
- Do not load multiple skills for simple tasks when local reasoning is enough.
