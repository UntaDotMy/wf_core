# Preserve Existing Flow Operating Checklist

1. Restate the user goal, constraints, and expected evidence.
2. Search narrowly first; widen only when the owner path is unclear.
3. Use `wf-core run -- <command>` for noisy validation commands.
4. Preserve current behavior and avoid duplicate ownership.
5. Validate the smallest proving surface, then widen for release risk.
6. Report changed files, evidence, and residual risks.

## Domain Checks

- Confirm entry point, producer, source of truth, and owner path.
- Confirm state/storage/queue, side-effect, consumer, and recovery ownership.
- Confirm edit boundary and validation evidence before patching.
