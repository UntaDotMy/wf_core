---
auto_execution_mode: 0
description: Review current changes for bugs, regressions, security issues, and missing tests
---

Use the `reviewer` skill. Focus on high-confidence findings first:

1. Logic errors and behavior regressions
2. Data loss, concurrency, lifecycle, or cleanup bugs
3. Security and secret-handling risks
4. Missing validation or test coverage
5. Integration and API contract mismatches

Report findings with file/line references when possible. If no findings are found, state that and note residual testing gaps.
