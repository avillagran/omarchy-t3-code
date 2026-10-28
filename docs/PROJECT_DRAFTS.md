# Project prompt drafts

The drawer ports T3 Code's native `ComposerStashMenu.tsx`,
`promptStashStore.ts` and `ChatComposer.tsx` stash/restore behavior. Those stores
are GUI-local browser storage, not a server RPC. O3 uses its existing private
Rust draft storage, with project scope and an adapter for pending composer
prompts in existing threads. It does not read or alter the Electron GUI's browser
storage.

Expanding the editor opens project drafts. Right-clicking the edit button opens
the same `T3ProjectDrafts` component instance inside the compact chat composer.
It lists saved project prompts and pending prompts from every thread in the
selected environment/project, including settled threads. Arrow keys and Enter
select an entry; Escape closes the drawer. The bookmark-plus button saves the
current prompt as an independent project entry. Delete removes the chosen entry.

As in native T3, restoring appends to any current text with a blank line, restores
attachments and keeps the destination model/provider and execution permissions.
It never selects another thread or sends a prompt. A pending thread prompt moves
to the current composer only after the destination is durable. Questionnaires,
approvals and send retry identities remain with their original threads; prompts
with an uncertain submission, queue edit or unresolved attachment are locked.

The project stash keeps up to 20 entries, matching native T3's cap. Unlike native
browser eviction, a full stash rejects another save and preserves the current
composer. Private atomic files and a durable transfer journal preserve both
prompts if the process stops between destination persistence and source removal.
A repeated request does not append twice. Opening a composer recovers an
interrupted transfer in its project before loading its draft. Conflicting newer
edits stop a stale transfer rather than clearing its source.

Tests: `cargo test --release --test prompt_stash --test drafts`,
`tools/test-qml project-drafts`, `tools/test-qml editor` and
`tools/test-qml draft-undo`. These use isolated state and never submit a prompt.
