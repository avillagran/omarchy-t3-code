# host_sync: read-only look at the host before a sync. stdin carries one request
# per line, fields separated by \037:
#   F <path>          a file to place: absent, dir, other, system or its SHA-256
#   G <path>          a skill folder: absent, dir, link (the host's own) or other
#   J <path>          a JSON file to merge into: absent or "<sha>\037<base64>"
#   V <path> <url>    the vault folder, git access to <url> and the GitHub CLI
# Paths are relative to $HOME and were validated by ot3. Nothing is written.
sep=$(printf '\037')
real_home=$(cd "$HOME" 2>/dev/null && pwd -P) || exit 5
sum() { if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi; }
run() { if command -v timeout >/dev/null 2>&1; then timeout "$@"; else shift; "$@"; fi; }
# Inside this user's home once every link is followed; "system" otherwise.
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
    if inside "$r/" || [ "$r" = "$real_home" ]; then echo absent; else echo system; fi
  fi
}
printf 'H%s%s\n' "$sep" "$HOME"
while IFS="$sep" read -r kind path url; do
  case "$path" in ''|/*|..|../*|*/../*|*/..) printf 'E%s%s\n' "$sep" "$path"; continue;; esac
  case "$kind" in
    F) printf 'F%s%s%s%s\n' "$sep" "$path" "$sep" "$(state "$path")";;
    G) f="$HOME/$path"
       if [ -L "$f" ]; then s=link; elif [ -d "$f" ]; then s=dir; elif [ -e "$f" ]; then s=other; else s=absent; fi
       printf 'G%s%s%s%s\n' "$sep" "$path" "$sep" "$s";;
    J) s=$(state "$path")
       case "$s" in
         absent|dir|other|system) printf 'J%s%s%s%s\n' "$sep" "$path" "$sep" "$s";;
         *) printf 'J%s%s%s%s%s%s\n' "$sep" "$path" "$sep" "$s" "$sep" "$(base64 < "$HOME/$path" | tr -d '\n')";;
       esac;;
    V) d="$HOME/$path"
       if [ ! -e "$d" ] && [ ! -L "$d" ]; then s=missing
       elif [ -d "$d" ] && [ -z "$(ls -A -- "$d" 2>/dev/null)" ]; then s=empty
       elif [ -d "$d/.git" ] && command -v git >/dev/null 2>&1; then s="clone$sep$(git -C "$d" config --get remote.origin.url 2>/dev/null | sed 's#//[^/@]*@#//#')"
       else s=other; fi
       printf 'V%s%s%s%s\n' "$sep" "$path" "$sep" "$s"
       if ! command -v git >/dev/null 2>&1; then a=nogit
       else
         err=$(GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND='ssh -o BatchMode=yes' run 25 git ls-remote -- "$url" HEAD 2>&1 >/dev/null) && a=ok || {
           case "$err" in
             *uthentication*|*'could not read Username'*|*'Permission denied'*|*'not found'*|*403*|*401*) a=auth;;
             *) a=unreachable;;
           esac; }
       fi
       printf 'A%s%s\n' "$sep" "$a"
       if command -v gh >/dev/null 2>&1; then
         if run 15 gh auth status >/dev/null 2>&1; then g=signed-in; else g=signed-out; fi
       else g=missing; fi
       printf 'T%s%s\n' "$sep" "$g";;
  esac
done
