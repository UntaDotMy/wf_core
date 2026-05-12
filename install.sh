#!/usr/bin/env bash
# Build and install the Rust-native wf-core global surfaces for Windsurf, Windsurf Next, and Devin.

set -euo pipefail

channel="both"
target="all"
modify_shell_profile="false"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --channel)
      channel="${2:-}"
      shift 2
      ;;
    --channel=*)
      channel="${1#*=}"
      shift
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --target=*)
      target="${1#*=}"
      shift
      ;;
    --modify-shell-profile)
      modify_shell_profile="true"
      shift
      ;;
    --scope|--project-root)
      printf 'wf-core install is global-only; %s is not supported.\n' "$1" >&2
      exit 1
      ;;
    --scope=*|--project-root=*)
      printf 'wf-core install is global-only; %s is not supported.\n' "$1" >&2
      exit 1
      ;;
    -h|--help)
      printf 'Usage: ./install.sh [--target windsurf|devin|all] [--channel stable|next|insiders|both] [--modify-shell-profile]\n'
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

if [ -n "${CARGO:-}" ]; then
  cargo_bin="$CARGO"
elif command -v cargo >/dev/null 2>&1; then
  cargo_bin="cargo"
else
  printf 'wf-core installer requires cargo on PATH.\n' >&2
  exit 1
fi

"$cargo_bin" build --release --locked --manifest-path "$script_dir/Cargo.toml"

binary="$script_dir/target/release/wf-core"
if [ -x "$script_dir/target/release/wf-core.exe" ]; then
  binary="$script_dir/target/release/wf-core.exe"
fi

"$binary" install --target "$target" --channel "$channel" --source-root "$script_dir"
"$binary" verify --target "$target" --channel "$channel"
"$binary" doctor --proxy --target "$target" --channel "$channel" || true

printf '\nwf-core proxy activation:\n'
printf '  eval "$(%s shell init --channel next)"\n' "$binary"
printf '  %s doctor --proxy --channel next\n' "$binary"

if [ "$modify_shell_profile" = "true" ]; then
  profile="${HOME}/.profile"
  [ -n "${SHELL:-}" ] && [ "$(basename "$SHELL")" = "zsh" ] && profile="${HOME}/.zshrc"
  backup="${profile}.wf-core.bak.$(date +%Y%m%d%H%M%S)"
  [ -f "$profile" ] && cp "$profile" "$backup"
  {
    printf '\n# wf-core managed:start\n'
    printf 'eval "$(%s shell init --channel next)"\n' "$binary"
    printf '# wf-core managed:end\n'
  } >> "$profile"
  printf 'Updated %s (backup: %s)\n' "$profile" "$backup"
else
  printf 'Shell profile not modified. Pass --modify-shell-profile to append a managed block.\n'
fi
