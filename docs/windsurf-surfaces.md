# Windsurf Surfaces

Windsurf stable and Windsurf Next are the same product family, but they keep
separate global config folders. `wf_core` installs equivalent managed files into
each channel home.

## Channel Homes

```text
~/.codeium/windsurf/          # Windsurf stable
~/.codeium/windsurf-next/     # Windsurf Next
~/.codeium/windsurf-insiders/ # Windsurf Insiders
```

## Source Bundle In This Repo

These source files are committed here and copied into the global channel homes:

```text
AGENTS.md
.windsurf/
  global_rules.md
  rules/
    00-wf-core-routing.md
    05-windsurf-channels.md
    10-token-saving.md
    20-preserve-existing-flow.md
    30-terminal-hook-policy.md
  skills/
    <skill-name>/SKILL.md
  workflows/
    wf-core-*.md
```

## Installed Global Surface

```text
~/.codeium/<channel>/memories/global_rules.md
~/.codeium/<channel>/skills/<skill-name>/SKILL.md
~/.codeium/<channel>/windsurf/workflows/wf-core-*.md
~/.codeium/<channel>/wf-core/AGENTS.md
~/.codeium/<channel>/wf-core/wf-core(.exe)
~/.codeium/<channel>/wf-core/shims/
~/.codeium/<channel>/wf-core/raw-output/
~/.codeium/<channel>/wf-core/gain/events.jsonl
```

The installer preserves existing global rules by replacing only the wf-core
managed block. Managed workflows use the `wf-core-` prefix so existing user
workflows such as `review.md` are not overwritten.

Restart Windsurf after global install so newly copied global skills and
workflows are discovered. Then activate the command proxy in each shell:

```bash
wf-core shim install --channel next
eval "$(wf-core shell init --channel next)"
wf-core doctor --proxy --channel next
```

Rules and workflows do not save tokens by themselves. Real automatic savings
require the shim directory to be ahead of the real tools on `PATH`; otherwise use
the explicit `wf-core run -- <command>` fallback.

For Devin for Terminal, see `docs/devin-local.md`; the same skill pack is also
installed into Devin's native global config home.
