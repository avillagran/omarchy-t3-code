# Project registration and navigation

The sidebar represents projects through their threads. Registering a folder alone
therefore leaves the project invisible. After Add Project succeeds, ot3 now waits
for the environment's project snapshot and opens an existing active thread, or
creates and selects its first empty thread. This does not send a prompt, start an
agent or create a worktree. The destination environment supplies the model choice.

`projects.mutate` returns a Project with an `id` field, whereas name-only creation
and tracked cloning return `projectId`. The desktop response normalizes both.
The UI also supports older desktop binaries whose Add Project acknowledgement
omitted the ID. Selecting a registered folder reopens it instead of registering
another project. A failed first-chat operation stays visible and can be retried
without repeating project creation.

The picker uses Omarchy colors for its fields, compact navigation rows, focus,
hover and confirmation buttons. Sources scroll within the dialog; folder names
filter immediately and directory changes are debounced.

Regression coverage: `tests/project_creation.rs`, `tools/test-qml project-picker`,
`tools/test-qml project-open`, and `tools/test-qml creation-dialogs`.

Add Project also lists registered projects immediately, before browsing or
registering a folder. The list shows title, workspace path and environment, and
matches every search token against those fields, following the native command
palette's project action/search metadata. The initial multi-environment screen
includes projects from all ready environments; choosing an environment scopes the
list. Selecting a row calls the existing scoped project navigation flow, without
another registration. Navigation failures remain visible and allow retry. Fresh
openings reset the search and scroll position. Creation and clone sources remain
available below the project list.
