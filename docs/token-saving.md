# Token-Saving Command Compaction

`wf-core run` is a Rust-native wrapper. It executes a command, preserves its
exit code, saves the full raw output locally, and prints a compact high-signal
summary when output is large.

## Use It Before Noisy Commands

```bash
wf-core run -- cargo test --workspace
wf-core run -- pytest -q
wf-core run -- npm test
wf-core run -- git status --short --branch
wf-core run --shell -- "npm test 2>&1 | tee test.log"
```

## What Gets Compacted

The wrapper compacts when output exceeds the line or byte threshold. It keeps:

- Command, exit code, elapsed time, raw byte count, and raw output path
- High-signal error/warning/failure lines with nearby context
- Head and tail of the original output

## Raw Output Recovery

Raw output is saved at:

```text
~/.codeium/<channel>/wf-core/raw-output/
```

Use this only for local recovery. Do not intentionally run commands that print
secrets into raw-output logs.

## Savings Analytics

```bash
wf-core gain --channel next
wf-core gain --channel stable --json
```

Analytics are stored as local JSONL events under:

```text
~/.codeium/<channel>/wf-core/gain/events.jsonl
```
