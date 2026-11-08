# Git integration and storage review — 2026-11-08

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
