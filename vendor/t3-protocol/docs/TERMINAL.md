# Android terminal bindings (0.3.7)

ABI remains 3. No C or JNI symbols or handle ownership rules change. The existing
JSON `domain` and typed `subscribe` operations expose these additions in the AAR:

- `T3Api(client).domain.terminal.context(shell, threadId, fallback)` resolves the
  selected server's worktree/project/provider context without local filesystem access.
- `domain.terminal.attach(threadId, terminalId, context, cols, rows, resume)` builds
  validated attach arguments. Initial explicit open uses restartIfNotRunning=true.
  Reconnect uses resume=true and restartIfNotRunning=false, preserving exited shells.
  Pass the result to `client.subscribeFn("terminal.attach", args)`.
- `domain.terminal.output(event, state)` wraps Rust `terminal::Output`. Pass returned
  state back for each ordered event. `text=null` means no renderer write; `replace`
  means reset renderer history/parser before applying snapshot text. `finished`,
  `exitCode`, `exitSignal` and `error` expose stopped/error observations. A restarted
  snapshot resets finished; error returns its state as well as terminal_remote_error.
- `api.terminal.observe(threadId, terminalId)` subscribes to T3's existing
  `terminal.observe`, with terminal:read. It observes existing sessions without
  creating, restarting, resizing or writing them. This is a distinct wire operation.

`terminal.attach` **still requires terminal:operate**, including attachment for
observation. No permission is weakened or substituted. Read-only hosts should use
`terminal.observe`; older servers lacking that method must report the failure.

Reconnect/backoff and stream cancellation remain host-owned. Subscribe again to the
same host/thread/terminal; replace history from the new snapshot. Sequence is an
ordering/deduplication marker, **not an afterSequence replay cursor** for terminals.
A transport closure is not a PTY exit. Keep state separate per host/thread/terminal;
discard it when ownership changes. Do not automatically retry writes.

Wire output is JSON Unicode text, not raw PTY bytes. Transport UTF-8 decoding already
handles WebSocket boundaries. These bindings preserve the complete string, including
ESC, OSC, control characters and Unicode, without trimming, stripping or truncating.
ANSI sequences may span events: the host must feed text in order to a persistent
terminal parser, never parse each chunk independently. No second UTF-8 decoder or
ANSI buffering is introduced here. Rendering, scrollback and input/IME stay host-owned.

Contract source: t3code@3f9448747, packages/contracts/src/terminal.ts and rpc.ts;
apps/server/src/ws.ts and auth/RpcAuthorization.ts. terminal.observe uses
TerminalObserveInput and TerminalAttachStreamEvent, terminal:read authorization.

Regenerate saved describe/Kotlin with:
`T3_UPDATE_FIXTURES=1 cargo test --test api_describe`.
No tag or release is created by this change; release CI builds the four-ABI AAR.
