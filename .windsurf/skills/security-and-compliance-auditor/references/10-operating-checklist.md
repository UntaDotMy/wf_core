# Security and Compliance Auditor Operating Checklist

1. Restate the user goal, constraints, and expected evidence.
2. Search narrowly first; widen only when the owner path is unclear.
3. Use `wf-core run -- <command>` for noisy validation commands.
4. Preserve current behavior and avoid duplicate ownership.
5. Validate the smallest proving surface, then widen for release risk.
6. Report changed files, evidence, and residual risks.

## Domain Checks

- Confirm threat models, trust boundaries, and authz/authn.
- Confirm secret handling, dependency, and supply-chain risk.
- Confirm compliance evidence and remediation prioritization.
