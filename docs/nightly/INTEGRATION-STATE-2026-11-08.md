# Git integration and disk review, 2026-11-08

All inspected worktrees belong to the original project Git stores in
`/home/node/Work/o3-code/.git`, `/home/node/Work/o3-server/.git` and
`/home/node/Work/lib-t3-rust/.git`. The audit covered 22 client, 73 server and
13 SDK worktrees. Nightly 4353 remains an active implementation target;
complete compatibility has not been accepted.

## Integration state

| Repository | Integration state | Remaining work |
| --- | --- | --- |
| Client | `feat/nightly-4247-parity` was at `785c145` during this audit, 12 commits ahead of public `ba6324b` in draft PR #35. Auth draft PR #36 (`eed75bb`) is also integrated locally with 21 byte-identical files. | Combined build and remaining GUI teardown acceptance; publication of the pending GUI-containing branch remains gated. |
| Server | Public `feat/nightly-4247-fresh-context` is `6be0e55`, draft PR #50. The accepted no-tab/negative source work is already integrated. | Native-launch fixture correction, complete present Status and managed-account controller/store work remain separate source candidates. CI at the public head passes default mode and fails minimal on the old JSON publication fixture. |
| SDK | Public `feat/nightly-4340-auth-remove-method` is `64e322be`, draft PR #40, clean. Required native and Android artifact CI passed at this head. | Whole-client/server integration and actual managed-store acceptance remain separate from SDK acceptance. |

The main client checkout `/home/node/Work/o3-code` still selects
`fix/share-t3-o3-label`; the Nightly working checkout is
`/var/tmp/o3-build/o3code-nightly`. A shared Git store makes the branches
available to the project; it does not put every candidate into the selected
working branch or install it into the running application.

## Preserved source checkpoints

Fifteen local refs preserve 291 changed or untracked source files. Every blob
was checked against the original working file. Original HEADs, indexes, file
bytes and identities stayed intact. These refs record work in progress and do
not declare test acceptance or merge it into the integration branch.

| Repository | Worktree | Checkpoint commit | Source files |
| --- | --- | --- | ---: |
| client | `o3code-nightly` | `dd8181a0befd` | 7 |
| client | `o3code-reload` | `246a2aa817f9` | 148 |
| client | `o3code-wayland-context-4314` | `7ec41703a959` | 21 |
| server | `auth-method-4340` | `70f86ce75fdd` | 10 |
| server | `device-adb-heap-check` | `9745587dbb19` | 4 |
| server | `device-stack-cache-check` | `7c58dda473ea` | 2 |
| server | `forge-legacy-redaction-check` | `1b1e773d4e80` | 1 |
| server | `goal-resume-red` | `1d6809e38886` | 2 |
| server | `live-history-red` | `902d27208a8e` | 1 |
| server | `native-browser-status-producers-4314` | `6942798d2843` | 26 |
| server | `native-launch-json-readiness-6be` | `2820f715c955` | 1 |
| server | `native-preview-toolkit-4314` | `22b9d1234292` | 1 |
| server | `speech-4314` | `d3fc6da7cc07` | 24 |
| server | `speech-worker-4314` | `e3a1d9ad3b1f` | 38 |
| server | `status-present-source-4340` | `16ba3ef44686` | 5 |

Each ref is `checkpoint/20261108/<worktree-name>` in its repository. They are
local checkpoints. The exact before/after evidence is retained in
`/var/tmp/o3-build/git-source-checkpoints-djtv5vvf/result.json`; the original
read-only audit is `/var/tmp/o3-build/git-integration-audit-20261108.json`.
No active branch, working tree or source file was removed.

## Disk review

A new inventory identified 39 old, regular, single-link compiler-cache leaves,
with 1,267,052,544 allocated bytes (about 1.18 GiB). They are only
`libo3_server-*.rlib` and `.rmeta` files in three old target directories. The
current client and server targets, SDK/auth caches, executables, runtimes,
source, evidence and previous cleanup targets are excluded. An independent
comparison of eight current manifests and 76,561 absolute input paths found
no candidate path/inode overlap; both source owners confirmed this separately.

The reviewed plan requires all 39 compressed backups to be fsynced and verified
by decompression hash and length before any exact leaf unlink. Source and
backup descriptors, directory identities, free-space bounds and protected
artifacts are checked throughout. No directory or symlink is removed.
The first execution stopped at its initial guard because an unrelated compiler
was active. No archive or deletion occurred in that execution. A fresh attempt
requires a real quiescent window and a separate receipt; old executors are never
resumed.

Inventory: `/var/tmp/o3-build/stale-own-server-cache-inventory-llx9tc2z/inventory.json`.
Root exclusion evidence: `/var/tmp/o3-build/cache-retirement-root-review-20261108.json`.
Final cleanup results, if any, must be recorded separately from this inventory.
