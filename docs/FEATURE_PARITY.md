# UI and chat parity audit

Current comprehensive review: [screen and section audit](SCREEN_PARITY_AUDIT.md) (2026-10-17), with all 28 native route files, 168 comparison rows and fresh QML/rendering evidence. The historical implementation notes below retain their original scope.

Compared on 2026-10-16 with the local T3 Code nightly checkout at `5a4e9980`.
Plugin baseline: `213ff5b`. This is a source-backed review, expanded to the
composer, transcript, workspace controls and preferences. It does not establish
runtime or pixel-level parity. No live conversations or native settings were
modified during this audit. The reference is the local nightly checkout, not a
freshly downloaded upstream version.

## T3 Code 0.1 sync (2026-10-20)

Compared with the local nightly checkout at `fb990765` (`v0.1.1-nightly.20261020.3386`).
The native generators, fixtures and drift tests now pin that revision.

Added in this round, each with a live check against a T3 server where the
method exists on the installed server, plus QML and Rust tests:

- Protocol: archive confirmations read `/api/orchestration/shell` (the RPC
  `orchestration.getShellSnapshot` never existed); a shell subscription per
  environment refreshes the sidebar live; the thread mirror applies PR link,
  detached session, background-work, context handoff/transfer events and keeps
  MCP app references.
- Composer: effort step shortcuts, branch picker (switch, create, adopt or
  restore after drift), worktree setup progress, context window meter,
  subagent bar with stop, quote a response selection, sender attribution
  (automation, agent, T3 Code) and prompt recall that skips those messages.
- Threads: command palette (Ctrl+K) with server-wide message search, fork from
  a response, merge back, reorder pinned threads, thread links across
  environments, notifications with T3's sounds.
