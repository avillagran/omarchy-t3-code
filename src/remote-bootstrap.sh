set -eu
base="$HOME/.t3"
actual=$(cat "$base/userdata/environment-id")
[ "$actual" = "$1" ] || exit 7
cli="$HOME/.t3/bin/t3"
if [ ! -x "$cli" ]; then cli="$HOME/.local/bin/t3"; fi
if [ ! -x "$cli" ]; then cli=$(command -v t3) || exit 127; fi
runtime="$base/userdata/server-runtime.json"
info=$("$cli" __ssh-helper runtime-port "$runtime" 2>/dev/null || true)
if [ -z "$info" ]; then
  unit="$HOME/.config/systemd/user/t3code.service"
  if [ -f "$unit" ]; then
    grep -Fx "Environment=T3CODE_HOME=$base" "$unit" >/dev/null || grep -Fx "Environment=T3CODE_HOME=\"$base\"" "$unit" >/dev/null || exit 8
    systemctl --user start t3code.service >/dev/null || exit 9
  else
    "$cli" service install --base-dir "$base" >/dev/null || exit 9
  fi
  attempts=0
  while [ "$attempts" -lt 30 ]; do
    info=$("$cli" __ssh-helper runtime-port "$runtime" 2>/dev/null || true)
    [ -z "$info" ] || break
    attempts=$((attempts + 1))
    sleep 0.25
  done
fi
[ -n "$info" ] || exit 9
port=${info#* }
"$cli" __ssh-helper wait-ready "$port" 15000 1000 >/dev/null || exit 9
cat "$runtime"
printf '\nT3_AUTH\n'
"$cli" auth session issue --base-dir "$base" --scope orchestration:read --scope orchestration:operate --scope filesystem:read --scope settings:write --scope terminal:read --scope terminal:operate --ttl 1d --label omarchy-t3-code --token-only
