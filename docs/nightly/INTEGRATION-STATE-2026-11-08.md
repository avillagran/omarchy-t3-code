# Git integration and disk review, 2026-11-08

## Latest follow-up: current integration and storage

Integration remains incomplete. The opened checkout is clean `07f0a01`; the
client integration checkout still selects local `e280d88`, 24 commits ahead of
the live feature branch `ba6324b`, before this documentation follow-up. The
server feature branch is clean and matches live `19132bf`, and the SDK auth
branch is clean and matches live `64e322be`. All thirteen SDK worktrees are
clean. Android is clean `f3aa340`, fifteen commits above live `origin/main`,
without an upstream or published feature branch.

The audit observed 487 pending paths across sixteen client/server worktrees.
Two current force-stop fixtures lacked checkpoint matches; both are now saved
in `2aa9baaa`, at
`refs/checkpoints/audit-20261108/force-stop-latest-dialog-boundary`, with HEAD,
the real index and source bytes preserved. Six new palette-search source files
are saved in `55896144`; the previous seven primary prototypes remain in
`dd8181a`. These checkpoints preserve work without merging or publishing it.
Ongoing implementation may add later source changes after this observation.

Current separately checkpointed candidates include GUI production/failure
diagnostics `a5f7a1ce` and its newer input-route investigation `d02fae17`, managed
auth `6ea2a687`, and full Status producers `0e492f76`. The diagnostic GUI bank
passes five cases, but production-bank attempts 37 and 38 fail on different
paths. Managed auth passes strict metadata in both modes and a normal library
build; its actual native lifecycle remains unaccepted. The first full Status
metadata attempt fails on missing standalone upload-policy coverage. None of
these observations establishes complete product acceptance or integration.

[Complete native palette search](NATIVE-SEARCH-4367.md) passes 34 original
callbacks and 192 assertions in V8; Qt's Unicode/lookbehind incompatibility and
the actual product/history consumers remain open.

### Storage observation

ROOT has approximately 2.31 GB available, HOME 60.01 GB. BuildCache allocates
522.40 GB on HOME. The two development trees on ROOT allocate approximately
50.78 GB and 51.12 GB respectively. These are allocation observations, not
guaranteed reclaim: hardlinks, shared extents and permission-limited private
directories affect totals.

| Cleanup review candidate | Allocation | Current qualification |
| --- | ---: | --- |
| ROOT: old `lib-nightly-4247/target/debug/incremental` | 1.18 GB | Regenerable compiler output; verify ownership and absence from current admissions before removing it. |
| HOME: `o3-desktop-next/gui-glue-target` | 23.90 GB | Historical output absent from the current audited GUI admissions; other owners and unique evidence still need review. |
| HOME: `o3-desktop-next/session-target` | 8.89 GB | Same qualification. |
| HOME: 66 old `browser-gui-platform-*` fixtures | 14.34 GB from independent measurements | Contains executable/probe copies; do not add to parent totals or remove without checking hardlinks and unique evidence. |

GUI evidence also contains about 2.95 MB of verified regenerable private-index
and Python bytecode files. Five auth admission manifests allocate about 160 MB,
but remain tied to retained evidence. Future owners now reference immutable
admission bases by hash with small overlays, rather than duplicating the full
manifest. Active executables, accepted and failed proof banks, source,
checkpoints and Android images remain protected. No files were deleted for
this audit. Cleaning HOME does not replenish ROOT.

Evidence under `/var/tmp/o3-build/`:

- `git-disk-recheck-20261108-k2brkfu2/report.json`, SHA256 `d65dbebc06a670ca4a6730ab9e51ca77750a6cbaff26f0dfc0c4ef73693f00d5`. Its initial SDK/Android path selection was corrected separately in `sdk-android-correction.json`; the corrected scope has 114 worktrees including Android.
- `native-palette-checkpoint-v2-594zl6du/receipt.json`, SHA256 `ed104cd06c66ba5121807db5ff088125b03f8236cd332c795f4ace38048ee715`.
- `gui-storage-checkpoint-supplement-vn94llgb/report.json`, SHA256 `2539b5f3aeff0bd6d8143cfac20513c2261639e996c38fa56acb05298e7c3035`.
- Force-stop receipt under `/var/tmp/o3-server/force-stop-current-preserve-x8dr8fph/receipt.json`, SHA256 `c174cb5316f76be88b47b28bc72f8c46546a117e94ec5b1a8ab3248a9df51933`.

