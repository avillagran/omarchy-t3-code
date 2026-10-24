set -eu
# Update this host's T3 server with T3's own `t3 update`, then wait until the
# restarted service publishes its port again. Runs locally or over SSH.
#   $1  the environment id ot3 is updating (refuses any other profile)
#   $2  the exact version to install
# Exit codes as remote-restart.sh: 7 wrong environment, 8 the service runs
# another profile, 9 the server did not come back, 127 no T3 CLI; and
# 10 `t3 update` failed, 11 no background service to switch over, 12 another
# update is running on this host.
base="$HOME/.t3"
actual=$(cat "$base/userdata/environment-id")
[ "$actual" = "$1" ] || exit 7
unit="$HOME/.config/systemd/user/t3code.service"
[ -f "$unit" ] || exit 11
grep -Fx "Environment=T3CODE_HOME=$base" "$unit" >/dev/null || grep -Fx "Environment=T3CODE_HOME=\"$base\"" "$unit" >/dev/null || exit 8
# One update at a time on this host, from any ot3. A lock older than 20
# minutes belongs to an update that can no longer be running.
lock="$base/userdata/ot3-update.lock"
if ! mkdir "$lock" 2>/dev/null; then
  [ -n "$(find "$lock" -maxdepth 0 -mmin +20 2>/dev/null)" ] || exit 12
  rm -rf "$lock"; mkdir "$lock" || exit 12
fi
trap 'rm -rf "$lock"' EXIT
# The runtime the service runs knows its own version; a desktop app's wrapper
# would report the app's bundled one instead.
service_cli() { sed -n 's/^ExecStart=\(\/.*\/t3\) __service-launcher$/\1/p' "$unit" | head -n 1; }
cli=$(service_cli)
if [ -z "$cli" ] || [ ! -x "$cli" ]; then cli="$HOME/.local/bin/t3"; fi
if [ ! -x "$cli" ]; then cli="$HOME/.t3/bin/t3"; fi
if [ ! -x "$cli" ]; then cli=$(command -v t3) || exit 127; fi
"$cli" update --yes --base-dir "$base" "$2" >/dev/null 2>&1 || exit 10
cli=$(service_cli)
[ -n "$cli" ] && [ -x "$cli" ] || exit 9
runtime="$base/userdata/server-runtime.json"
attempts=0
while [ "$attempts" -lt 240 ]; do
  info=$("$cli" __ssh-helper runtime-port "$runtime" 2>/dev/null || true)
  if [ -n "$info" ] && "$cli" __ssh-helper wait-ready "${info#* }" 2000 500 >/dev/null 2>&1; then
    echo "updated $2"
    exit 0
  fi
  attempts=$((attempts + 1))
  sleep 0.25
done
exit 9
