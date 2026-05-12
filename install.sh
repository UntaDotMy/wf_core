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

activation_channel="next"
case "$channel" in
  stable) activation_channel="stable" ;;
  insiders) activation_channel="insiders" ;;
  next|both) activation_channel="next" ;;
esac

activation_binary="${HOME}/.codeium/windsurf-next/wf-core/wf-core"
case "$activation_channel" in
  stable) activation_binary="${HOME}/.codeium/windsurf/wf-core/wf-core" ;;
  insiders) activation_binary="${HOME}/.codeium/windsurf-insiders/wf-core/wf-core" ;;
esac
if [ "$target" = "devin" ]; then
  activation_binary="${HOME}/.config/devin/wf-core/wf-core"
fi

"$binary" install --target "$target" --channel "$channel" --source-root "$script_dir"
"$binary" verify --target "$target" --channel "$channel"
"$binary" doctor --proxy --target "$target" --channel "$channel" || true

printf '\nwf-core proxy activation:\n'
printf '  eval "$(%s shell init --channel %s)"\n' "$activation_binary" "$activation_channel"
printf '  %s doctor --proxy --channel %s\n' "$activation_binary" "$activation_channel"

if [ "$modify_shell_profile" = "true" ]; then
  profile="${HOME}/.profile"
  [ -n "${SHELL:-}" ] && [ "$(basename "$SHELL")" = "zsh" ] && profile="${HOME}/.zshrc"
  backup="${profile}.wf-core.bak.$(date +%Y%m%d%H%M%S)"
  backup_created="false"
  if [ -f "$profile" ]; then
    cp "$profile" "$backup"
    backup_created="true"
  fi
  managed_block="$(mktemp)"
  {
    printf '# wf-core managed:start\n'
    printf 'eval "$(%s shell init --channel %s)"\n' "$activation_binary" "$activation_channel"
    printf '# wf-core managed:end\n'
  } > "$managed_block"
  if [ -f "$profile" ] &&
    grep -q '# wf-core managed:start' "$profile" &&
    grep -q '# wf-core managed:end' "$profile"; then
    tmp_profile="${profile}.wf-core.tmp.$$"
    awk -v block_file="$managed_block" '
      BEGIN {
        while ((getline line < block_file) > 0) {
          block = block line ORS
        }
      }
      /# wf-core managed:start/ {
        printf "%s", block
        in_block = 1
        next
      }
      /# wf-core managed:end/ && in_block {
        in_block = 0
        next
      }
      !in_block { print }
    ' "$profile" > "$tmp_profile"
    mv "$tmp_profile" "$profile"
  else
    {
      [ -f "$profile" ] && printf '\n'
      cat "$managed_block"
    } >> "$profile"
  fi
  rm -f "$managed_block"
  if [ "$backup_created" = "true" ]; then
    printf 'Updated %s (backup: %s)\n' "$profile" "$backup"
  else
    printf 'Updated %s (no previous file to back up)\n' "$profile"
  fi
else
  printf 'Shell profile not modified. Pass --modify-shell-profile to append a managed block.\n'
fi
