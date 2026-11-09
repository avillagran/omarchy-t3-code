# Git integration and storage review — 2026-11-08

> Current status: see the [23:59 UTC audit](GIT-STORAGE-STATUS-2026-11-08.md).
> The observations below are retained as historical evidence.

## Latest observed state — 22:39 UTC

The project is not fully integrated or published. The opened client checkout is
clean at published `07f0a01`; the implementation checkout remains a separate
worktree at `c167b72`, 31 commits ahead of the actual remote feature tip
`ba6324b`, before this documentation commit. Its 36 pending source paths are
preserved in local checkpoints. Browser, auth, DateTime, voice and GUI work
remain at different integration and validation stages.

The fresh inventory covers 117 worktrees: 22 client, 80 server, 14 SDK and one
Android checkout. The final read at 22:39 UTC found 17 dirty worktrees and 565
pending path versions, all exactly matched to reachable local checkpoint blobs.
These are file versions across worktrees, not 565 independent features.

The first read found seven new provider-cancellation versions absent from all
290 server refs. That concurrent lane subsequently had nine pending paths. Its
fresh, stable nine-path state is preserved in local commit `a0ee94e1573d1ff1a1a214c062cddd9989294138`,
`refs/checkpoints/audit-20261108/current-provider-cancellation-recheck`, using
an independent index. Source bytes, identities, HEAD and the real index were
unchanged. The final 565-path observation supersedes the earlier 563-path cut;
it does not claim preservation of transient versions between observations.

Published SDK `39b73284` and server `1797ed0e` feature tips remain separate from
their default branches. Android `632af9fc` is clean with 16 local commits over
main, without an upstream or same-name remote feature branch. Local checkpoints
protect source; they do not establish integration, publication or acceptance.

Existing proof results were inspected without starting another build or test:
the canonical client main and public SDK compile passed, and the private Qt
sidebar scenario passed with the unchanged 16-scenario worker proof. Its shell
metadata and supplied server-result events qualify this as UI acceptance,
not live multi-environment server acceptance. Browser Open passed; viewport
320×240 and DPR 2 passed before snapshot timed out. PNG and Encoder acceptance
remain pending. Native DateTime and auth ownership source also need their
prepared compilation/runtime gates. Full Nightly parity is incomplete.

Current allocated sizes are 528.49 GB for BuildCache and 164.27 GB for the
separate server cache. The three Git directories together allocate 139.87 MB.
ROOT has 2.21 GB free, HOME 49.40 GB, and the separate AVD volume 38.49 MB.
No observed free-space change is attributed to this audit.

All previously reviewed candidate sizes remain unchanged: historical server
caches in HOME total 134.27 GB; GUI groups within BuildCache total 47.13 GB;
the three old ROOT targets total 17.37 GB. These are review candidates, not
guaranteed physical reclaim or blanket deletion approval. No candidate appeared
in inspectable process references; restricted `/proc` reads limit that result.
Current targets, reserved artifacts, worktrees, source checkpoints, unique
proof evidence, rescue data, Android images and snapshots were retained.

Evidence is in `/var/tmp/o3-build/git-storage-current-review-jasvmnho/`:

- `final-preservation-resolution.json`, SHA256 `19dd139628a12b363069a4fc4892817d33e94543b9c3ded0880d27b07118f9bd`: all 565 observed versions matched local checkpoints; every worktree read was stable.
- `provider-cancellation-preservation.json`, SHA256 `e1716c31e25e5382aafbae6f4cc483f512e11deab21b8b5ec5c3d23bcdae5e4e`: exact nine-path preservation without changing the active worktree.
- `storage.json`, SHA256 `bd9eefed8efb517fb41b9cae03e7e198433149a4a5fe0a567a3099c4e5f48a73`: fresh candidate sizes, free space and process visibility limits.

The live remote and branch audit is
`/var/tmp/o3-build/current-three-repo-audit-20261109.json`, SHA256
`fc5e0d464f4d24acced84daa6f903f30a89678b4a0ceb7df2de3c54805a8b9f4`.
No cleanup, feature merge, code publication, installation or new build/test/runtime
was performed for this review. This documentation adds one local client commit.

## Current recheck — from 21:13 UTC

The work is still not fully integrated or published. The fresh inventory covers
117 registered worktrees: 22 client, 80 server, 14 SDK and one Android checkout.
Sixteen worktrees contain 534 pending paths. These are path versions, including
the same relative filenames in different worktrees, rather than 534 features.

Existing durable local checkpoints cover 522 current versions. Twelve recent
maintenance versions in `/var/tmp/o3-server/force-stop` were absent from every
existing server ref. They are now preserved exactly in `273169514b534e1d573bc2e36a7e0bb765e74c20`,
`refs/checkpoints/audit-20261108/mise-maintenance-current-recheck`, using an
independent index. HEAD, the real index, source bytes, modes and identities did
not change. One documentation file changed concurrently after the first
observation; the checkpoint uses its freshly observed stable version. The
earlier 510-path audit below is a separate historical observation.

