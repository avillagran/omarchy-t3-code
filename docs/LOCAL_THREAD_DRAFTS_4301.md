# Local thread drafts and first-send lifecycle

Source reference: T3 Code nightly4301,
`4cb6945dc60ed03f567dc5daafd083e52308e82b`. The existing published shared SDK
`d4bd7e525fca4a620d79cbd4f97bcf3dcbff472b` remains unchanged.

## Product behavior

New chat creates local metadata and a separately persisted composer document.
It does not create an empty server thread. A cached project is sufficient to
open and edit a draft offline, including before provider configuration arrives.
Sending is unavailable while that environment is disconnected. Missing model
configuration does not fabricate a provider selection.

An empty draft in the project can be reused after its document has loaded.
Authored text, attachments and context prevent reuse. Explicit model choices
survive reuse, restarts and routing. Sticky provider/model selections and
model-specific options are persisted separately from project defaults.
Configured project defaults, carried selection, sticky state and usage fallback
are evaluated at creation. The existing native minute clock reevaluates cached
fallback windows offline; expiration removes the notice without replacing an
already selected model or authored content. Connected drafts may refresh their
configuration through the existing typed settings APIs.

The local draft ID, reserved future thread ID and launch command ID have
separate lifetimes. The composer is saved before the first launch is submitted.
An unknown outcome retains the exact attempt and command ID. A known rejection
restores the editable document; a later attempt gets a new command ID while
retaining the future thread ID. Acknowledgements from other attempts cannot
promote or clear the active document. Accepted background creation does not
steal another selected route.

Automatic environment routing saves the source and then the destination before
moving the local index. Edits during either acknowledgement cause the latest
source content to be saved again. The source recovery document is retained.
A durable local owner prevents replacing an unrelated destination document.
Project removal or a changed selection epoch fences a pending transfer.
Attachments bound to the source environment prevent automatic relocation.

## Native wire boundary

`useComposerSend.ts` supplies an internal bootstrap object to the client-runtime
`startThreadTurn` operation. The whole operation in
`packages/client-runtime/src/operations/commands.ts` consumes it and emits the
existing `orchestration.launchThread` command with workspace strategy and an
initial message. Bootstrap is not an extra message-dispatch RPC field. O3 uses
the published typed `ThreadLaunch` API; no new bootstrap RPC or private SDK
extension is necessary.

The contracts declare optional `bootstrapThreadDisposition` failure metadata,
but the pinned source has no server producer of that field. This implementation
does not infer rollback or deletion from error text, invent server metadata, or
replace an uncertain future thread ID. Later support for a genuine structured
producer must preserve this distinction.

## Validation boundaries

`generate-local-thread-draft-fixtures` executes whole pinned native functions
with actual Effect and contracts: 48 start-thread traces,36 provider/default
cases,8 sticky-store actions and72 workspace/default observations. Source and
runtime dependency hashes are recorded; canonical schema identity is checked.
The generated Qt provider module is derived from those pinned source bodies.
These are selected whole-function observations, not a claim that every branch
of the full React new-thread handler has been reproduced.

Focused private Rust tests cover persistence/restart, ownership, typed launch
wire identity and workspace observations. Private QML fixtures use the actual
backend/composer adapter for first-send acknowledgement races, offline first
configuration, the real wall-minute boundary and asynchronous routing. The
separate actual-server runner snapshots verified immutable executables and runs
QML, the real client worker and a credential-free synthetic Codex peer inside
an isolated container. It never starts a paid model or accesses a live profile.
No test removes a filesystem tree; evidence and recovery files are retained.

The client-wide runtime filesystem audit remains a separate gate. Unloaded
project thread counts, creation with an unknown/unloaded project, arbitrary V8
versus Qt Date.parse differences, and absent native structured rollback producers
remain qualified boundaries. Existing browser, device and terminal capability
claims are unchanged.
