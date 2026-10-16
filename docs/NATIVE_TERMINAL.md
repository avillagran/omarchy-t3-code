# Native terminal

Use the terminal icon in the chat header or **Ctrl+J**. Ot3 opens the terminal
configured by Omarchy and attaches it to T3's primary terminal (`term-1`) for
that chat. Commands run in the selected T3 environment, in the chat's worktree
or project directory. Remote environments use the existing T3 connection;
no separate SSH host, password or terminal configuration is required.

Closing the native window detaches the frontend. It does not close T3's shell
or terminate its jobs. Reopening restores the server's terminal history. Use
`exit` to end the shell. Opening an exited shell starts it again. A connection
failure ends the frontend instead of replaying input or reconnecting silently.
Read-only connections cannot launch terminals.

The native emulator supplies selection, copy/paste, scrollback, fonts and the
Omarchy theme. When launched from the overlay panel, ot3 hides the panel so it
does not cover the terminal; detached ot3 windows remain open. The chat draft
is preserved in both cases. This terminal is independent of the agent's tool
executions and permission selector. Project scripts and sending a terminal
selection as structured prompt context remain separate parity work.

## Implementation

The reusable `lib-t3-rust::terminal` module resolves the native thread context,
builds attach/write/resize payloads and handles history sequence deduplication.
The desktop Rust frontend forwards terminal output, UTF-8 input, Ctrl+C and
window size changes. It restores termios and cancels the subscription on exit.
There is no embedded browser terminal, JavaScript backend or local execution
of remote commands. Credentials remain in the inherited T3 connection and are
never included in terminal launcher arguments.

Reference: `t3code-nightly` at `5a4e9980`,
`packages/contracts/src/terminal.ts`, `rpc.ts`, and
`apps/web/src/components/ThreadTerminalDrawer.tsx`.

Validation: isolated PTY/WebSocket integration covers snapshot/live output,
replay deduplication, split UTF-8 input, Ctrl+C, resizing, normal shell exit,
frontend termination/detachment and termios restoration. QML checks cover the
header button, Ctrl+J, duplicate launch prevention, failures, late replies,
draft preservation and read-only gating. These fixtures do not execute commands
in a live user project or establish end-to-end LAN/emulator validation.
