# Security Notes

`wf_core` is designed to save context-output tokens, not user credentials.

## It Does Not

- Read Windsurf auth databases
- Copy cookies or session state
- Persist API keys or auth tokens
- Send telemetry or analytics over the network
- Install browser hooks or editor extensions

## It Does

- Copy managed rules, skills, and workflows into Windsurf config directories
- Save raw command output locally under `~/.codeium/<channel>/wf-core/raw-output/`
- Record local output-size analytics under `~/.codeium/<channel>/wf-core/gain/events.jsonl`

## User Responsibility

Do not run commands through `wf-core run` if those commands print secrets unless
you accept that the raw output log will contain those secrets locally.
