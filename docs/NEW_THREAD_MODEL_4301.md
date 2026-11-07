# Native new-thread fallback selection

Reference: T3 Code nightly4301,
`4cb6945dc60ed03f567dc5daafd083e52308e82b`.

`NativeNewThreadModel.js` retains the complete `exhaustedUsageWindow`,
`resolveNewThreadModelSelection` and provider/project enablement functions.
The canonical private oracle evaluates nominal constant IDs with the actual
contracts constructors and verifies Effect4.1.2 plus every staged source hash.
The build adapter erases types/exports, uses the equivalent
`hasOwnProperty.call` for Qt and shares the original availability predicate.
608 whole-function observations cover provider/model-specific windows, reset
boundaries, explicit selections, unavailable/uninstalled/unauthenticated or
exhausted fallback providers and global/project enablement.

New-chat creation reads one current server configuration before applying the
project defaults and fallback. The presentation boundary includes only selection
fields, provider identity/availability, auth status, the relevant window fields
and enablement flags. Credentials and arbitrary provider configuration are not
projected. The default selection and fallback come from the same final config,
including a settings change between the earlier settings read and config read.
Explicit New Local keeps its root strategy while still resolving model defaults.
Project-open creation uses the same selection helper, fences stale reads and
checks that its destination project still exists before creating anything.
Existing explicit-model and follow-up paths keep their own selection authority.

An accepted fallback creates a session-only composer notice identifying the
selected model and exhausted provider. The user can dismiss it or explicitly
restore the original model in the local composer draft. It disappears after a
run starts or the selected model changes. No inference or real provider credit
is used by the acceptance tests.

Evidence:

- `tests/new_thread_models.rs`: two private Rust cases; real HTTP-upgraded
  WebSocket settings/config reads, fresh snapshot precedence, credential-free
  projection and the actual write dispatcher. Fixtures are retained.
- `tests/qml/new-thread-model.qml`: all608 native outcomes through the generated
  Qt module, actual creation/default-setting receipt paths, root workspace,
  stale replies, automatic project opening, disappearing project and actual
  composer restore action.
- Combined all-target strict Clippy and all20 translation catalogs are checked
  separately from the full client runtime matrix, which remains gated by the
  existing transitive filesystem audit.

This checkpoint preserves the current O3 empty-server-thread creation model.
It does not implement T3's separate local-draft/sticky-provider lifecycle or its
minute-by-minute default reevaluation while a local draft remains unsubmitted.
The corpus establishes its recorded reset-date inputs; arbitrary JavaScript
Date.parse differences between V8 and Qt are not certified. These are tracked
parity gaps, not claims of complete new-thread workflow equivalence.
