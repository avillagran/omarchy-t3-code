#!/usr/bin/env bash
# Install/update O3 Code: the desktop app (bin/o3, its own process) and
# its optional bar widget; reuse T3's existing profile and connections.
set -euo pipefail
# The brace group is parsed whole before it runs, so `curl | bash` never executes a partial download.
{
fail() { printf 'omarchy-t3-code: %s\n' "$*" >&2; exit 1; }
# set -e would otherwise stop silently; name the step that failed.
trap 'printf "omarchy-t3-code: step failed (line %s): %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR
[[ $EUID -ne 0 ]] || fail 'Run as your desktop user, not root.'
command -v omarchy >/dev/null || fail 'Omarchy with the Quickshell plugin manager is required.'
command -v omarchy-shell >/dev/null || fail 'The Omarchy shell must be installed and running.'
command -v pacman >/dev/null || fail 'This installer targets Omarchy on Arch Linux.'
t3_base=${T3CODE_HOME:-$HOME/.t3}
t3_base=${t3_base/#\~/$HOME}

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

# ot3 is a client of T3 Code nightly: install it if absent, then open it once so it
# writes its profile and the ~/.t3/bin/t3 CLI that ot3 drives.
t3_package=t3code-nightly-bin
t3_bootstrapped=0
if [[ ! -x "$HOME/.t3/bin/t3" ]] && ! command -v t3code-nightly >/dev/null && ! command -v t3code >/dev/null; then
  printf 'Installing T3 Code nightly (%s).\n' "$t3_package"
  if pacman -Si "$t3_package" >/dev/null 2>&1; then sudo pacman -S --needed --noconfirm "$t3_package"
  elif command -v yay >/dev/null; then yay -S --needed --noconfirm "$t3_package"
  elif command -v paru >/dev/null; then paru -S --needed --noconfirm "$t3_package"
  else fail "Install T3 Code nightly ($t3_package) and run this installer again: https://t3.codes"
  fi
fi
if [[ ! -d "$t3_base/userdata" || ! -x "$HOME/.t3/bin/t3" ]]; then
  [[ -n ${WAYLAND_DISPLAY:-} ]] || fail 'Open T3 Code once from your desktop session, then run this installer again.'
  t3_app=$(command -v t3code-nightly || command -v t3code) || fail 'T3 Code is not on PATH; open it once, then run this installer again.'
  printf '%s\n' 'Opening T3 Code once to create its profile.'
  t3_bootstrapped=1
  # Its own unit, so this bootstrap window closes again afterwards: the backend then
  # runs as T3's native service and does not depend on a T3 window staying open.
  t3_unit=''
  if systemctl --user show-environment >/dev/null 2>&1 && systemd-run --user --quiet --collect --unit=ot3-t3-first-open -- "$t3_app" >/dev/null 2>&1; then
    t3_unit=ot3-t3-first-open
  else
    setsid -f "$t3_app" >/dev/null 2>&1 </dev/null
  fi
  for _ in {1..90}; do [[ -f "$t3_base/userdata/server-runtime.json" && -x "$HOME/.t3/bin/t3" ]] && break; sleep 1; done
  [[ -d "$t3_base/userdata" && -x "$HOME/.t3/bin/t3" ]] || fail 'T3 Code did not finish its first start; open it once, then run this installer again.'
  if [[ -n $t3_unit ]]; then
    systemctl --user stop "$t3_unit" >/dev/null 2>&1 || true
    for _ in {1..20}; do [[ -f "$t3_base/userdata/server-runtime.json" ]] || break; sleep 0.5; done
  fi
fi

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
  expected_commit=$(git -C "$plugin_dir" rev-parse 'FETCH_HEAD^{commit}')
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
    quickshell ipc -p "${XDG_STATE_HOME:-$HOME/.local/state}/omarchy-t3-code/app" call "$plugin_id" quit >/dev/null 2>&1 || true
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
  # The dictation helper is optional: packages built without CMake lack it.
  tar -xzf "$downloads/$asset" -C "$downloads/unpacked" bin/ot3-dictate 2>/dev/null || true
  ldd "$downloads/unpacked/bin/omarchy-t3-client" "$downloads/unpacked/bin/libt3-document.so" "$downloads/unpacked/bin/libt3-qml-preview.so" "$downloads/unpacked/bin/libt3-browser.so" "$downloads/unpacked/bin/t3-browser-secret" $([[ -f $downloads/unpacked/bin/ot3-dictate ]] && printf '%s' "$downloads/unpacked/bin/ot3-dictate") > "$downloads/libraries"
  if rg -q 'not found' "$downloads/libraries"; then
    cat "$downloads/libraries" >&2
    fail 'Runtime libraries do not match this package; update Omarchy or use OT3_BUILD_FROM_SOURCE=1.'
  fi
  if [[ -z "$stage" ]]; then
    quickshell ipc -p "${XDG_STATE_HOME:-$HOME/.local/state}/omarchy-t3-code/app" call "$plugin_id" quit >/dev/null 2>&1 || true
    omarchy-shell "$plugin_id" closePanel >/dev/null 2>&1 || true
    git -C "$plugin_dir" merge --ff-only "$expected_commit"
  fi
  install -m755 "$downloads/unpacked/bin/omarchy-t3-client" "$build_dir/bin/omarchy-t3-client.new"
  mv -f "$build_dir/bin/omarchy-t3-client.new" "$build_dir/bin/omarchy-t3-client"
  for file in libt3-document.so libt3-qml-preview.so libt3-browser.so t3-browser-secret; do
    install -m755 "$downloads/unpacked/bin/$file" "$build_dir/bin/$file"
  done
  if [[ -f $downloads/unpacked/bin/ot3-dictate ]]; then install -m755 "$downloads/unpacked/bin/ot3-dictate" "$build_dir/bin/ot3-dictate"; fi
fi
# OmaBridge is bundled but optional: without it, connections keep their saved routes.
if [[ -x "$build_dir/bin/fetch-omabridge" ]]; then
  "$build_dir/bin/fetch-omabridge" || printf '%s\n' 'OmaBridge was not installed; connections use their saved routes. Rerun the installer to retry.' >&2
fi
[[ -f "$build_dir/bin/libt3-document.so" ]] || fail 'The HTML preview did not build; check Qt development packages and retry.'
omarchy plugin validate "$build_dir"
if [[ -n "$stage" ]]; then mv -- "$stage" "$plugin_dir"; stage=''; fi
# A device published with the bundled binary (OmaBridge's own unit) keeps
# running; restart it on the updated binary.
bridge_unit=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/omabridge-serve.service
if [[ -f "$bridge_unit" ]] && grep -qF "$plugin_dir/bin/omabridge" "$bridge_unit" && command -v systemctl >/dev/null; then
  systemctl --user try-restart omabridge-serve.service || printf '%s\n' 'Could not restart the published OmaBridge service.' >&2
fi

# Append the owned integration: replace only the agent shortcut; send stays inside the app.
bindings=$HOME/.config/hypr/bindings.lua
if [[ -f "$bindings" ]] && ! rg -q '(omarchy-t3-code|io.github.avillagran.omarchy-t3-code)/(integrations/hyprland/bindings|hypr-bindings)\.lua' "$bindings"; then
  cp -p -- "$bindings" "$bindings.t3-backup-$(date +%Y%m%d%H%M%S)"
  lua_path=$(printf '%s' "$plugin_dir/integrations/hyprland/bindings.lua" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '\n-- O3 Code: Super+Shift+A agent; Ctrl+Enter sends inside O3.\ndofile("%s")\n' "$lua_path" >> "$bindings"
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
# The app: launcher entry, hidden start at login and `o3` (and the old `ot3`) on the PATH.
"$plugin_dir/bin/o3" install-desktop
"$plugin_dir/bin/o3" restart >/dev/null
# The bar widget is optional: OT3_BAR_WIDGET=0 installs only the desktop app.
if [[ ${OT3_BAR_WIDGET:-1} != 0 ]]; then
  omarchy-shell shell rescanPlugins >/dev/null
  omarchy plugin enable "$plugin_id"
  omarchy restart shell
fi
# The T3 window is optional (ot3 uses T3's service); offer it after a first-time setup.
# stdin is the script under `curl | bash`, so ask on the terminal, and only when there is one.
if [[ $t3_bootstrapped == 1 ]] && { : </dev/tty; } 2>/dev/null; then
  read -r -p 'Open T3 Code now? [y/N] ' answer </dev/tty || answer=''
  if [[ $answer == [yYsS]* ]]; then setsid -f "$t3_app" >/dev/null 2>&1 </dev/null; fi
fi
printf '%s\n' 'Installed. Press Super+Shift+A, open "O3 Code" from the app launcher or click O3 in the bar. Local/remotes and model favorites are inherited from T3.'
exit
}