During final revalidation, the concurrent maintenance lane committed those
twelve versions in `05743fe7` and became clean. The final observed pending count
is therefore 522 paths in fifteen dirty worktrees. The preserved 534-version
selected cut and the later 522-pending-path state are distinct observations.

| Scope | Current verified state |
| --- | --- |
| Opened client checkout | Clean, published `07f0a01`; this is a different checkout from the integration branch. |
| Client integration checkout | `7540744`, 30 commits ahead of live `ba6324b`, before this report update. Twenty WIP paths exactly preserved in local `da7bed01`; combined UI acceptance and publication remain pending. |
| Wayland GUI | Twenty-six WIP paths exactly preserved in `d67bbc4c`; separate from the client integration branch. |
| Managed auth server | 164 WIP paths exactly preserved in `45983218`; local, not merged or published. Factory compilation and composed three-case private acceptance pass; full provider catalog/lifecycle/production activation remain pending. |
| Complete Browser Status | Fifty-one WIP paths exactly preserved in `fbe327d4`; local, not merged or published. Strict minimal/default metadata checks pass; physical feature tests and integration remain pending. |
| Server integration feature | Clean, published `19132bf`. Main checkout remains clean at `97d3e12`, six commits behind live `dev` at `4cf398c`. |
| Thread-open server | Clean, published `1797ed0e`, draft PR61. Both CI239 jobs pass; no default-branch merge or installation. |
| Thread-open SDK | Clean, published `39b73284`, draft PR41, both public CI workflows pass. Exact public pin is committed in the client; SDK main remains `104a9abd`. |
| Maintenance server | Clean `05743fe7`; the twelve preserved versions are now committed in this separate lane. |
| Android | Clean local feature head `632af9fc`, sixteen commits ahead of live `main` at `f7888857`; no upstream or published feature branch. |

Current ROOT availability is approximately 2.22 GB and HOME 51.19 GB.
BuildCache allocates 528.27 GB on HOME. The Git directories for the client,
server and SDK together allocate approximately 135 MB; they are not the main
space consumers. The separate AVD filesystem has approximately 571 MB free;
its change since the earlier observation is not attributed to this audit.

All fourteen historical server candidate sizes and cache tags below were
rechecked and are unchanged: 134.27 GB in HOME and 17.37 GB in ROOT. The three
GUI candidate groups are also unchanged at a combined 47.13 GB. No candidate
path appeared in inspectable current process references; restricted `/proc`
queries prevent a universal absence claim. Sizes are allocated accounting,
not guaranteed physical reclaim. Preserve unique evidence and check current
owners and shared blocks before any separately scoped cleanup.

No cleanup, feature merge, code push, deployment or new build/test/runtime was
launched for this recheck. An existing Browser metadata compilation was
observed to its actual terminal success. Current artifacts, worktrees,
checkpoints, proof banks, Android images and snapshots were retained.

Current evidence:

- `/var/tmp/o3-build/git-preservation-current-audit-mgj0_k3t/`: fresh inventory, live remote heads, force-only refreshed observation, all-ref coverage and preservation receipt.
- `/var/tmp/o3-build/git-disk-recheck-20261108-ww3z698n/storage.json`, SHA256 `80f8c7ae191e1431fe0ae4e05d05992e11407641d8452680384f0b2db9bf9771`: fresh sizes, mounts, free space and process-query limitations.
- `/var/tmp/o3-build/thread-open-4380-qualified-components-ci239-green.json`, SHA256 `ab7615daf5ec5343f04e252a740383f9384bbaceafc02828695b4705b0eebf3f`: public server/SDK validation and remaining limits.
- `/var/tmp/o3-server/status-complete-metadata5-f2318w3i/result.json`, SHA256 `8fcf8954b9639d0f87de8d37f058403a8fccda60c4728ae47d7da3a59488215c`: strict metadata success, zero tests or executable links.

## Earlier snapshot — 19:44–19:52 UTC

The work is not fully integrated or published. All 510 pending source-path
versions observed across 117 worktrees have exact copies in durable local Git
refs. Checkpoints preserve work; they do not merge, publish, or validate it.
Concurrent development can add changes after this observation.

## Integration

| Scope | Verified state |
| --- | --- |
| Opened client checkout | Clean `07f0a01`, published `fix/share-t3-o3-label`. |
| Client integration checkout | `/var/tmp/o3-build/o3code-nightly`, `d5c49a9`, 26 commits ahead of live `ba6324b`; 30 pending paths. This documentation commit adds one local commit. |
| Server integration feature | Clean `19132bf`, matching live `feat/nightly-4247-fresh-context`. |
| Server main checkout | Clean `dev` at `97d3e12`, six commits behind live `origin/dev` at `4cf398c`; not updated by this review. |
| Managed auth server | 160 pending paths, all exactly preserved in local `3127cf12`; not integrated in `dev` or published. |
| Complete Browser Status | 28 worktree paths preserved in `4c61c5a2`; later 25-path staged source `52263503` remains separate, uncompiled work. |
| SDK auth | Clean published feature `64e322be`, not merged into main `104a9abd`. |
| Thread-open SDK/server | Clean local feature heads `a22c5a5c` / `1764baaf`; no published feature branches. SDK compile passes; private runtime is prepared but has not run. |
| Android | Clean `f3aa340`, 15 commits above `origin/main` at `f7888857`; no upstream or published feature branch. |

