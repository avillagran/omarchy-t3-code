# Git and storage status — 2026-11-08, 23:59 UTC

All 544 pending file versions observed across 118 worktrees now have exact local
Git copies. The work is **not fully integrated or published**. Local checkpoint
refs protect the observed source versions on this machine; they are not an
off-machine backup, feature acceptance, or a merge into the application branch.

## Implementation follow-up after the audit cut

The inventory counts and storage candidates below retain their original
23:59 UTC cut. They have not been presented as a new complete inventory after
the following implementation work.

- The Nightly client published `71a6ed9d` in draft PR #35, including original
  palette submenu navigation and nested search. Strict compilation and the real
  Qt/main fixture passed. The same 13 prototypes remain locally checkpointed.
- SDK `1c6c8c82fed581ce09e2e0cd9c0d19c00a772af8` is public in draft
  [PR #43](https://github.com/avillagran/lib-t3-rust/pull/43), stacked on the
  public lossless RPC work. Eight WebSocket cases, four HTTP cases and the
  affected original DPOP regression passed; all seven CI jobs passed on that
  exact commit. This covers the additive Rust transport.
- The active Nightly checkout still uses SDK `39b73284`. The isolated client
  candidate `3c94a377` combines the palette changes with exactly 141 canonical
  vendor leaves from public SDK `1c6c8c82`; it has not been compiled or run.
  A public SDK and a preserved candidate are separate from client integration.
- Browser source checkpoint `7086c366` passed four compile-only commands. The
  prior real PNG component produced 640×480. The latest Encoder runtime still
  fails during its auxiliary browser launch; no video was accepted. The abort
  cause remains unconfirmed, and Browser work remains unpublished.
- Auth checkpoint `592509b5` preserves guarded session publication and owned
  installation/retirement. Twelve regression fixtures are written and unrun;
  its updated compiler proposal has not launched. Complete session-manager and
  MCP capability-ceiling integration remain pending.

No cleanup was performed for these follow-ups. Current reserved executables,
source worktrees and validation receipts remain retained. Full Nightly parity
and integration into the application remain incomplete.

## Integration

| Component | Observed state | Remaining integration |
| --- | --- | --- |
| Opened client checkout | Clean, published `07f0a01`, branch `fix/share-t3-o3-label` | This checkout is separate from the Nightly implementation. |
| Nightly client | Published `815090ea`, draft [PR #35](https://github.com/avillagran/omarchy-t3-code/pull/35), 13 untracked prototypes locally checkpointed | Prototype review and remaining full Nightly functionality. |
| Native DateTime server | Published `4198b7a9`, draft [PR #61](https://github.com/avillagran/o3-server/pull/61); prior private RPC and CI gates passed | Feature branch remains separate from main. |
| Browser source32 | 59 pending paths, exact checkpoint `efec75f6` | Not committed to the feature HEAD or published; Snapshot wrapper compilation and physical PNG/Encoder gates remain pending. |
| Auth ownership | 168 pending paths, exact checkpoint `9b1d34f6`; prior seven-case runtime passed | Not committed to the feature HEAD or published; complete provider/session integration remains pending. |
| Public SDK | Published `39b73284`, draft [PR #41](https://github.com/avillagran/lib-t3-rust/pull/41) | New UTF16 work is separate: 15 pending paths, checkpoint `3d8ac44a`, no published feature branch or new PR, no compilation yet. |
| Android | Clean `632af9fc`; 16 Git patches beyond main | No upstream or same-name remote feature branch. |

The inventory contains 22 client, 80 server, 15 SDK and one Android worktree.
Seventeen worktrees are dirty: 187 pending client versions, 342 server versions
and 15 SDK versions. These are file versions across worktrees, not 544 features.

Git patch comparison also finds client changes outside the Nightly target in
the Browser IME, reset-credit and microphone-refresh branches. The IME branch
has ten commits beyond its actual remote tip and one patch without an equivalent
in the integration target; reset-credit and microphone-refresh have one and
24 such patches, respectively. These counts are Git heuristics, not semantic
proof that every change should be merged. Historical reload/Wayland worktrees
also contain preserved WIP. Integrate reviewed deltas, rather than replacing
current files with entire older checkpoint trees.

Two current maintenance versions lacked an exact local ref. They are now saved
in `2f969804ccbd927600849ae016b8d571dd14de07`, ref
`refs/checkpoints/audit-20261108/current-maintenance-after-search`.
Another 34 external validation-tool sources, including the six accepted Auth
compiler/runtime owner scripts, are saved in `26c4c4e967166edb52e25d8b5d06e396a14679fd`,
ref `refs/checkpoints/audit-20261109/auth-external-proof-source-preservation`.
Both operations used separate indexes and preserved working files, HEAD and
the real indexes. The final source recheck found no changes to the observed cut.

## Disk allocation and cleanup candidates

Sizes below use decimal GB. They describe allocated storage, not guaranteed
reclaim. Nested paths must not be added to their parents.

| Location | Allocated / available | Interpretation |
| --- | ---: | --- |
| ROOT free | 1.96 GB | Nearly full; cleaning HOME will not free ROOT. |
| HOME free | 48.72 GB | Separate ext4 filesystem. |
| `~/BuildCache` | 528.50 GB | Main build-storage consumer; not all obsolete. |
| `~/.cache/o3-server` | 164.27 GB | Includes 134.27 GB of historical candidates below. |
| `/var/tmp/o3-build` | 50.97 GB | Mixed sources, targets and evidence; retain as a whole. |
| `/var/tmp/o3-server` | At least 51.80 GB | Mixed sources, targets and evidence; some private overlay directories were unreadable. |
| Client/server/SDK Git metadata combined | 0.142 GB | Small relative to build output; deleting branches would not address the main consumption. |

Fourteen historical Cargo cache candidates were rechecked. The eleven HOME
directories below total **134.27 GB** under `~/.cache/o3-server/`:

`checkpoint-01fa5f2`, `checkpoint-21de2b9`, `checkpoint-3eee6df`,
`checkpoint-5b031dc`, `checkpoint-7b12206`, `checkpoint-8534855`,
`checkpoint-canonical-positions`, `checkpoint-controller`, `checkpoint-d89ec19`,
`checkpoint-native-busy`, and `checkpoint-release-records`.

Three ROOT candidates total **17.37 GB**:

| Path | Allocated |
| --- | ---: |
| `/var/tmp/o3-server/live-history-target` | 13.53 GB |
| `/var/tmp/o3-server/target-release` | 1.66 GB |
| `/var/tmp/o3-build/node-release-target` | 2.18 GB |

All fourteen roots are real directories with standard Cargo layouts and valid
Cargo cache signatures. This corrects the older review's `tagValid=false`
classification. No inspected process references, active Cargo locks or direct
container mounts used them. However, 385 processes had some inaccessible fields;
historical proof references and Btrfs snapshots/extents still need to be accounted
for before removal. No current reservation has been released.

Within BuildCache, three targets alone allocate 289.90 GB:

| Target under `~/BuildCache/` | Total | Runtime banks | `debug` |
| --- | ---: | ---: | ---: |
| `o3-server/admission-integration-target-20261106` | 124.06 GB | 55 banks / 73.07 GB | 50.99 GB |
| `o3-server/fresh-context-target-20261106` | 97.63 GB | 183 banks / 62.66 GB | 34.97 GB |
| `o3-code/client-nightly-target-20261106` | 68.20 GB | 50 test banks plus 9 Electron pools / 28.21 GB | 40.00 GB |

The **297 banks/pools occupy 163.94 GB**, already included in the target totals.
All 11,672 inspected bank files were regular files with one link and no shared
inodes between banks. This is independently stored output, not hardlink savings;
content equality was not hashed. Executables, test programs and shared libraries
dominate. Some banks are referenced by historical accepted proofs, whose receipts
live elsewhere. Review terminal, superseded banks against their manifests and
reservations before retiring individual outputs. Avoid recreating a whole
runtime/dependency bank for every invocation.

The earlier GUI candidate groups allocate another 47.13 GB within BuildCache:
`o3-desktop-next/gui-glue-target` (23.90 GB), `session-target` (8.89 GB), and
66 direct `browser-gui-platform-*` fixture roots (14.34 GB). They remain review
candidates; the nested GUI copies inside `gui-glue-target` are already counted.

Android Git metadata allocates 1.26 GB. Eight large Maven/Robolectric JAR objects
appear in reflog history rather than current refs. Do not expire recovery history
as routine cleanup during this recovery task. All four repos reported zero Git
garbage objects; this does not mean every historical object is needed forever.

Current client/server/SDK targets, reserved executables, proof receipts, source
worktrees, Android images, rescue data and snapshots were retained. No cleanup,
compiler, test, runtime, feature merge or installation was performed for this audit.

## Evidence

Private reports are in `/var/tmp/o3-build/git-storage-review-tfChxyNo/`:

- `git-inventory.json`: live remote refs, all worktrees, modes, blobs and identities; SHA256 `64a23e1a3dcbf0ce14647533da2c0948d063797087f6b82378e3856bf4344eb6`.
- `final-preservation-resolution.json`: 544/544 observed versions preserved, no revalidation changes; SHA256 `544daff49ba37bd5f0d68810504d9a15e6944ccea274c1263929c2122ea3d939`.
- `storage-inventory.json`: allocation, free space, Git object counts and visibility limits; SHA256 `698b5cd9685a747870ecd2c4d8633eefa57e62abfc13391e766e46ce76e04845`.
- `historical-cache-review.json`: exact fourteen paths, sizes, signatures and process checks.
- `buildcache-bank-review.json`: bank counts, allocation, file types, link identities and proof-retention limits.
- `maintenance-preservation.json` and the separate Auth receipt in `/var/tmp/o3-build/auth-external-source-preservation-gudbcn89/receipt.json` record the new local source checkpoints.

The observed source cut precedes this documentation commit. Concurrent edits
after that cut require a new observation; no later state is silently certified.
