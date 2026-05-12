# Native Token-Saving Command Proxy

`wf-core` is a Rust-native command proxy. It executes noisy terminal commands,
captures full stdout/stderr locally, returns compact semantic output to the
agent, preserves the original exit code, and records estimated token savings.
It does not integrate RTK and does not shell out to RTK.

Token saving means output/context token saving, not auth token storage. Raw
output stays local and may contain secrets if the command printed them; compact
output redacts likely secret-looking lines by default.

## Automatic Shim Mode

Automatic mode is the RTK-like path: shims sit first in `PATH` and call
`wf-core dispatch` internally.

```bash
wf-core shim install --channel next
eval "$(wf-core shell init --channel next)"
wf-core doctor --proxy --channel next
```

Then run commands normally:

```bash
cargo test --workspace
pytest -q
npm test
git diff
rg "foo" .
cat src/main.rs
docker logs api
kubectl logs deploy/api
```

The shim decides whether the command is noisy. Known noisy tests, builds, lints,
searches, diffs, large file reads, logs, and infra outputs route through the
proxy. Interactive or risky commands pass through.

## Explicit Fallback

Explicit mode is always reliable and does not depend on `PATH` order:

```bash
wf-core run -- cargo test --workspace
wf-core run -- pytest -q
wf-core run -- git diff
wf-core run -- rg "foo" .
wf-core run --shell -- "npm test 2>&1 | tee test.log"
```

Useful flags:

```bash
wf-core run --json -- <command>
wf-core run --full -- <command>
wf-core run --adapter tests -- <command>
wf-core run --list-adapters
```

## Semantic Reducers

Adapters are registered in priority order: `tests`, `git`, `search`,
`build_lint`, `files`, `logs`, then `generic`. The generic adapter is fallback
only. Compact output includes:

- `PASS`/`FAIL`/`DIFF`/`SEARCH`/`FILE`/`LOGS` style summary
- failing tests, diagnostics, file:line paths, high-signal logs, or omitted counts
- `raw: wf-core raw <raw_id>` for recovery when compacted
- `saved: <n> tokens estimated (<pct>%)`

## Raw Recovery

Raw output is saved under:

```text
~/.codeium/<channel>/wf-core/raw-output/YYYY-MM-DD/<raw_id>/
%APPDATA%\devin\wf-core\raw-output\YYYY-MM-DD\<raw_id>\
$WF_CORE_HOME/raw-output/YYYY-MM-DD/<raw_id>/
```

Commands:

```bash
wf-core raw <raw_id>
wf-core raw --path <raw_id>
wf-core raw list --limit 20
wf-core raw prune --older-than 30d
wf-core replay <raw_id>
```

`replay` refuses destructive-looking commands unless `--allow-risky` is given.

## Gain And Discover

```bash
wf-core gain --channel next
wf-core gain --channel next --json
wf-core gain --since 7d --adapter tests
wf-core discover --channel next --since 7d --min-tokens 10000
```

Gain events are local JSONL records under:

```text
~/.codeium/<channel>/wf-core/gain/events.jsonl
```

Savings are estimated tokens using the local token meter. Do not claim savings
percentages unless they came from `wf-core gain` events.

## Limitations

- Windsurf rules alone do not intercept output; automatic saving requires
  shim/PATH proxy mode.
- Explicit `wf-core run --` is the reliable fallback for every host.
- Estimated tokens are approximate.
- Raw output may contain secrets because it is full local recovery data.
