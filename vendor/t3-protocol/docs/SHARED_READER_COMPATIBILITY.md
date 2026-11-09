# Shared reader compatibility

The shared feature lineage retains the public reader contracts from
`7cbad5d8198d37e9ded767d33ac03f2cf15875f4` alongside the persistent RPC and
quota observation work from `cedb80cf9de3f46493e38490b96f0a553d6bc3ff`.
Both exact histories are parents of the integration checkpoint. No main branch
or private vendor patch is required.

The restored reader includes terminal observation and output state, history and
document readers, cancellable registry icons, all ABI 3 operations, the Android
presentation domain functions, unavailable quota messages, the fx descriptor,
and all 2,027 translation keys in 20 locales. Stream failure reasons, scheduled
task subscriptions and thread modes, terminal clear/close, optional fresh context,
persistent physical RPC sessions and observed reset credit IDs remain available.

Existing Rust struct literal and exhaustive enum matching contracts remain
unchanged. Use `TaskUpsertWithMode` with `upsert_with_mode` for explicit thread
modes and `CommandWithContext` with `dispatch_with_context` for optional fresh
context. Registry/JSON callers support these extensions directly. Optional
extensions preserve omission and false values without changing nested context
payload ordering. Existing Kotlin `upsert` retains its original positional and
JVM signature; `upsertWithMode` adds the explicit mode at its final parameter.

`tools/check-public-api.py` compares independently generated Rustdoc JSON inputs.
The baseline has 1,128 structural exported entries; this checkpoint has 1,162,
with no missing or changed legacy module, type, field, variant, function or
inherent method entry. The six exported C ABI symbols and original C caller are
also verified against the actual static library. Runtime fixtures cover behavior
in addition to API shape.

All runtime checks use disposable rootless containers, read-only source and
private state. Android, Kotlin and JNI compilation and execution use
`android-sandbox` exclusively. Test harnesses do not recursively remove file
system trees; disposable container teardown reclaims temporary output.

The integrated SDK runtime suite passes 414 tests across 61 native targets, with
zero failures or ignored tests. Android sandbox fixtures pass real Kotlin/JNI,
stream and Java native calls. A separate Kotlin/JVM ledger preserves all 1,155
legacy declaration/descriptor entries across generated API classes. Strict
Clippy passes with eight existing baseline lint classes qualified: an independent
baseline/candidate comparison finds the same 26 emitted warnings at 17 unique
positions and no new warnings.
