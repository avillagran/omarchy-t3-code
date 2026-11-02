# Indexed sidebar projection

`sidebar.view` keeps its existing full response by default. `compact: true` opts
into `format: indexed-v1`: `threads` contains each presented row once, and each
section contains `[threadIndex, overlayObject]`. Apply `rowDefaults` to absent
row properties except names in `$absent`, remove `$absent`, then overlay section
properties on the referenced row. This restores exactly the full response,
including nesting fields and nulls. Readers can share an unmodified row between
the section and thread collections. The Android reader restores its public API.

The compact projection admits at most 1,000 aggregate threads and 4 MiB of
serialized environment input, and bounds output to 8 MiB. Oversized projections
fail explicitly with `sidebar_presentation_limit`, without truncation. Android
also caps shell HTTP/WebSocket snapshots at 4 MiB; desktop transport stays at
64 MiB. A byte cap alone cannot bound enrichment of many short rows.

These are conservative host-tested admission bounds, not ART heap certification.
Larger profiles need a native retained-shell API with bounded windows, section
references, output budgets and revisions, avoiding full shell round trips. That
API should retain existing full readers while allowing Android to page rows.
