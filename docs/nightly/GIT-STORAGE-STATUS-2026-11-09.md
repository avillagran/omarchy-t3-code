# Git and storage status — 2026-11-09, 03:11 UTC

The work is **not fully integrated or published**. The current source census
contains 120 worktrees, 16 with pending changes, and 660 pending file versions.
All 660 versions have exact local Git copies, including file modes. The final
stability check found no changes to the observed sources, HEADs, or indexes.
Local checkpoints protect source versions on this machine; they do not establish
feature acceptance, application integration, or an off-machine backup.

This is a new source delta and storage follow-up to the
[previous audit](GIT-STORAGE-STATUS-2026-11-08.md). Its 118-worktree/544-version
cut remains historical. The counts below precede this documentation commit.

## Integration

| Component | Current state | Remaining work |
| --- | --- | --- |
| Opened client checkout | Clean `07f0a01`, `fix/share-t3-o3-label` | Separate from the Nightly implementation checkout. |
| Nightly implementation | Published `163b31db`; production changes through `71a6ed9d`, draft PR #35 | Thirteen untracked prototypes remain exactly checkpointed. Active SDK pin is still `39b73284`. |
| Combined client candidate | Clean local `99d70b89`, including public SDK `1c6c8c82` and palette changes | Separate branch; no same-name remote branch. Runtime validation and integration into the Nightly checkout remain pending. |
| Server integration branch | Clean, published `19132bf5` | Native DateTime `4198b7a9` remains on a separate published branch. Browser and Auth changes below are not integrated. |
| Browser | 66 pending paths preserved in local source36 checkpoint `04477f0a` | Source36 is uncompiled and unrun; no same-name remote branch. Earlier compile/PNG proofs do not validate the new source or complete Encoder functionality. |
| Auth | 292 pending paths preserved in local checkpoint `592509b5` | Updated compiler and runtime fixtures remain unrun; complete session-manager and MCP capability integration is pending. No same-name remote branch. |
| SDK | Clean public `1c6c8c82`, draft PR #43; affected private checks and seven CI jobs passed previously | Its publication is separate from integration into the active client. |
| Android | Clean `632af9fc`; 16 patches beyond main | No upstream or same-name remote feature branch. |

The census covers 23 client, 80 server, 16 SDK, and one Android worktree.
Pending versions comprise 187 client and 473 server paths; SDK and Android
worktrees are clean. These are file versions across worktrees, not feature counts.
Selected published branch tips were verified with live `git ls-remote` calls
without fetching or changing admitted Git object databases.

The combined client candidate's actual strict and normal Cargo commands passed.
Its original overall owner result remains failed because an inherited fence
incorrectly treated an ordinary old cache rlib as immutable. A separate read-only
review verified 405 emitted graph files, 1,755 sources, and 29 reserved executable
aliases. This qualified compile evidence does not establish runtime acceptance
or turn the original failed result into a pass.

## Disk usage and cleanup candidates

All sizes use decimal GB. Parent and nested rows must not be added together.
Measurements below were taken around 03:09–03:11 UTC.

| Location | Allocated / available | Interpretation |
| --- | ---: | --- |
| ROOT free | 1.91 GB | Separate filesystem from HOME; ROOT needs its own space recovery. |
| HOME free | 45.03 GB | Current available space. |
| `~/BuildCache` | 529.83 GB | Main build-output consumer; includes current and historical artifacts. |
| `~/.cache/o3-server` | 164.27 GB | Includes the 134.27 GB of historical candidates below. |
| `/var/tmp/o3-build` | 51.09 GB | Mixed source worktrees, build output, and proof receipts. |
| `/var/tmp/o3-server` | At least 51.83 GB | Same mixture; three private overlay subdirectories were unreadable. |
| Eleven historical HOME Cargo caches | 134.27 GB | Root identities, allocated sizes, and valid Cargo cache tags match the previous audit. |
| Three historical ROOT Cargo targets | 17.37 GB | Same identity, size, and cache-tag checks passed. |
| Earlier GUI candidate groups | 47.13 GB | Included in BuildCache: gui-glue 23.90 GB, session 8.89 GB, and 66 fixture roots 14.34 GB. |

The HOME candidates are under `~/.cache/o3-server/`:

`checkpoint-01fa5f2`, `checkpoint-21de2b9`, `checkpoint-3eee6df`,
`checkpoint-5b031dc`, `checkpoint-7b12206`, `checkpoint-8534855`,
`checkpoint-canonical-positions`, `checkpoint-controller`, `checkpoint-d89ec19`,
`checkpoint-native-busy`, and `checkpoint-release-records`.

The ROOT candidates are:

| Path | Allocated |
| --- | ---: |
| `/var/tmp/o3-server/live-history-target` | 13.53 GB |
| `/var/tmp/o3-server/target-release` | 1.66 GB |
| `/var/tmp/o3-build/node-release-target` | 2.18 GB |

These fourteen historical candidates total **151.63 GB** across two filesystems.
That is candidate allocation, not certified garbage or guaranteed reclaim.
Current checks revalidated identity, cache tags, and allocation. Process and
container observations from the previous audit remain historical; proof
reservations, current ownership, and shared extents/snapshots still require
accounting before retiring individual paths.

Three large BuildCache targets retain the same total allocations: server
admission 124.06 GB, server fresh-context 97.63 GB, and client Nightly 68.20 GB.
The previous detailed inventory found 297 nested runtime banks/pools occupying
163.94 GB in these targets, with 11,672 regular files and no shared inodes between
banks. This follow-up rechecked parent allocations rather than rehashing bank
contents. Some banks support historical proofs, so the whole 163.94 GB is not
classified as disposable.

All four repositories currently report zero Git garbage objects. Removing
branches would not address the dominant build-output consumption. Android's
large recovery-history objects remain retained.

No cache, source, branch, recovery history, executable, or proof receipt was
deleted. Current client/server/SDK targets and executable reservations remain
protected. This audit launched no compiler, test, runtime, installation, or merge.
The only concurrent implementation change was the seven-path Browser source
checkpoint, which was independently revalidated before closing the census.

## Evidence

- Source census and exact preservation: `/var/tmp/o3-build/git-storage-followup-20261109-wqy60_i1/source-delta.json`, SHA256 `e350e43d56887b889d36f419d4584815def7b21ec294ff64514dca968066032d`.
- Component and live remote checks: `/var/tmp/o3-build/git-storage-followup-20261109-wqy60_i1/components-storage.json`, SHA256 `cef265d3451335fcb38cba9f9524a796db81b9d4df240e24aab05c71c42b8630`.
- Disk follow-up: `/var/tmp/o3-build/git-storage-disk-followup-20261109-qb5o9se1/storage-followup.json`, SHA256 `19f08d612ac042ec3d0068334b82d606995ad5f3f7a12f77114955a4266407a2`.

Later concurrent changes require another observation; this cut does not silently
certify future source or cache state. The complete Nightly parity goal remains
active and incomplete.
