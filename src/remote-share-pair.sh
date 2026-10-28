set -eu
# One-use T3 pairing for an OmaBridge peer this host shares T3 with (omabridge_share.rs).
base="$HOME/.t3"
actual=$(cat "$base/userdata/environment-id")
[ "$actual" = "$1" ] || exit 7
cli="$HOME/.t3/bin/t3"
if [ ! -x "$cli" ]; then cli="$HOME/.local/bin/t3"; fi
if [ ! -x "$cli" ]; then cli=$(command -v t3) || exit 127; fi
exec "$cli" auth pairing create --base-dir "$base" --ttl 2m --label "$2" --json
