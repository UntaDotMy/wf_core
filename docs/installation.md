# Installation

`wf_core` is Rust-native and global-only. Install writes to Windsurf global
Codeium channel homes and Devin for Terminal's global config home, not to an
arbitrary user workspace.

## Channel Homes

```text
~/.codeium/windsurf/          # Windsurf stable
~/.codeium/windsurf-next/     # Windsurf Next
~/.codeium/windsurf-insiders/ # Windsurf Insiders
%APPDATA%\devin\              # Devin for Terminal on Windows
~/.config/devin/              # Devin for Terminal on macOS/Linux
%APPDATA%\wf-core\            # Shared wf-core memory on Windows
~/.local/share/wf-core/       # Shared wf-core memory on macOS/Linux
```

## Install Windsurf Stable, Windsurf Next, and Devin Local

PowerShell:

```powershell
.\install.ps1 -Target all -Channel both
```

Bash:

```bash
./install.sh --target all --channel both
```

CMD:

```bat
install.cmd -Target all -Channel both
```

Then restart Windsurf, Windsurf Next, and Devin for Terminal.

## Install One Channel

```bash
cargo build --release
./target/release/wf-core install --target windsurf --channel next --source-root "$PWD"
./target/release/wf-core install --target windsurf --channel stable --source-root "$PWD"
./target/release/wf-core install --target windsurf --channel insiders --source-root "$PWD"
./target/release/wf-core install --target devin --source-root "$PWD"
```

## Verify

```bash
~/.config/devin/wf-core/wf-core status --target all --channel both
~/.config/devin/wf-core/wf-core verify --target all --channel both
~/.config/devin/wf-core/wf-core doctor --target all --channel both
~/.config/devin/wf-core/wf-core hook list --target all --channel both
```

`verify` checks more than existence. The installer writes
`manifest.tsv` in each managed home with each managed standalone file, its
checksum, and its byte size. Verification recomputes those checksums, verifies
Devin hook/config wiring, and reports stale files left in managed skill
directories, `wf-core-*` workflows, or the bundled wf-core payload.

Windows:

```powershell
& "$env:APPDATA\devin\wf-core\wf-core.exe" doctor --target all --channel both
```

## Uninstall Managed Global Files

Uninstall removes only the managed global files owned by this repo:

```bash
~/.config/devin/wf-core/wf-core uninstall --target all --channel both --yes
```
