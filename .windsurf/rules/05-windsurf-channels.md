---
description: "Keep Windsurf stable and Windsurf Next global config separate but equivalent"
trigger: always_on
---

# Windsurf Channels

- Windsurf stable and Windsurf Next are the same product family, but they use different global config folders.
- Windsurf stable global home is `~/.codeium/windsurf/`.
- Windsurf Next global home is `~/.codeium/windsurf-next/`.
- Keep wf-core rules, skills, workflows, and terminal hook discipline equivalent in both channel folders.
- Do not install managed wf-core files into an arbitrary user workspace; use the global channel homes.