## Prior recheck: preserved work, incomplete integration

The recheck covers 22 client, 78 server and 13 SDK worktrees, plus Android.
These are worktrees of the existing project Git stores. All 468 pending source
paths observed in sixteen dirty worktrees now have exact local Git checkpoints.
Saving work in a checkpoint does not integrate, publish or install it.

| Repository | Verified branch and publication state |
| --- | --- |
| Client | Implementation head `b929bd5` is 23 commits ahead of live public `ba6324b`. This documentation commit adds one further local commit. The opened checkout is still clean `07f0a01` on `fix/share-t3-o3-label`; the integration checkout is `/var/tmp/o3-build/o3code-nightly`. |
| Server | Primary `19132bf` is clean and matches the live remote feature branch. [CI 42497001835](https://github.com/avillagran/o3-server/actions/runs/42497001835) succeeds in both modes at this exact head, including formatting, fixture policy, tests, strict Clippy, compatibility and Python checks. |
| SDK | Auth branch `64e322be` remains clean and published; all thirteen SDK worktrees are clean. The main checkout remains `104a9abd`. |
| Android | Clean `f3aa340`, fifteen local commits above `origin/main` at `f7888857`, with no upstream or published feature branch. No Android build or tests ran for this recheck. |

The three previously identified client omissions are now integrated locally:
T3/O3 sharing text, provider device-code copying and the last preferences page.
A further Rust persistence omission was corrected. [Scoped proof and cache
findings](RESTORED-CONTROLS-4367.md) distinguish two passing QML cases, actual
four-process state-module persistence and a normal library build from the
pending rebuilt-application and combined GUI regression.

### Source preservation

The initial observation found 181 pending client paths and 287 server paths.
Eight recent paths lacked matching checkpoint blobs. They are now preserved:

| Candidate | Local checkpoint | Scope |
| --- | --- | --- |
| GUI scoped registration, sibling and capacity source | `b5ae5d46`, `checkpoint/20261108/gui-context-sibling-capacity-4340` | All 26 GUI paths; includes the three latest fixture changes beyond `2da26d0`. No build or runtime acceptance of this new candidate. |
| Current query-reader work | `82490289`, `refs/checkpoints/audit-20261108/force-stop-reader-current` | Five current paths, with source, selected HEAD and real index preserved. |
| Managed native auth and linked-environment lifecycle | `fd088c83`, `checkpoint/20261109/managed-auth-native-worker-linked-source` | 156 pending source paths; includes the complete 113-file native 4367 closure. Uncompiled, inactive and unpublished. |
| Complete Browser Status producers | `3e6a1cc1`, `refs/checkpoints/status-complete-producers-4367/20261108-source-2` | Thirteen source paths, uncompiled and unpublished. |

An independent follow-up reread all 468 initial paths: bytes, size and mode
remained identical, and all were covered by durable local checkpoint refs.
It checked 1,395 recorded commit/path blob claims without a discrepancy.
This is an observation of those pathspecs, not a promise that ongoing work
cannot add new changes. Seven primary prototypes and the 148-path reload
candidate remain separate preserved work in progress.

The server's canonical birth and capture changes are now public, including
actual ACK-before-navigation and cancellation evidence. CI at `65beeff` first
failed thirteen tests because two synthetic CDP peers omitted session IDs.
Head `19132bf` fixes those peers without changing production matching; a private
twelve-reply protocol probe and the new full CI both pass. Complete Status,
managed-auth lifecycle, GUI acceptance and the wider Nightly goal remain open.

### Disk candidates and active reservations

BuildCache currently allocates approximately 522.4 GB (486.5 GiB) on HOME.
Its largest entries remain `o3-server` (221.7 GB), `o3-code` (68.2 GB) and
`o3-desktop-next` (60.3 GB). These include active artifacts and are not removable
space estimates. ROOT has about 2.35 GB available, HOME about 60.9 GB, and the
AVD filesystem only 38.6 MB at observation. Cleaning HOME does not free ROOT.

| Review candidate | Measured allocation | Qualification |
| --- | ---: | --- |
| `/home/node/BuildCache/o3-desktop-next/gui-glue-target` | 23,898,259,456 bytes | Historical compiler output; no match in the audited current GUI/primary admissions. Other-owner and historical evidence review still needed. |
| `/home/node/BuildCache/o3-desktop-next/session-target` | 8,887,746,560 bytes | Same qualification; no current deletion or reclaim claim. |
| 66 sibling `browser-gui-platform-*` fixture directories | 14,344,863,744 bytes from independent `du` calls | Several contain large executable/probe copies; hardlink overlap and unique evidence must be checked. Do not add this figure to parent totals. |
| `/var/tmp/o3-build/lib-nightly-4247/target/debug/incremental` | 1,182,683,136 bytes | Old incremental output on ROOT; candidate for an ownership/input check. |
| The two development `tmp` directories on ROOT | 35,926,085,632 bytes combined | Retained test fixtures and private container data; some directories cannot be read. Not blanket disposable temporary data. |

All nineteen protected GUI/client artifact identities remain intact, including
the current library-test executable `17cfc60b`, retained application/watch
executables and accepted or failed Qt banks. All twenty-four server executable
reservations remain protected. A stale shared Cargo SDK fingerprint was fixed
earlier by a reversible sixteen-byte rename; that is a correctness fix and
does not explain changes in free space.

No cache trees, worktrees, sources, executables, snapshots, rescue data or
Android images were removed in this recheck. No build or runtime test was
started for the audit. Earlier cleanup scopes remain closed. The disk figures
are allocated bytes, with possible shared extents and hardlinks, rather than
guaranteed reclaim.

Evidence:

- Git observation and five-path preservation: `/var/tmp/o3-build/git-disk-current-audit-b5y7ow23/`.
- Independent 468-path verification: `/var/tmp/o3-build/git-disk-current-independent-verify-zp0h0o2r/result.json`, SHA256 `cee546ec404455ce18cb756391fc50e9ee334ce38921fba34609b993420d68a7`.
- GUI source checkpoint: `wayland-input-method-owner-source-review/gui-sibling-capacity-checkpoint-ceci6h8c/receipt.json`, SHA256 `b6f9e1e0fe9589a2a8b21ed64d35c85aa6085a81a04e66e2112d52c2f1d2227b`, under `/var/tmp/o3-build`.
- GUI disk/reservation inventory: `/var/tmp/o3-build/gui-client-readonly-disk-audit-qthmxlbe/report.json`, SHA256 `ee213009546baab6f0bb311b9cda9a2984278fb8c27cf06e37680b344cd14b7a`.
- Exact-head CI: `/var/tmp/o3-server/cdp-session-fix-publication-remj1qh2/ci1913/terminal.json`, SHA256 `ce9aa4e57da713d148ccf300ab4b15d8301e896a1389dd106df8d9d363137166`.

## Prior audit: Nightly 4367

The audit covers 22 client, 76 server and 13 SDK worktrees, plus the Android
checkout. All use their original shared Git stores. Work is preserved in Git,
but integration and publication remain incomplete.

| Repository | Verified current state |
| --- | --- |
| Client | Integration source head `0da87b6`, 18 commits ahead of live public `ba6324b` before this documentation commit. The audit commit adds one further local commit. The opened checkout still selects `fix/share-t3-o3-label`; implementation lives in `/var/tmp/o3-build/o3code-nightly`. |
| Server | Primary `6b4bb63` is clean and matches its live remote feature branch. [CI 42486616139](https://github.com/avillagran/o3-server/actions/runs/42486616139) completed successfully in both minimal and default modes at this exact head, including tests, strict Clippy, formatting and policy checks. |
| SDK | Auth feature `64e322be` is clean and published; all 13 SDK worktrees are clean. The main checkout remains `104a9abd`. Publication of the feature branch does not imply a default-branch merge. |
| Android | Clean `f3aa340`, 15 commits ahead of live `origin/main` at `f7888857`; no upstream or published feature branch. No Android builds or tests were run. |

### Pending changes and preservation

Fifteen worktrees contain 317 changed or untracked paths: 180 client paths
and 137 server paths. An independent final check read every current file and
compared it with its checkpoint blob: all 317 match. Seven primary-client
prototypes remain untracked in that worktree, with exact copies in checkpoint
`dd8181a`. Reload's 148 paths remain separate work in progress.

The audit found nine server `force-stop` paths without a matching checkpoint.
They are now saved in non-branch ref
`refs/checkpoints/audit-20261108/force-stop-current` at `771e267`.
File bytes, identities, modes, original HEAD and real index were preserved.
This is local preservation, without merge, publication or test acceptance.
Current candidate checkpoints also include GUI `cdcfa9ab` (25 paths), managed
auth `5b71ea4` (15 paths), and canonical Browser birth `833ffc23` (14 paths).

Branch ancestry alone overstates missing integration: nine client worktree
heads have equivalent cherry-picked patches in the primary. Accepted server
readiness, early-dialog, no-tab and handoff changes are likewise integrated
through equivalent patches. A focused source review nevertheless found three
actual UI omissions in the primary:

- OmaBridge still displays **Share T3 Code**, instead of the **T3/O3** wording
  saved on `fix/share-t3-o3-label` (`07f0a01`).
- Provider device-code sign-in lacks the **Copy code** action saved in
  `0e0f574`; the cloud-connection copy action serves a different flow.
- Preferences reopen without retaining their last page; the saved change is
  `e53eedc`.

Reset-credit authorization and immutable IME intake are present in evolved
form; unmatched patch IDs are not enough to declare features missing. The
remaining voice history and one non-ancestral profile-report merge still need
semantic review. GUI alias acceptance stopped at its first failed cleanup
case; the remaining fault cases were not started. Combined GUI regression and
the separate auth/Browser lifecycle candidates remain pending.

### Current disk allocation

| Location | Measured allocation or available space |
| --- | ---: |
| ROOT available | 2,408,382,464 bytes, about 2.24 GiB; 99% used |
| HOME available | 61,689,921,536 bytes, about 57.45 GiB |
| `/home/node/BuildCache` | 522,543,271,936 bytes, about 486.66 GiB |
| `/var/tmp/o3-build` | 50,471,018,496 bytes, about 47.00 GiB |
| `/var/tmp/o3-server` | 50,787,921,920 bytes, about 47.30 GiB |
| The two ROOT development `tmp` directories | 35,926,085,632 bytes combined, about 33.46 GiB |
| Old `lib-nightly-4247` incremental cache | 1,182,683,136 bytes, about 1.10 GiB |

BuildCache is on HOME; `/var/tmp` is on ROOT. ROOT needs its own space recovery.
The largest HOME caches are `o3-server` (about 206.47 GiB), `o3-code` (63.52
GiB), and `o3-desktop-next` (56.19 GiB). These totals include active artifacts
and are not estimates of removable space.

The old incremental cache is a concrete review candidate. Retained fixtures,
evidence, runtimes and backups account for much of ROOT usage and require
ownership checks. Fifteen clean client checkouts have ancestral or equivalent
committed patches and about 643 MiB of Git-listed source allocation; generated
files, ignored files, merge resolutions and current input reservations are
excluded from that assessment. No worktree removal is approved by these
figures. Btrfs shared extents, hardlinks and permission-limited private
directories also prevent treating `du` totals as guaranteed reclaim.

No files were deleted, cleanup archives created, builds launched, or tests
repeated in this audit. Earlier cleanup attempts remain closed. Source code,
rescue data, snapshots, Android images and reserved artifacts were preserved.

Evidence:

- Client: `/var/tmp/o3-build/o3-code-git-integration-audit-8okfy1oe/report.json`.
- UI review: `/var/tmp/o3-build/o3-code-git-integration-audit-8okfy1oe/semantic-five-candidates.json`.
- Server/SDK/Android: `/var/tmp/o3-server/git-integration-readonly-20261108-status-agent.json`.
- New checkpoint: `/var/tmp/o3-server/force-stop-preservation-ui6t6mjl/receipt.json`.
- Independent 317-path check: `/var/tmp/o3-build/git-disk-source-recheck-k884qkwv/result.json`.
- Disk: `/var/tmp/o3-build/sdk-auth-readonly-disk-summary-20261109.json` (the filename date differs from the observation date).
- Exact-head CI: `/var/tmp/o3-build/ci-4367-integration-audit-20261108.json`.

## Earlier audit record

The following observations describe earlier checkpoints. The current audit
above supersedes their moving branch heads, worktree counts and disk readings.

All inspected worktrees belong to the original project Git stores in
`/home/node/Work/o3-code/.git`, `/home/node/Work/o3-server/.git` and
`/home/node/Work/lib-t3-rust/.git`. The audit covered 22 client, 73 server and
13 SDK worktrees. Nightly 4353 remains an active implementation target;
complete compatibility has not been accepted.

## Integration state

| Repository | Integration state | Remaining work |
| --- | --- | --- |
| Client | `feat/nightly-4247-parity` contains implementation through `785c145` and validation records through `a38cc1e`, 16 commits ahead at recheck. Public draft PR #35 remains at `ba6324b`. Auth draft PR #36 (`eed75bb`) is also integrated locally with 21 byte-identical files. | Combined build, remaining GUI fault/quiet/burst acceptance and regression without diagnostics; pending GUI work is not published. |
| Server | Public `feat/nightly-4247-fresh-context` is `2e1721d`, draft PR #50. It integrates the native-launch JSON fixture correction, early owned-dialog retention and oversized handoff selection. Full CI `42479617926` passes both modes at this exact head. | Complete present Status, canonical Browser birth runtime acceptance and managed-account State/RPC/full-lifecycle work remain separate source candidates. |
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

Later local checkpoints preserve subsequent scoped work without changing the
selected integration branch:

| Candidate | Local ref | Checkpoint |
| --- | --- | --- |
| GUI26 lifecycle acceptance and remaining GUI source | `checkpoint/20261108/gui-context-lifecycle-4340` | `0979ee91a329` |
| Managed controller/store/lease, including the accepted original-peer gate | `checkpoint/20261109/managed-chatgpt-native-peer` | `c627eb83d6f1` |
| Canonical Browser birth source and strict metadata acceptance | `refs/checkpoints/status-canonical-birth-4353/20261108-source-3` | `d35b64ffda35` |

The managed peer gate passes 14 original removal outcomes and six real lease
protocol cases, including bidirectional live-owner exclusion and heartbeats.
It does not accept RPC/subscriptions or the complete auth lifecycle. The
canonical birth gate passes strict metadata checks in both modes, with zero
runtime tests; it does not establish physical ACK-before-navigation behavior.
Initial managed-catalog and Browser cancellation source changes are being
developed after these checkpoints and need their own final snapshots and gates.

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

## Read-only recheck after subsequent implementation

Remote refs still resolve to client `ba6324b`, server `2e1721d` and SDK
`64e322be`. Android still has no published feature ref and remains 15 local
commits ahead of `origin/main`. The SDK's primary checkout selects `main` at
`104a9abd`; its accepted feature branch is a separate checkout/ref. These are
Git integration facts, not evidence about the installed application.

The recheck measured 62,841,335,808 available bytes on HOME (58.5 GiB),
2,450,661,376 on ROOT (2.28 GiB), and 148,713,472 on `/mnt/avd` (142 MiB).
Allocated directory sizes remain approximately 487 GiB for BuildCache,
47 GiB for `/var/tmp/o3-build` and 48 GiB for `/var/tmp/o3-server`.
The largest BuildCache directories are `o3-server` (207 GiB), `o3-code`
(64 GiB) and `o3-desktop-next` (57 GiB); those totals are not reclaim estimates
and do not reopen previously closed cleanup targets. The small ROOT margin
also rules out treating retained fixtures or backup archives as disposable.

No files were deleted and no cleanup archives were created by this recheck.
The earlier cache attempts remain closed. Evidence:
`/var/tmp/o3-build/git-disk-recheck-levnm41u/result.json`, SHA-256
`1a2f40b27ef90beb13643ec69bf2fddcf3fcdc631cdbab219999b5806616b3f2`.
