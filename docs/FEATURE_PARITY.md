# Chat interaction parity audit

Compared on 2026-10-16 with the local T3 Code nightly checkout at `5a4e9980`.
This is a source-backed review of message, Markdown and media interactions, with
one adjacent PR workflow. It is not a claim of whole-application parity.

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
Connections, Providers and Keyboard shortcuts. General exposes supported native
update, resume, auto-settle, snooze, streaming and background-profile settings.
Appearance controls existing panel preferences and inherits the Omarchy theme.
Connections show inherited status; Providers expose status and native model
favorites. Settings target the selected environment through native RPC. This is
a partial port: connection editing, provider credential/configuration editing,
editable keybindings, advanced project/storage/source-control/integration and
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
