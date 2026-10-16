#!/usr/bin/env bash
# Install/update the Omarchy plugin; reuse T3's existing profile and connections.
set -euo pipefail
fail() { printf 'omarchy-t3-code: %s\n' "$*" >&2; exit 1; }
[[ $EUID -ne 0 ]] || fail 'Run as your desktop user, not root.'
command -v omarchy >/dev/null || fail 'Omarchy with the Quickshell plugin manager is required.'
command -v omarchy-shell >/dev/null || fail 'The Omarchy shell must be installed and running.'
command -v pacman >/dev/null || fail 'This installer targets Omarchy on Arch Linux.'
t3_base=${T3CODE_HOME:-$HOME/.t3}
t3_base=${t3_base/#\~/$HOME}
[[ -d "$t3_base/userdata" && -x "$HOME/.t3/bin/t3" ]] || fail 'Install and open T3 Code once before installing this plugin: https://t3.codes'

packages=(ttf-firacode-nerd git ripgrep base-devel pkgconf jq libsecret openssh wl-clipboard qt6-declarative qt6-webengine)
if ! cargo --version >/dev/null 2>&1 && ! command -v rustup >/dev/null; then packages+=(rust); fi
missing=()
for package in "${packages[@]}"; do pacman -Q "$package" >/dev/null 2>&1 || missing+=("$package"); done
if ((${#missing[@]})); then
  printf '%s\n' 'Installing build, clipboard and HTML-preview dependencies (sudo may request your password).'
  sudo pacman -S --needed --noconfirm "${missing[@]}"
fi
if ! cargo --version >/dev/null 2>&1; then rustup default stable; fi

plugin_id=io.github.avillagran.omarchy-t3-code
plugins_dir=$HOME/.config/omarchy/plugins
plugin_dir=$plugins_dir/omarchy-t3-code
canonical=$plugins_dir/$plugin_id
[[ ! -e "$canonical" || ! -e "$plugin_dir" ]] || fail 'Two plugin copies exist; keep one installation before continuing.'
if [[ -d "$canonical" ]]; then plugin_dir=$canonical; fi
stage=''
cleanup() { if [[ -n "$stage" && -d "$stage" ]]; then rm -rf -- "$stage"; fi; }
trap cleanup EXIT
mkdir -p "$plugins_dir"
if [[ -e "$plugin_dir" ]]; then
  [[ -d "$plugin_dir/.git" ]] || fail "Existing directory is not a Git checkout: $plugin_dir"
  origin=$(git -C "$plugin_dir" remote get-url origin)
  case "$origin" in https://github.com/avillagran/omarchy-t3-code.git|git@github.com:avillagran/omarchy-t3-code.git) ;; *) fail 'Existing checkout has a different origin; refusing to update it.';; esac
  [[ -z $(git -C "$plugin_dir" status --porcelain) ]] || fail 'Existing checkout has local changes; commit or stash them before updating.'
  omarchy-shell "$plugin_id" closePanel >/dev/null 2>&1 || true
  git -C "$plugin_dir" pull --ff-only origin main
  build_dir=$plugin_dir
else
  stage=$(mktemp -d "$plugins_dir/.t3-install.XXXXXXXX")
  git clone -- https://github.com/avillagran/omarchy-t3-code.git "$stage"
  build_dir=$stage
fi
"$build_dir/bin/build-client"
[[ -f "$build_dir/bin/libt3-document.so" ]] || fail 'The HTML preview did not build; check Qt development packages and retry.'
omarchy plugin validate "$build_dir"
if [[ -n "$stage" ]]; then mv -- "$stage" "$plugin_dir"; stage=''; fi

# Append the owned integration: replace only the agent shortcut; send stays inside the app.
bindings=$HOME/.config/hypr/bindings.lua
if [[ -f "$bindings" ]] && ! rg -q '(omarchy-t3-code|io.github.avillagran.omarchy-t3-code)/(integrations/hyprland/bindings|hypr-bindings)\.lua' "$bindings"; then
  cp -p -- "$bindings" "$bindings.t3-backup-$(date +%Y%m%d%H%M%S)"
  lua_path=$(printf '%s' "$plugin_dir/integrations/hyprland/bindings.lua" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '\n-- Omarchy T3 Code: Super+Shift+A agent; Ctrl+Enter sends inside ot3.\ndofile("%s")\n' "$lua_path" >> "$bindings"
elif [[ ! -f "$bindings" ]]; then
  printf '%s\n' 'Lua bindings.lua not found; use the Send button. See docs/DEVELOPMENT.md for agent/send shortcuts.'
fi
# Updates can remove an old binding from the already-sourced plugin module.
if [[ -f "$bindings" ]]; then
  hyprctl reload >/dev/null
  binding_errors=$(hyprctl configerrors)
  [[ -z ${binding_errors//[[:space:]]/} ]] || fail "Hyprland configuration errors: $binding_errors"
fi
status=$("$plugin_dir/bin/omarchy-t3-client" backend-status)
if [[ $(jq -r '.owner' <<< "$status") == stopped ]]; then
  result=$("$plugin_dir/bin/omarchy-t3-client" backend-control start)
  [[ $(jq -r '.error // empty' <<< "$result") == '' ]] || fail "Could not start T3's local backend: $(jq -r '.error' <<< "$result")"
fi
omarchy-shell shell rescanPlugins >/dev/null
omarchy plugin enable "$plugin_id"
omarchy restart shell
printf '%s\n' 'Installed. Open T3 Code from the status bar. Local/remotes and model favorites are inherited from T3.'