The review found 16 dirty worktrees: three client and thirteen server. All
fourteen SDK worktrees and the Android checkout are clean. Path counts include
the same relative paths in different worktrees; they are not 510 independent
features. Branch ancestry counts likewise do not prove missing patch content.

New preservation refs saved the latest client search/visit work and concurrent
server maintenance fixtures without changing selected HEADs, real indexes,
source bytes, modes, or file identities:

- `refs/checkpoints/audit-20261108/native-search-current`, `d5d79117`: all 30 client pending paths.
- `refs/checkpoints/audit-20261108/mise-maintenance-current`, `05f751e2`: the first two maintenance helpers.
- `refs/checkpoints/audit-20261108/mise-maintenance-concurrent`, `71e22975`: all five subsequently observed maintenance paths.

The latest client search runtime passes the original 34 callbacks / 192
assertions in Electron and the separate ten-scenario worker protocol proof.
The new Rust/QML integration still needs compilation and combined acceptance.
Preserving those sources is not acceptance of the combined application.

## Storage

ROOT has approximately 2.23 GB available; HOME has 52.46 GB. BuildCache allocates
528.08 GB on HOME. The separate historical `.cache/o3-server` candidates below
allocate another 134.27 GB. Allocations do not establish unique physical blocks
or guaranteed reclaim, particularly with hardlinks, shared extents and snapshots.
Three private overlay directories could not be read during the global scan.
The separate AVD filesystem has only 51.90 MB available; cleaning HOME or ROOT
will not replenish it. Android images were not selected for removal.

| Review candidate | Allocated bytes | Qualification |
| --- | ---: | --- |
| Eleven historical HOME server `checkpoint-*` caches | 134,268,284,928 | Valid cache tags, sizes unchanged, no matches in current Status/auth admissions. |
| ROOT `/var/tmp/o3-server/live-history-target` | 13,529,362,432 | Historical Cargo target; not the current Status/auth/protected-server target. |
| ROOT `/var/tmp/o3-server/target-release` | 1,657,217,024 | Historical tagged Cargo target. |
| ROOT `/var/tmp/o3-build/node-release-target` | 2,179,596,288 | Historical tagged Cargo target. |
| HOME `o3-desktop-next/gui-glue-target` | 23,898,259,456 | No current inspected admission or admitted-inode overlap. |
| HOME `o3-desktop-next/session-target` | 8,887,746,560 | Same qualification. |
| HOME 66 direct `browser-gui-platform-*` fixtures | 14,344,863,744 | Historical executable copies and evidence; preserve unique receipts/source/profile data before considering removal. |
| ROOT `lib-nightly-4247/target/debug/incremental` | 1,182,683,136 | Regenerable output, including one inode with an outside hardlink; ownership/admission review remains necessary. |

The first three ROOT targets total 17,366,175,744 bytes (16.17 GiB).
The three GUI groups form a disjoint candidate union of 47,130,869,760 bytes.
Nested fixtures inside `gui-glue-target` are already included and must not be
added again. These are review candidates, not a guaranteed reclaim total.
No references were found in inspectable current processes; some `/proc` reads
were restricted. Current executable reservations remain outside these groups.

The eleven HOME cache basenames under `/home/node/.cache/o3-server` are
`checkpoint-01fa5f2`, `checkpoint-21de2b9`, `checkpoint-3eee6df`,
`checkpoint-5b031dc`, `checkpoint-7b12206`, `checkpoint-8534855`,
`checkpoint-canonical-positions`, `checkpoint-controller`, `checkpoint-d89ec19`,
`checkpoint-native-busy`, and `checkpoint-release-records`.

Sources, checkpoints, current caches, accepted/failed proof banks, Android
images, snapshots and rescue data must remain distinguishable from disposable
compiler output. HOME cleanup cannot replenish ROOT. No cleanup, build, test,
runtime, merge, code push, or deployment ran for this audit.

## Evidence

The fresh audit directory is
`/var/tmp/o3-build/git-disk-review-20261108-m4z9vkdf/`:

- `preservation-resolution.json`: all 510 observed paths matched checkpoint blobs; SHA256 `22bb55a057c2c48346dc9130f4a0b37afbd275e8e84c5e01dd1856b9139fae2f`.
- `preservation.json` and `concurrent-preservation.json`: exact local refs and HEAD/index/source preservation.
- `global-allocation.json`, `desktop-allocation.json`, and `incremental-allocation.json`: allocated sizes and hardlink qualifications.
- `historical-cache-candidates.json`: all fourteen server candidate paths, tags and current sizes; SHA256 `d28de028aeab74b1c3ab1e2a83d323712b129795cf543ec0831b940204f2f431`.
- GUI owner review: `/var/tmp/o3-build/gui-historical-cache-current-w1sojqzb/report.json`, SHA256 `79b2734e5b31bbfcca097cd576f7a257b036b472c1a05230d94f919794ab3802`.

The full Nightly 1:1 implementation goal remains active and incomplete.
