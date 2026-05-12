#!/usr/bin/env bash
# Bootstrap wf-core from the latest GitHub release, or build from source with --from-source.

set -euo pipefail

repository="${WF_CORE_REPOSITORY:-UntaDotMy/wf_core}"
version="${WF_CORE_VERSION:-latest}"
channel="${WF_CORE_CHANNEL:-both}"
target="${WF_CORE_TARGET:-all}"
modify_shell_profile="true"
from_source="false"

usage() {
  cat <<'USAGE'
Usage: install.sh [--target windsurf|devin|all] [--channel stable|next|insiders|both] [--version latest|vX|bootstrap-X] [--repository owner/repo] [--no-modify-shell-profile] [--from-source]

Default mode downloads the matching wf-core release archive and installs it.
Use --from-source inside a cloned repo to build with Cargo instead.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --channel) channel="${2:-}"; shift 2 ;;
    --channel=*) channel="${1#*=}"; shift ;;
    --target) target="${2:-}"; shift 2 ;;
    --target=*) target="${1#*=}"; shift ;;
    --version) version="${2:-}"; shift 2 ;;
    --version=*) version="${1#*=}"; shift ;;
    --repository) repository="${2:-}"; shift 2 ;;
    --repository=*) repository="${1#*=}"; shift ;;
    --no-modify-shell-profile) modify_shell_profile="false"; shift ;;
    --modify-shell-profile) shift ;; # backwards compat; now default
    --from-source) from_source="true"; shift ;;
    --scope|--project-root)
      printf 'wf-core install is global-only; %s is not supported.\n' "$1" >&2
      exit 1
      ;;
    --scope=*|--project-root=*)
      printf 'wf-core install is global-only; %s is not supported.\n' "$1" >&2
      exit 1
      ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
  esac
done

need_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'wf-core installer requires %s\n' "$1" >&2
    exit 1
  fi
}

normalize_tag() {
  case "$1" in
    v*|bootstrap-*) printf '%s\n' "$1" ;;
    [0-9]*) printf 'v%s\n' "$1" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

asset_version_from_tag() {
  case "$1" in
    v[0-9]*) printf '%s\n' "${1#v}" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

detect_os() {
  case "$(uname -s)" in
    Darwin) printf 'darwin\n' ;;
    Linux) printf 'linux\n' ;;
    MINGW*|MSYS*|CYGWIN*) printf 'windows\n' ;;
    *) printf 'Unsupported operating system: %s\n' "$(uname -s)" >&2; exit 1 ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) printf 'amd64\n' ;;
    arm64|aarch64) printf 'arm64\n' ;;
    *) printf 'Unsupported architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
  esac
}

latest_release_tag() {
  curl -fsSL \
    -H 'Accept: application/vnd.github+json' \
    -H 'User-Agent: wf-core-installer' \
    "https://api.github.com/repos/${repository}/releases/latest" |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -n 1
}

activation_channel() {
  case "$channel" in
    stable) printf 'stable\n' ;;
    insiders) printf 'insiders\n' ;;
    next|both) printf 'next\n' ;;
    *) printf 'next\n' ;;
  esac
}

installed_binary_path() {
  local selected_channel="$1"
  local user_home="${HOME}"
  local appdata_home="${APPDATA:-}"
  if command -v cygpath >/dev/null 2>&1; then
    [ -n "${USERPROFILE:-}" ] && user_home="$(cygpath -u "$USERPROFILE")"
    [ -n "${APPDATA:-}" ] && appdata_home="$(cygpath -u "$APPDATA")"
  fi
  if [ "$target" = "devin" ]; then
    if [ -n "$appdata_home" ]; then
      printf '%s/devin/wf-core/wf-core.exe\n' "$appdata_home"
    else
      printf '%s/.config/devin/wf-core/wf-core\n' "$HOME"
    fi
    return 0
  fi
  case "$selected_channel" in
    stable) printf '%s/.codeium/windsurf/wf-core/wf-core%s\n' "$user_home" "$([ -n "$appdata_home" ] && printf .exe)" ;;
    insiders) printf '%s/.codeium/windsurf-insiders/wf-core/wf-core%s\n' "$user_home" "$([ -n "$appdata_home" ] && printf .exe)" ;;
    *) printf '%s/.codeium/windsurf-next/wf-core/wf-core%s\n' "$user_home" "$([ -n "$appdata_home" ] && printf .exe)" ;;
  esac
}

install_from_source() {
  local script_dir cargo_bin binary
  script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
  if [ -n "${CARGO:-}" ]; then
    cargo_bin="$CARGO"
  elif command -v cargo >/dev/null 2>&1; then
    cargo_bin="cargo"
  else
    printf 'wf-core source install requires cargo on PATH. Omit --from-source to use release assets.\n' >&2
    exit 1
  fi
  "$cargo_bin" build --release --locked --manifest-path "$script_dir/Cargo.toml"
  binary="$script_dir/target/release/wf-core"
  if [ -x "$script_dir/target/release/wf-core.exe" ]; then
    binary="$script_dir/target/release/wf-core.exe"
  fi
  run_install "$binary" "$script_dir"
}

