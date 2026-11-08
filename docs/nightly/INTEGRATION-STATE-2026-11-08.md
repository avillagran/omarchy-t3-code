# Git integration and disk review, 2026-11-08

All inspected worktrees belong to the original project Git stores in
`/home/node/Work/o3-code/.git`, `/home/node/Work/o3-server/.git` and
`/home/node/Work/lib-t3-rust/.git`. The audit covered 22 client, 73 server and
13 SDK worktrees. Nightly 4353 remains an active implementation target;
complete compatibility has not been accepted.

## Integration state

| Repository | Integration state | Remaining work |
| --- | --- | --- |
| Client | `feat/nightly-4247-parity` contains unpublished implementation through `785c145` and audit documentation in `ce7e2ac`. Public draft PR #35 remains at `ba6324b`. Auth draft PR #36 (`eed75bb`) is also integrated locally with 21 byte-identical files. | Combined build and remaining GUI teardown acceptance; pending GUI work is not published. |
| Server | Public `feat/nightly-4247-fresh-context` is `6be0e55`, draft PR #50. The accepted no-tab/negative source work is already integrated. | Native-launch fixture correction, complete present Status and managed-account controller/store work remain separate source candidates. CI at the public head passes default mode and fails minimal on the old JSON publication fixture. |
| SDK | Public `feat/nightly-4340-auth-remove-method` is `64e322be`, draft PR #40, clean. Required native and Android artifact CI passed at this head. | Whole-client/server integration and actual managed-store acceptance remain separate from SDK acceptance. |
| Android | `/home/node/Work/o3-android` is clean at `f3aa34034ed459160773eed728cc79efe52d2b6b`, on `feat/android-client-parity-20261106`, 15 commits ahead of `origin/main` (`f7888857`). A read-only remote check found no published feature branch; no upstream is configured. | The 15 commits are saved locally, but are not published on this branch or merged into `main`. No Android build or tests were run for this audit. |

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

A second independent check resolved all 15 refs and verified that all 291
captured files still match their saved bytes and file identities. The later
integration report is the only additional committed path in the primary tree.
The GUI and reload checkpoints are not ancestors of the integration branch.
Reload still changes all 148 files with `git diff -w`; it must not be discarded
as a formatting-only change without reviewing the actual diff. The seven
primary prototypes and 21 GUI files also remain separate work in progress.

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
Both executions stopped at their initial guard because an unrelated compiler
was active. Neither created archives or deleted files. No third attempt was
started. The 39 candidate files and all 16 protected artifacts remain unchanged.

Inventory: `/var/tmp/o3-build/stale-own-server-cache-inventory-llx9tc2z/inventory.json`.
Root exclusion evidence: `/var/tmp/o3-build/cache-retirement-root-review-20261108.json`.
Final cleanup results, if any, must be recorded separately from this inventory.

## Final disk observation

The later read-only observation found 63,542,906,880 available bytes on HOME
(about 59.2 GiB), 2,525,261,824 on ROOT (about 2.35 GiB), and 220,889,088 on
`/mnt/avd` (about 211 MiB). Free space changed while unrelated work was running;
this audit did not establish its cause and does not claim that space as its own
cleanup result. The HOME cache plan would consume scarce ROOT space for its
backups, so it was not attempted again.

BuildCache accounts for approximately 522.5 GB of allocated space. On ROOT,
`/var/tmp/o3-build` accounts for approximately 50.1 GB and
`/var/tmp/o3-server` for 50.4 GB. These include source worktrees, retained runtime
fixtures, evidence and backups, so their total sizes are not reclaim estimates.
In particular, their `tmp` directories contain approximately 9.27 GB and
26.66 GB of retained fixtures.

A bounded scan identified 1,182,683,136 allocated bytes (about 1.10 GiB) of old
incremental compiler files under
`/var/tmp/o3-build/lib-nightly-4247/target/debug/incremental`. Another 19 old
regular `.rlib`/`.rmeta` leaves in ROOT targets account for 482,652,160 bytes.
These are candidates for an ownership and input-manifest check, not approved
deletions or demonstrated reclaim. Current protected targets remain excluded.

`/mnt/avd` also contains system and development snapshots and incident rescue
data. Its `android-avd` area accounts for about 25.07 GB, including 13.86 GB
under `RESCATE-2026-11-05`; it is not a disposable emulator cache. No snapshot,
rescue data, AVD, worktree, source, evidence or archive was removed.

The final disk audit is
`/var/tmp/o3-build/root-cache-and-disk-audit-20261108.json`, SHA-256
`f7ceca784376d710a606f19175aa34b71de40c534ec61127552df392e0c09282`.
