# host_sync: place what ot3 sends. stdin is a tar archive with `stamp`, `plan`
# and `files/<path>`. Plan lines, fields separated by \037:
#   G <path>                             a skill folder; one that became a link is kept
#   F <path> <expected>                  write files/<path> if the host still has <expected>
#   V <path> <url> <mode> <origin> <name> <email>  clone or pull the vault
# What is replaced is first copied to ~/.local/state/omarchy-t3-code/sync-backups/<stamp>.
# Answers one line per plan line: F <path> ok|changed|kept|failed, V <path> <result>.
sep=$(printf '\037')
real_home=$(cd "$HOME" 2>/dev/null && pwd -P) || exit 5
stage=$(mktemp -d "${TMPDIR:-/tmp}/ot3-sync.XXXXXX") || exit 6
trap 'rm -rf -- "$stage"' EXIT
tar -xpf - -C "$stage" 2>/dev/null || exit 3
stamp=$(cat "$stage/stamp")
case "$stamp" in ''|*[!0-9TZ-]*) exit 4;; esac
backup="$HOME/.local/state/omarchy-t3-code/sync-backups/$stamp"
sum() { if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi; }
inside() { case "$1" in "$real_home"/*) return 0;; *) return 1;; esac; }
state() {
  f="$HOME/$1"
  if [ -L "$f" ] || [ -e "$f" ]; then
    r=$(readlink -f -- "$f" 2>/dev/null) || { echo other; return; }
    inside "$r" || { echo system; return; }
    if [ -d "$f" ]; then echo dir; elif [ -f "$f" ]; then sum < "$f"; else echo other; fi
  else
    d=$(dirname -- "$f"); while [ ! -e "$d" ] && [ "$d" != / ]; do d=$(dirname -- "$d"); done
    r=$(cd "$d" 2>/dev/null && pwd -P) || { echo other; return; }
    if inside "$r/"; then echo absent; else echo system; fi
  fi
}
kept=""
while IFS="$sep" read -r kind path a b c d e; do
  case "$path" in ''|/*|..|../*|*/../*|*/..) continue;; esac
  case "$kind" in
    G) if [ -L "$HOME/$path" ]; then kept="$path/"; else kept=""; fi;;
    F) if [ -n "$kept" ]; then case "$path" in "$kept"*) printf 'F%s%s%skept\n' "$sep" "$path" "$sep"; continue;; esac; fi
       now=$(state "$path")
       if [ "$now" = system ]; then printf 'F%s%s%skept\n' "$sep" "$path" "$sep"; continue; fi
       if [ "$now" != "$a" ]; then printf 'F%s%s%schanged\n' "$sep" "$path" "$sep"; continue; fi
       f="$HOME/$path"; target="$f"
       if [ -L "$f" ]; then target=$(readlink -f -- "$f"); fi
       if [ "$a" != absent ]; then
         mkdir -p -- "$(dirname -- "$backup/$path")" && cp -p -- "$target" "$backup/$path" || { printf 'F%s%s%sfailed\n' "$sep" "$path" "$sep"; continue; }
       fi
       tmp="$target.ot3-sync.$$"
       if mkdir -p -- "$(dirname -- "$target")" && cp -p -- "$stage/files/$path" "$tmp" && mv -f -- "$tmp" "$target"; then
         printf 'F%s%s%sok\n' "$sep" "$path" "$sep"
       else rm -f -- "$tmp"; printf 'F%s%s%sfailed\n' "$sep" "$path" "$sep"; fi;;
    V) dir="$HOME/$path"; url="$a"; mode="$b"; r=failed
       if ! command -v git >/dev/null 2>&1; then r=nogit
       elif [ "$mode" = clone ]; then
         if [ ! -e "$dir" ] || { [ -d "$dir" ] && [ -z "$(ls -A -- "$dir" 2>/dev/null)" ]; }; then
           mkdir -p -- "$(dirname -- "$dir")" && GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND='ssh -o BatchMode=yes' git clone -q -- "$url" "$dir" >/dev/null 2>&1 && r=cloned
         else r=occupied; fi
       elif [ "$mode" = pull ]; then
         origin=$(git -C "$dir" config --get remote.origin.url 2>/dev/null | sed 's#//[^/@]*@#//#')
         if [ "$origin" != "$c" ]; then r=occupied
         elif GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND='ssh -o BatchMode=yes' git -C "$dir" pull -q --rebase --autostash >/dev/null 2>&1; then r=pulled
         else
           if [ -d "$dir/.git/rebase-merge" ] || [ -d "$dir/.git/rebase-apply" ]; then git -C "$dir" rebase --abort >/dev/null 2>&1; r=conflict; fi
         fi
       fi
       # A clone needs a commit identity; only one the vault there lacks is set.
       if [ "$r" = cloned ] || [ "$r" = pulled ]; then
         [ -n "$d" ] && [ -z "$(git -C "$dir" config --local user.name)" ] && git -C "$dir" config --local user.name "$d"
         [ -n "$e" ] && [ -z "$(git -C "$dir" config --local user.email)" ] && git -C "$dir" config --local user.email "$e"
       fi
       printf 'V%s%s%s%s\n' "$sep" "$path" "$sep" "$r";;
  esac
done < "$stage/plan"
[ -d "$backup" ] && printf 'B%s%s\n' "$sep" "$backup"
exit 0
