# Preserve Existing Flow Schema

Use this schema when a non-trivial edit changes existing source behavior.

Create and validate the artifact with the native Rust command surface:

```bash
wf-core flow start --target-file src/example.rs --target-function handle_example --task "preserve behavior"
wf-core flow check
wf-core flow finish
```

By default the artifact is stored globally per channel and workspace at
`~/.codeium/<channel>/wf-core/memories/workspaces/<workspace-slug>/flow/flow-check.json`.
Use `--repo-root`, `--channel`, `--output`, or `--artifact` to override those
defaults when needed.

```json
{
  "version": 1,
  "task": "short task label",
  "target_file": "src/example.ts",
  "target_function": "handleExample",
  "current_behavior_to_preserve": "What currently works and must stay working.",
  "entry_point": "Where the input, command, event, request, or timer enters.",
  "producer": "Where intent or data is created.",
  "source_of_truth": "Where the final behavior decision is owned.",
  "storage_state_queue_owner": "Where state, queue, cache, or persistence is owned; use Not found if none exists.",
  "side_effect_owner": "Where the system writes, sends, persists, renders, or mutates outside state.",
  "consumers": ["Who reads or acts on the value; use Not found if none exists."],
  "cleanup_recovery_path": "Where success, failure, retry, rollback, or cleanup is handled.",
  "edit_boundary": "The files or functions allowed to change and what must not move.",
  "validation_needed": ["The checks that prove preserved behavior."],
  "validation_evidence": ["The checks actually run and their outcome."],
  "duplicate_owner_logic": false,
  "migration_approved": false,
  "docs_only": false,
  "formatting_only": false,
  "generated_only": false,
  "greenfield": false
}
```

Required for existing-source edits:

- Owner path fields must be filled.
- At least one consumer or `Not found` must be recorded.
- Validation needed and validation evidence must be present.
- If `duplicate_owner_logic` is true, `migration_approved` must also be true.
- Docs-only, formatting-only, generated-only, and greenfield changes can be
  exempt when explicitly labeled.
