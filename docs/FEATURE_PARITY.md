# UI and chat parity audit

Compared on 2026-10-16 with the local T3 Code nightly checkout at `5a4e9980`.
Plugin baseline: `213ff5b`. This is a source-backed review, expanded to the
composer, transcript, workspace controls and preferences. It does not establish
runtime or pixel-level parity. No live conversations or native settings were
modified during this audit. The reference is the local nightly checkout, not a
freshly downloaded upstream version.

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
- Portable approval and history helpers live in `crates/t3-protocol` for Omi.
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
| High | Tool details | Activity labels and expandable summary lists exist. Missing the original item inspector, fetched tool output and detailed reasoning/tool presentation. | `components/chat/MessagesTimeline.tsx` (`V2ItemInspector`, `FetchedToolOutput`); `ui/qml/T3ActivityBlock.qml`, `crates/t3-protocol/src/activity.rs`, `projection.rs`. |
| High | Diffs | Changed-file summary and a colored unified diff modal exist. File rows themselves are not actionable; missing the native per-file diff panel, file navigation and richer diff controls. | `components/DiffPanel.tsx`, `components/chat/MessagesTimeline.tsx::onOpenTurnDiff`; `ui/qml/T3Changes.qml`, `T3Panel.qml::diffPopup`. |
| High | Chat modes | Model/options work, but runtime permissions are a read-only label. No native Plan/Default mode switch or plan-ready follow-up workflow. Provider slash-command suggestions are not equivalent to dispatching built-in chat-mode actions. | `composer-logic.ts::ComposerSlashCommand`, `components/chat/ComposerPlanFollowUpBanner.tsx`, `ChatView.tsx`; `ui/qml/T3Composer.qml`, `crates/t3-protocol/src/composer.rs` implements only built-in `/model`; new threads hardcode `interactionMode: default` in `src/write.rs`. |
| Medium | Composer references | File/folder and skill lookup exist. Missing thread/PR suggestion kinds, rich context chips, assistant-selection citations and structured T3 clipboard contexts. The editor currently inserts textual Markdown references. | `components/chat/ComposerCommandMenu.tsx`, `ComposerPromptEditorTiptap.tsx`, `chat/AssistantSelectionToolbar.tsx`; `src/suggestions.rs` allows only path/skill/command, `ui/qml/T3Composer.qml`. |
| Medium | Prompt stash | Durable draft and prompt history exist, but there is no multi-entry stash menu to save/restore/delete independent prompts and their attachments. This is separate from queued messages. | `components/chat/ComposerStashMenu.tsx`, `ComposerStashBadge.tsx`; `ui/qml/T3Composer.qml`, `T3Backend.qml`. |
| Medium | Workspace controls | Missing full branch/worktree picker, Git action controls, project-script runner/editor and editor picker inside the chat details UI. “New thread on this branch” already exists and should be preserved. | `components/chat/ThreadDetailsPanel.tsx`, `components/BranchToolbar.tsx`, `GitActionsControl.tsx`, `ProjectScriptsControl.tsx`; `ui/qml/T3SidebarActions.qml`, `T3Panel.qml`. |
| Medium | Terminal | No embedded terminal drawer or terminal-selection context in the prompt. Tool activity labels do not provide an interactive terminal. | `components/ThreadTerminalDrawer.tsx`, `components/chat/ComposerPendingTerminalContexts.tsx`; no equivalent QML component or terminal transport. |
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
Connections show inherited status; Providers expose status and native model
favorites. Source Control ports branch naming, worktree defaults, merge method,
writing style/templates and review-guide preferences. General includes optional
inactivity days and automatic resume after rate limits. Preferences use dedicated
sidebar navigation, grouped cards, compact themed controls and a responsive
content column. Search covers the supported native settings across categories.
Text edits save explicitly
and remain staged on failure. Settings target the selected environment through native RPC. This is
a partial port: connection editing, provider credential/configuration editing,
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
| High | Code blocks have their own copy action, language header and expand control. | QML rich text renders code, but has no equivalent per-block controls or expanded code view. | `ChatMarkdown.tsx`, code frame and `handleCopy` |
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