- Conversation: secret requests, MCP apps (host JSON-RPC bridge through
  `mcpApps.*`), Mermaid diagrams (bundled mermaid.js), QML previews (`qml`
  fences render in a sandboxed Qt process: allowed imports only, no network,
  files or launches; Omarchy colours as `omarchy.*`), LaTeX math (Qt rich
  text, behind T3's `mathRenderingEnabled`), copy tables as Markdown/CSV,
  HEIC/AVIF attachments, live command output in the tool inspector.
- Pull requests: merge/close/reopen/ready/draft, comment, approve, request
  changes, reviewers, labels, link/unlink/watch on the thread, open in a
  thread or worktree.
- Pages: usage (cost, tokens, sessions, per day and model, price refresh),
  scheduled tasks, agent session import, process diagnostics, storage cleanup
  report and run, provider install/sign-in/update, ACP registry agents,
  browser search engine, settled worktree retention, discovered dev servers,
  device settings and actions, Initialize repository.

Also added: telemetry export endpoints with checks, resource history in the
process view, live client activity reports, worktree setup progress and
Initialize repository.

Still different by design: the terminal opens in an Omarchy terminal instead
of an embedded drawer; the theme follows Omarchy instead of T3's theme editor;
T3 Connect's relay client is installed through the `t3 connect` CLI flow ot3
already drives; and ChatGPT profile import (moving Codex credentials between
environments) is left to the T3 desktop app, since ot3 never handles provider
tokens. Workflows show their script (orchestration.getWorkflowScript) and can be
stopped (orchestration.stopWorkflow) from the subagent bar.

## Recommended implementation order

1. Approval requests, older history and in-conversation search: implemented in
   the operational update below; retain their regression checks.
2. Tool inspection, per-file diffs and Plan/Default controls: complete the core
   coding conversation rather than only its status presentation.
3. Code/table/media actions and structured composer context.
4. Workspace, terminal, relationships, PR mutations and remaining preferences.

Keep large payloads and expensive renderers on demand. History needs bounded
pagination with a stable visible-message anchor, not an unbounded transcript.
Tool inspection needs a separate fetch path: the compact shared activity summary
deliberately excludes raw inputs/outputs and must not become an eager payload dump.

## Operational update: approvals, history and find

- Native approvals for commands, file reads/changes, permissions and app access,
  with provider labels/warnings, primary decisions and secondary choices. Dead
  provider requests and read-only connections cannot submit. The Rust host checks
  current pending state/options before the first dispatch, persists the command
  and replays exactly that ID/payload on explicit retry. Decisions survive reloads
  separately from the prompt. No automatic approval is introduced.
- “Load earlier messages” uses the native authenticated history endpoint and
  opaque cursor. Pages are fetched only on request, merged with live messages
  without duplicate IDs and released when switching chats. The visible message
  remains anchored after prepend. Historical pages never replace live approval,
  queue or provider state. Images and Markdown remain available in loaded pages.
- In-chat find: Ctrl+F or the header search icon; Enter/Shift+Enter and
  Ctrl+G/Ctrl+Shift+G navigate, Escape closes. Ctrl+Shift+F searches threads.
  Counts, current-message outline and selection apply to loaded message text;
  older pages must be loaded before searching them. Formatted assistant text is
  indexed without Markdown delimiters or hidden link destinations. This is not
  yet server-wide search or simultaneous highlighting of every occurrence.
- Portable approval and history helpers live in [`lib-t3-rust`](https://github.com/avillagran/lib-t3-rust) for Omi.
  The desktop still owns durable storage and transport scheduling.

Verified with isolated native HTTP/RPC integration tests, approval persistence
checks, QML click/navigation tests, history overlap/live-retention/viewport tests,
upward-scroll and send-follow regressions. No real user approval was accepted
as a test. Read-only history requests also succeeded against the real local
headless server and the inherited NODE connection. After deployment, both
environments remained ready, the local live subscription reconnected, and the
selected chat, detached window and saved draft were preserved. The T3 server
process was not restarted. Tool inspection, per-file diff controls and chat
modes remain next.

## Additional confirmed gaps from the expanded review

Native paths below are relative to `apps/web/src/` in the reference checkout.
“Partial” means an existing feature needs additional behavior, not a replacement.

| Priority | Area | Missing or partial behavior | Native evidence / plugin evidence |
| --- | --- | --- | --- |
| High | Tool details | Activity labels and expandable summary lists exist. Missing the original item inspector, fetched tool output and detailed reasoning/tool presentation. | `components/chat/MessagesTimeline.tsx` (`V2ItemInspector`, `FetchedToolOutput`); `ui/qml/T3ActivityBlock.qml`, `lib-t3-rust/src/activity.rs`, `projection.rs`. |
| High | Diffs | Changed-file summary and a colored unified diff modal exist. File rows themselves are not actionable; missing the native per-file diff panel, file navigation and richer diff controls. | `components/DiffPanel.tsx`, `components/chat/MessagesTimeline.tsx::onOpenTurnDiff`; `ui/qml/T3Changes.qml`, `T3Panel.qml::diffPopup`. |
| High | Chat modes | Model/options and provider-filtered runtime permissions work beside the composer model picker, with per-thread draft persistence and native mode updates before dispatch. No native Plan/Default mode switch or plan-ready follow-up workflow. Provider slash-command suggestions are not equivalent to dispatching built-in chat-mode actions. | `composer-logic.ts::ComposerSlashCommand`, `components/chat/ComposerPlanFollowUpBanner.tsx`, `ChatView.tsx`; `ui/qml/T3Composer.qml`, `lib-t3-rust/src/composer.rs` implements only built-in `/model`; new threads hardcode `interactionMode: default` in `src/write.rs`. |
| Medium | Composer references | File/folder and skill lookup exist. Missing thread/PR suggestion kinds, rich context chips, assistant-selection citations and structured T3 clipboard contexts. The editor currently inserts textual Markdown references. | `components/chat/ComposerCommandMenu.tsx`, `ComposerPromptEditorTiptap.tsx`, `chat/AssistantSelectionToolbar.tsx`; `src/suggestions.rs` allows only path/skill/command, `ui/qml/T3Composer.qml`. |
| Adapted | Project drafts/stash | Multi-entry project-scoped save/restore/delete and attachments are now implemented, shared between fullscreen and Edit right-click. Native floating stash badge/banner presentation differs. | `components/chat/ComposerStashMenu.tsx`, `ComposerStashBadge.tsx`; `ui/qml/T3ProjectDrafts.qml`, `T3Composer.qml`, `T3Backend.qml`; see current audit row 41. |
| Medium | Workspace controls | Missing full branch/worktree picker, Git action controls, project-script runner/editor and editor picker inside the chat details UI. “New thread on this branch” already exists and should be preserved. | `components/chat/ThreadDetailsPanel.tsx`, `components/BranchToolbar.tsx`, `GitActionsControl.tsx`, `ProjectScriptsControl.tsx`; `ui/qml/T3SidebarActions.qml`, `T3Panel.qml`. |
| Medium | Terminal | Native Omarchy terminal now attaches to the chat’s T3 shell (local or remote), with history, input and resize support. Embedded terminal drawer and terminal-selection context in the prompt remain unported. | `components/ThreadTerminalDrawer.tsx`, `components/chat/ComposerPendingTerminalContexts.tsx`; `src/terminal.rs`, `lib-t3-rust/src/terminal.rs`, `docs/NATIVE_TERMINAL.md`. |
| Medium | Relationships and automations | Lineage metadata exists in the normalized sidebar data, but no full parent/fork/subagent relationship panel or thread automation editor. | `components/chat/ThreadRelationshipsControl.tsx`, `ThreadAutomationsPanel.tsx`; `src/lib.rs` preserves lineage, `ui/qml/T3Panel.qml` has no corresponding panel. |

The Markdown/media/message-action gaps below remain valid. Sidebar hover actions,
thread/project context menus, working elapsed time, shelves, minimap, queues,
tasks, questionnaires, image copy/preview, embedded file/HTML preview, model
favorites and preferences are already present; they are not newly missing
features. Existing implementations still need visual/interaction regression
checks when adjacent components change.

## Implemented in this update

| Interaction | Omarchy behavior | Verification |
| --- | --- | --- |
| Copy message | Icon action below user/assistant text; copies original text/Markdown, not Qt-generated HTML. Hidden while the message is streaming. | QML action and production Rust process tests |
| Copy image | Hover/focus action and right-click menu on thumbnails, plus a button in the enlarged viewer. Fetches the same authenticated native attachment, including remote environments. | QML viewer/context-menu tests; production cache/clipboard test |
| Copy feedback | Pending state, success check and localized failure with error detail. Success follows clipboard ownership confirmation. | Forced process failure and QML result tests |
| Clipboard image format | PNG sent as image bytes; JPEG/GIF/WebP converted to PNG on demand in Rust. Animated formats copy their first frame. | All four formats decoded and checked in Rust tests |

The desktop host owns Wayland clipboard access. Attachment acquisition continues
to use the shared T3 protocol; no clipboard monitoring, GUI server or JavaScript
backend is introduced. Decoding happens only on copy, with bounded input, pixel
count and decoder allocation. Clipboard ownership uses the installed `wl-copy`.
The integration tests replace it in a private environment.

## Preferences and connection lifecycle

The sidebar gear opens an embedded preferences view with General, Appearance,
Source Control, Connections, Providers and Keyboard shortcuts. General exposes supported native
update, resume, auto-settle, snooze, streaming and background-profile settings.
Appearance controls existing panel preferences and inherits the Omarchy theme.
Connections manage saved native routes (see below); Providers expose status and native model
favorites. Source Control ports branch naming, worktree defaults, merge method,
writing style/templates and review-guide preferences. General includes optional
inactivity days and automatic resume after rate limits. Preferences use dedicated
sidebar navigation, grouped cards, compact themed controls and a responsive
content column. Search covers the supported native settings across categories.
Text edits save explicitly
and remain staged on failure. Settings target the selected environment through native RPC. This is
a partial port: provider credential/configuration editing,
editable keybindings, source-control host credentials and writer/review model
selection, advanced project/storage/integration and
diagnostic panels remain to be ported. Native reference: `routes/settings.*.tsx`
and `components/settings/SettingsPanels.tsx`.

Inherited SSH targets now start a stopped native remote service before opening
the tunnel. Active servers are reused. Isolated production-script tests cover
running/stopped/missing services and profile/environment mismatch; live validation
uses the existing NODE server without interrupting its sessions.

## Confirmed remaining gaps

| Priority | Native T3 behavior | Current plugin gap | Source evidence |
| --- | --- | --- | --- |
| High | Markdown-authored image references resolve to interactive media previews. | Native image attachments work, but `rendered_markdown` strips Markdown image tags to alt text; those images need the authenticated media pipeline and actions too. | `ChatMarkdown.tsx` / `src/lib.rs`, `rendered_markdown` |
| High | Code blocks have their own copy action, language header and expand control. | Per-block language/copy and explicit single-line shell execution now work through the native terminal. Expanded code view, syntax highlighting and line-wrap controls remain unported. | `ChatMarkdown.tsx`, code frame and `handleCopy` |
| High | Tables can be copied as Markdown or CSV. | Tables render through Qt rich text; there is no table action menu. | `ChatMarkdown.tsx`, `Copy as Markdown` / `Copy as CSV` |
| High | Media context menu offers Save image/video and source-aware path/URL actions. | This update adds Copy image; Save/download and full media context menus remain absent. File preview is already embedded. | `media/MediaActions.tsx` |
| Medium | A user-message action copies a shareable prompt link carrying repository/provider/model context. | There is no prompt-link action or structured composer-context clipboard flavor. | `chat/MessagesTimeline.tsx`, `promptLinks/copyPromptLink.ts` |
| Medium | Edit from here reverts to an earlier checkpoint before resuming. | No user-message checkpoint/revert action; ordinary draft editing is implemented. Requires the native revert workflow and file-state handling. | `chat/MessagesTimeline.tsx`, `RevertUserMessageButton` |
| Medium | Assistant copy is attached to the terminal response and understands Codex directives. | Copy operates on each non-streaming textual message; terminal-response grouping and directive-specific clipboard conversion need a separate port. | `chat/MessagesTimeline.logic.ts`, `resolveAssistantMessageCopyState` |
| Medium | Mermaid fences render diagrams with source/diagram and expand controls. | Mermaid is currently rendered as a plain code fence. | `ChatMarkdown.tsx`, `MarkdownMermaidCodeBlock` |
| Medium | Video attachments have an inline player and media actions. | Images have previews; other attachments use file-reference actions, without an inline video player. | `media/MediaVideoPlayer.tsx`, `MessagesTimeline.tsx` |
| Medium | PR workflows include mutations and stack-aware authoring/review actions. | Existing PR list/details/checks/activity/diff are read-only; no review/comment/merge workflow. | `pullRequest/` compared with `ui/qml/T3PullRequests.qml` |

Native sources above are relative to `apps/web/src/components/` in the nightly
checkout. Plugin references: `T3Panel.qml`, `T3MessageImage.qml`,
`T3CopyButton.qml`, `src/clipboard.rs`, and `tools/test-copy.cjs`.

## ot3 interaction update

- Existing-folder projects now use an embedded environment → source → folder
  picker based on native `filesystem.browse` and `filesystem.getMetadata`.
  Paths are validated on the selected server, including inherited SSH hosts.
  No OS folder dialog is opened. New-repository creation and hosted-repository
  discovery/clone sources remain separate parity work.
- Ctrl+Enter sends prompts and questionnaire answers. Enter keeps multiline
  editing. The desktop integration binds only the default-agent shortcut; it
  no longer unbinds or intercepts Omarchy’s Super+Enter terminal shortcut.
- Window control cycles panel → floating → tiled → panel, preserving the same
  workspace/editor/draft/subscription. The close action is the rightmost header
  control. Managed layout is scoped to ot3’s verified PID/title/address and
  compares current floating state before dispatching, so repeated application
  cannot toggle an already-correct window. Saved mode migrates old detached state.

Validated with real Hyprland floating/tiled/panel tests, embedded-picker flow
checks, native RPC fixtures for remote folder validation/project creation,
read-only browsing against local and NODE, Ctrl+Enter regressions and installer
isolation. New projects were not created in live user data during validation.

## Saved connection management

Preferences now manages the native encrypted T3 connection catalog: SSH and
direct/hosted pairing-link onboarding, endpoint/SSH target editing with environment
identity checks, labels, enabled state, preferred routes, reconnect, route removal
and environment removal. Disabled entries remain visible with their credentials
intact. Removal confirms before deleting this device’s credentials; remote chats
and projects are not deleted. Local connection availability and the existing
headless service controls are included. GUI-owned servers cannot be stopped here.

Catalog operations preserve unrelated routes, credentials and unknown fields,
use an optimistic revision check, keep an encrypted previous-file backup, and
retain the native encryption version/key. QML sees sanitized route metadata, not
saved bearer tokens; pairing input is transient. SSH onboarding runs independently
of sends and draft saves. Unchanged SSH tunnels survive catalog label edits.
Portable transformations and pairing-link normalization live in t3-protocol.

The selected environment now also exposes native authorized clients, pairing
links and explicit permission selection, individual/bulk revocation, version,
network reachability, supported environment icons and linked T3 Connect
activity/webhook preferences. See [connection access](CONNECTION_ACCESS.md) for
contracts and validation. Local network exposure and Tailscale HTTPS use native
launch settings and an ownership-checked, confirmed service restart. Native CLI
account authorization, browser account creation, relay installation approval,
managed/publish-only setup, unlink and logout are integrated; CLI authorization
is separate from the Electron Clerk session (see [cloud adapter](CONNECTION_CLOUD.md)).
Native balancing preferences now route eligible automatic drafts to the matching
project/environment; GitHub sharing preferences guard cross-environment PR reads
(see [connection preferences](CONNECTION_PREFERENCES.md)). SSH key/passphrase
enrollment remains in the native application. Tests use isolated encrypted
catalogs, fake native processes, mocked HTTP endpoints and QML forms. Live smoke
checks only read state; they do not change account, exposure or sharing settings.

## Composer background send and project sources

The composer has a **Send and start new thread** action beside Edit,
with **Ctrl+Shift+Enter** in its tooltip. Composer and header thread creation
read T3's native `defaultThreadEnvMode` before launching. If unset, one modal
asks to Enable worktrees, Use same folder, or Cancel. Approval saves the chosen
native environment default through `server.updateSettings`; subsequent threads
and app restarts read that setting and launch without another confirmation.
Existing project overrides take precedence, and native
`newWorktreesStartFromOrigin` is applied. Read/write failures keep the draft and
offer Retry without creating a thread. The setting is shared with original T3,
not stored as a separate local approval flag.

Worktree mode uses native `orchestration.launchThread` with
`workspaceStrategy: {type: "worktree", baseRef: <source branch or HEAD>}`;
T3 allocates the new branch and worktree. Same-folder mode explicitly uses
`root` or `existing_worktree`, retaining the source branch/path.

Composer launches pass `initialMessage` to the same native call. T3 durably
accepts the prompt and defers execution until workspace preparation completes;
there is no independent send that could run in the wrong folder. The source
draft clears only after native acknowledgement. Draft save failures and RPC
errors retain the prompt and exact launch/message identities for explicit retry.
Legacy persisted create-then-send attempts retain their previous recovery path.
Worktree creation does not enable automatic merging. Integration remains in
native T3 Git/pull-request workflows; those controls are not yet ported here.

Verified against local native source `packages/contracts/src/orchestrationV2.ts`
(`OrchestrationV2ThreadLaunchInput`, workspace strategy and result),
`apps/server/src/ws.ts` launch handler, and
`apps/server/src/orchestration-v2/ThreadLaunchService.ts` (receipts,
`defer_start`, workspace preparation and generated branch).

Add Project reads each selected environment's native configuration and source
control discovery. It offers Local folder, New project when `newProjectsRoot`
is available, Git URL, and the official six provider sources. Unavailable
providers are marked as requiring setup. Creation uses `projects.createNew`;
cloning uses `projectClone.start` and requires clone tracking support. Native
provider authentication and clone transport settings stay on the T3 server.
The folder picker filters names immediately, debounces directory changes,
and supports Enter and arrow-key navigation entirely inside the ot3 window.
Acknowledged creation receipts are durable; an unacknowledged non-idempotent
creation is not blindly replayed.

Validation uses synthetic RPC servers and isolated QML state, including failed
sends and visible creation dialogs in panel, floating and tiled modes. Live user
repositories were not created or cloned. This does not yet port the optional
publish-new-project toggle, all source-palette suggestions or clone-progress UI.

### Inline HTML pages

Native `html_render` attachments render automatically inside the conversation with
an interactive Chromium surface; they retain the native measured content height
(up to 1800 px) and can expand into the embedded document modal. Author CSS takes
precedence over theme defaults, and native theme notifications reach the page.
Each visible page has an independent isolated renderer, bounded to four inline
sessions plus the modal. Pages outside the viewport, hidden panels and thread
changes release their processes. Late frames cannot revive a closed page or
close another renderer. Chromium repaints drive frame delivery, including delayed
JavaScript changes, without continuous captures while idle. No WebEngine library
loads into Quickshell or the persistent Rust backend.

Validation: `tools/test-qml inline-html`, `tools/test-qml documents`,
`tools/test-document-viewer`, and `cargo test --release --test documents --bin
omarchy-t3-client`. The real tooltip HTML attachment was also read through native
asset grants and rendered with three independent sessions; no user prompt or
project command was executed.

Project draft drawer: fullscreen editor and edit-button right-click share one
QML component. Ports native GUI stash/restore rules with project scope, includes
pending prompts from other project threads, preserves target model/permissions
and attachments, and saves the destination before consuming a source. See
[PROJECT_DRAFTS.md](PROJECT_DRAFTS.md).

Archive screen follow-up (2026-10-17): [native grouped archived threads](ARCHIVE.md) is implemented in Preferences, with native scope, Unarchive/Delete, confirmation preference and durable explicit retry/status handling. Local/node archive reads passed; live mutations and original-app pixel comparison remain unverified. Other administration gaps remain in the comprehensive screen audit.


Native keybindings follow-up: editable grouped commands/conditions, native key-file
operations, original primary/active runtime resolution, change subscriptions and
supported chat/script dispatch are installed. Missing workspace commands and
alternate-layout/IME/compositor matching remain partial; see [KEYBINDINGS.md](KEYBINDINGS.md).

Open-source licenses follow-up: General → Diagnostics → View licenses now exposes
the application manifest with native filtering/counts/source links and one lazy,
selectable notice.238locked component/asset/compiler notices are collected at
build time; reads need no native account or environment role. The General link,
actual-worker notice reads and error/stale/retry behavior are tested. See
[LICENSES.md](LICENSES.md); five other dedicated administration pages remain.
