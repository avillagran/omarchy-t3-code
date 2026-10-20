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

source_build=${OT3_BUILD_FROM_SOURCE:-0}
packages=(ttf-firacode-nerd git curl ripgrep jq libsecret sqlite openssh wl-clipboard librsvg qt6-declarative qt6-webengine)
if [[ $source_build == 1 ]]; then packages+=(base-devel pkgconf nodejs python); fi
if [[ $source_build == 1 ]] && ! cargo --version >/dev/null 2>&1 && ! command -v rustup >/dev/null; then packages+=(rust); fi
missing=()
for package in "${packages[@]}"; do pacman -Q "$package" >/dev/null 2>&1 || missing+=("$package"); done
if ((${#missing[@]})); then
  printf '%s\n' 'Installing runtime dependencies (sudo may request your password).'
  sudo pacman -S --needed --noconfirm "${missing[@]}"
fi
if [[ $source_build == 1 ]] && ! cargo --version >/dev/null 2>&1; then rustup default stable; fi

release_ref=main
if [[ $source_build != 1 ]]; then
  arch=$(uname -m)
  case "$arch" in aarch64|x86_64) ;; *) fail "No prebuilt package for $arch. Use OT3_BUILD_FROM_SOURCE=1.";; esac
  release_json=$(curl -fsSL https://api.github.com/repos/avillagran/omarchy-t3-code/releases/latest)
  release_ref=$(jq -er '.tag_name | select(test("^v[0-9][A-Za-z0-9._-]*$"))' <<< "$release_json") || fail 'No published binary release available. Use OT3_BUILD_FROM_SOURCE=1 to build main.'
fi

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
  git -C "$plugin_dir" fetch origin "$release_ref"
  expected_commit=$(git -C "$plugin_dir" rev-parse FETCH_HEAD)
  # Verify the package before changing an existing installation.
  if ! git -C "$plugin_dir" merge-base --is-ancestor HEAD "$expected_commit"; then
    # Commit messages were rewritten once (trees unchanged). Move a clean checkout to the
    # rewritten commit with the identical tree; anything else really diverges.
    head_tree=$(git -C "$plugin_dir" rev-parse 'HEAD^{tree}')
    same_commit=$(git -C "$plugin_dir" log --format='%H %T' "$expected_commit" | awk -v tree="$head_tree" '$2 == tree { print $1; exit }')
    [[ -n "$same_commit" ]] || fail 'Installed source is ahead of or diverges from this release. Use OT3_BUILD_FROM_SOURCE=1 for main.'
    git -C "$plugin_dir" reset -q --hard "$same_commit"
  fi
  build_dir=$plugin_dir
else
  stage=$(mktemp -d "$plugins_dir/.t3-install.XXXXXXXX")
  git clone --branch "$release_ref" -- https://github.com/avillagran/omarchy-t3-code.git "$stage"
  build_dir=$stage
  expected_commit=$(git -C "$build_dir" rev-parse HEAD)
fi
if [[ $source_build == 1 ]]; then
  if [[ -z "$stage" ]]; then
    omarchy-shell "$plugin_id" closePanel >/dev/null 2>&1 || true
    git -C "$plugin_dir" merge --ff-only "$expected_commit"
  fi
  "$build_dir/bin/build-client"
else
  downloads=$(mktemp -d)
  trap 'rm -rf -- "$downloads"; cleanup' EXIT
  asset="omarchy-t3-code-$release_ref-linux-$arch.tar.gz"
  base_url="https://github.com/avillagran/omarchy-t3-code/releases/download/$release_ref"
  curl -fsSL "$base_url/$asset" -o "$downloads/$asset"
  curl -fsSL "$base_url/$asset.sha256" -o "$downloads/$asset.sha256"
  (cd "$downloads" && sha256sum -c "$asset.sha256") || fail 'Binary checksum failed.'
  [[ $(tar -xOf "$downloads/$asset" COMMIT) == "$expected_commit" ]] || fail 'Binary and source revisions differ.'
  mkdir "$downloads/unpacked"
  tar -xzf "$downloads/$asset" -C "$downloads/unpacked" bin/omarchy-t3-client bin/libt3-document.so bin/libt3-qml-preview.so bin/libt3-browser.so bin/t3-browser-secret
  ldd "$downloads/unpacked/bin/omarchy-t3-client" "$downloads/unpacked/bin/libt3-document.so" "$downloads/unpacked/bin/libt3-qml-preview.so" "$downloads/unpacked/bin/libt3-browser.so" "$downloads/unpacked/bin/t3-browser-secret" > "$downloads/libraries"
  if rg -q 'not found' "$downloads/libraries"; then
    cat "$downloads/libraries" >&2
    fail 'Runtime libraries do not match this package; update Omarchy or use OT3_BUILD_FROM_SOURCE=1.'
  fi
  if [[ -z "$stage" ]]; then
    omarchy-shell "$plugin_id" closePanel >/dev/null 2>&1 || true
    git -C "$plugin_dir" merge --ff-only "$expected_commit"
  fi
  install -m755 "$downloads/unpacked/bin/omarchy-t3-client" "$build_dir/bin/omarchy-t3-client.new"
  mv -f "$build_dir/bin/omarchy-t3-client.new" "$build_dir/bin/omarchy-t3-client"
  for file in libt3-document.so libt3-qml-preview.so libt3-browser.so t3-browser-secret; do
    install -m755 "$downloads/unpacked/bin/$file" "$build_dir/bin/$file"
  done
fi
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