install_from_release() {
  need_command curl
  need_command mktemp

  local os arch release_tag asset_version archive_name archive_ext download_url temporary_directory archive_path extract_directory installer_binary bundle_root
  os="$(detect_os)"
  arch="$(detect_arch)"
  archive_ext="tar.gz"
  if [ "$os" = "windows" ]; then
    need_command powershell
    archive_ext="zip"
  else
    need_command tar
  fi
  if [ "$version" = "latest" ]; then
    release_tag="$(latest_release_tag)"
    if [ -z "$release_tag" ]; then
      printf 'Unable to resolve latest wf-core release for %s\n' "$repository" >&2
      exit 1
    fi
  else
    release_tag="$(normalize_tag "$version")"
  fi
  asset_version="$(asset_version_from_tag "$release_tag")"
  archive_name="wf-core_${asset_version}_${os}_${arch}.${archive_ext}"
  download_url="https://github.com/${repository}/releases/download/${release_tag}/${archive_name}"
  temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/wf-core-install.XXXXXX")"
  cleanup() { rm -rf "$temporary_directory"; }
  trap cleanup EXIT

  archive_path="${temporary_directory}/${archive_name}"
  extract_directory="${temporary_directory}/extract"
  mkdir -p "$extract_directory"

  printf 'Downloading wf-core %s for %s-%s...\n' "$release_tag" "$os" "$arch"
  curl -fL --retry 3 --retry-delay 2 -o "$archive_path" "$download_url"
  if [ "$os" = "windows" ]; then
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath '$archive_path' -DestinationPath '$extract_directory' -Force"
  else
    tar -xzf "$archive_path" -C "$extract_directory"
  fi

  installer_binary="${extract_directory}/wf-core"
  if [ "$os" = "windows" ]; then
    installer_binary="${extract_directory}/wf-core.exe"
  fi
  if [ ! -x "$installer_binary" ]; then
    if [ "$os" = "windows" ]; then
      installer_binary="$(find "$extract_directory" -type f -name wf-core.exe | head -n 1)"
    else
      installer_binary="$(find "$extract_directory" -type f -name wf-core -perm /111 | head -n 1)"
    fi
  fi
  if [ -z "$installer_binary" ] || [ ! -x "$installer_binary" ]; then
    printf 'Release archive did not contain an executable wf-core binary.\n' >&2
    exit 1
  fi

  bundle_root="$(cd "$(dirname "$installer_binary")" && pwd)"
  run_install "$installer_binary" "$bundle_root"
}

run_install() {
  local binary source_root selected_channel activation_binary
  binary="$1"
  source_root="$2"
  selected_channel="$(activation_channel)"
  activation_binary="$(installed_binary_path "$selected_channel")"

  "$binary" install --target "$target" --channel "$channel" --source-root "$source_root"
  "$binary" verify --target "$target" --channel "$channel"
  "$binary" doctor --proxy --target "$target" --channel "$channel" || true

  printf '\nwf-core installed. Activate proxy mode:\n'
  printf '  eval "$(%s shell init --channel %s)"\n' "$activation_binary" "$selected_channel"
  printf '  %s doctor --proxy --channel %s\n' "$activation_binary" "$selected_channel"

  if [ "$modify_shell_profile" = "true" ]; then
    update_shell_profile "$activation_binary" "$selected_channel"
  else
    printf 'Shell profile not modified. Omit --no-modify-shell-profile to modify it.\n'
  fi
}

update_shell_profile() {
  local activation_binary selected_channel profile backup backup_created managed_block tmp_profile
  activation_binary="$1"
  selected_channel="$2"
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
    printf 'eval "$(%s shell init --channel %s)"\n' "$activation_binary" "$selected_channel"
    printf '# wf-core managed:end\n'
  } > "$managed_block"
  if [ -f "$profile" ] && grep -q '# wf-core managed:start' "$profile" && grep -q '# wf-core managed:end' "$profile"; then
    tmp_profile="${profile}.wf-core.tmp.$$"
    awk -v block_file="$managed_block" '
      BEGIN { while ((getline line < block_file) > 0) block = block line ORS }
      /# wf-core managed:start/ { printf "%s", block; in_block = 1; next }
      /# wf-core managed:end/ && in_block { in_block = 0; next }
      !in_block { print }
    ' "$profile" > "$tmp_profile"
    mv "$tmp_profile" "$profile"
  else
    { [ -f "$profile" ] && printf '\n'; cat "$managed_block"; } >> "$profile"
  fi
  rm -f "$managed_block"
  if [ "$backup_created" = "true" ]; then
    printf 'Updated %s (backup: %s)\n' "$profile" "$backup"
  else
    printf 'Updated %s (no previous file to back up)\n' "$profile"
  fi
}

if [ "$from_source" = "true" ]; then
  install_from_source
else
  install_from_release
fi
