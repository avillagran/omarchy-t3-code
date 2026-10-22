set -eu
# Restart a remote T3 server that no longer answers: kill it outright, start
# it the normal way, and wait until it publishes its port again.
#   $1  the environment id the user confirmed (refuses any other profile)
# Exit codes as remote-bootstrap.sh: 7 wrong environment, 8 the service runs
# another profile, 9 the server did not come back, 127 no T3 CLI.
base="$HOME/.t3"
actual=$(cat "$base/userdata/environment-id")
[ "$actual" = "$1" ] || exit 7
cli="$HOME/.t3/bin/t3"
if [ ! -x "$cli" ]; then cli="$HOME/.local/bin/t3"; fi
if [ ! -x "$cli" ]; then cli=$(command -v t3) || exit 127; fi
runtime="$base/userdata/server-runtime.json"
unit="$HOME/.config/systemd/user/t3code.service"
if [ -f "$unit" ]; then
  grep -Fx "Environment=T3CODE_HOME=$base" "$unit" >/dev/null || grep -Fx "Environment=T3CODE_HOME=\"$base\"" "$unit" >/dev/null || exit 8
  # Every process of the service (launcher, server, its children) at once.
  systemctl --user kill --signal=KILL t3code.service >/dev/null 2>&1 || true
  systemctl --user reset-failed t3code.service >/dev/null 2>&1 || true
  rm -f "$runtime"
  systemctl --user restart t3code.service >/dev/null || exit 9
else
  # Not a service: this user's servers for this profile, as T3 Desktop's
  # launcher and the runtime file record them.
  pids=""
  for file in "$base"/ssh-launch/*/pid; do [ -f "$file" ] && pids="$pids $(cat "$file" 2>/dev/null || true)"; done
  if [ -f "$runtime" ]; then pids="$pids $(sed -n 's/.*"pid"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' "$runtime")"; fi
  for pid in $pids; do
    case "$pid" in ''|*[!0-9]*) continue ;; esac
    # Only a T3 process owned by this user.
    if [ -r "/proc/$pid/cmdline" ] && [ "$(stat -c %u "/proc/$pid")" = "$(id -u)" ] && tr '\0' ' ' < "/proc/$pid/cmdline" | grep -Eq '/t3 (serve|__service-launcher)'; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
  done
  rm -f "$runtime"
  "$cli" service install --base-dir "$base" >/dev/null || exit 9
fi
attempts=0
while [ "$attempts" -lt 240 ]; do
  info=$("$cli" __ssh-helper runtime-port "$runtime" 2>/dev/null || true)
  if [ -n "$info" ] && "$cli" __ssh-helper wait-ready "${info#* }" 2000 500 >/dev/null 2>&1; then
    echo "restarted ${info#* }"
    exit 0
  fi
  attempts=$((attempts + 1))
  sleep 0.25
done
exit 9
