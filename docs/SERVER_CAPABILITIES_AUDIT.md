# Server capability audit corrections

Client base remains 0e82d6e. Vendor and Cargo pin now use lib-t3-rust
6e17bb2276da5aee0912fbe084bc024819f65899 (0.3.8), based on origin/main
1498ad6, preserving gzip and brotli.

Upserts distinguish explicit rpc_rejected from uncertain transport outcomes.
Diagnostics remain sanitized. Local payload validation happens before dispatch.
The editor creates one command identity when opened and sends that same identity
on retries. Both servers use commandId for idempotent task creation (T3 conclusion
is source-only, nightly 680152e27b2a961686389ca69a1557cbb2c55f08,
ScheduledTaskService.ts resolved id; o3-server scheduled.rs command receipts).
Closing and opening a new editor begins a new operation; there is no automatic
mutation retry. A loopback WebSocket test drops both responses and verifies two
attempts use the same commandId and return outcome-unknown.

Task-list responses resolve a relative webhook path against the selected
Environment's trusted origin, checking the resolved origin before returning it
to QML. An existing absolute T3 URL is retained. The displayed URL remains
read-only and selectable.

Evidence: /var/tmp/o3-build/client-independent-audit/desktop-behavior-red.log,
desktop-green.log, desktop-clippy.log and desktop-vendor.log. The behavior
regression reinstates the old random-identity/error mapping/URL behavior before
running the new test, then restores the fix. Owner checkouts and Obsidian were
untouched. No live server acceptance or actual signed delivery was attempted.

The compression repin adds native crates to the desktop license manifest. The
existing lock-hash regression first failed. The notice collector also found
that alloc-stdlib omits its workspace license file; it now imports the exact
BSD notice from its resolved alloc-no-stdlib dependency, requiring matching
repository and SPDX metadata. Regenerated manifests include all 272 entries.

Validation: 524 distinct tests covered successfully across the full `cargo test`
run and corrected/remaining target reruns. The full run stopped at stale license
metadata; the corrected notices test and every later target passed separately.
The transport header test was updated from identity-only to gzip/brotli offers.
`cargo clippy` passes with existing warnings. Optional `--all-targets` still
fails on four pre-existing `unused_io_amount` findings in tests/scoped_settings.rs.
No GUI interaction or live delivery was verified.
