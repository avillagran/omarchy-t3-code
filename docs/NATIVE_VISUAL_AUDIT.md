# Native T3 chat and sidebar rendering audit

Reviewed 2026-10-17 against the local `t3code-nightly` source checkout at
`5a4e9980533705774dc3f45b35cacf76c3d78dbc`. This is a
source-backed rendering and interaction review, not a claim of pixel identity
between Chromium/React and Qt Quick. Omarchy owns palette, fonts and scale.

## Source map

- `apps/web/src/components/ChatView.tsx`: conversation/composer overlay and inset.
- `apps/web/src/components/chat/MessagesTimeline.tsx`: row hierarchy, user
  bubbles, work logs, metadata visibility, media and long-message folding.
- `apps/web/src/components/chat/MessagesTimeline.logic.ts`: projection and folds.
- `apps/web/src/components/ChatMarkdown.tsx`: Markdown, links, files, code and media.
- `apps/web/src/index.css`: lane width, typography, Markdown and sidebar tokens.
- `apps/web/src/components/Sidebar.tsx`: card/slim variants, status priority,
  hover actions, project and branch metadata, pins, drafts, shelves and order.
- `apps/web/src/components/Sidebar.logic.ts`: unread/receding/status semantics.
- `apps/web/src/components/sidebar/SidebarThreadHeader.tsx`: search and actions.
- `apps/web/src/components/ui/collapsible-section-header.tsx`: shelf geometry.
- `apps/web/src/components/ProjectFavicon.tsx`: project icon priority.
- `packages/client-runtime/src/state/threadSettled.ts`: snooze countdown.

Paths above are relative to the native T3 checkout, not this plugin.

## Chat

| Area | Native behavior | O3 result |
| --- | --- | --- |
| Width | Centered 46rem comfortable lane; 72rem wide/full are client preferences | Default 736px lane now centered, shared with composer, scaled by Omarchy. Wide/full preference synchronization remains separate. |
| User bubble | Right aligned, max 80%, content-sized, 12px padding | Corrected from fixed 88%; short prompts shrink and long ones cap at 80%. |
| Metadata | Timestamp/actions outside user background; assistant metadata beneath body | Corrected; controls retain layout but reveal on hover/focus/feedback. |
| Author | Accessibility heading without a visible author row | Accessible names retained; no redundant visible row. |
| Row spacing | Work has no external padding, commentary 8px, terminal/user 16px | Corrected; no global 22px list gap, compact 24px command line. |
| Commentary | No terminal metadata row | Corrected using adjacent projected rows; see projection limitation below. |
| Paragraphs | 0.65rem block spacing; first/last edges trimmed | Explicit Qt-compatible styling, including first/last root block margins. |
| Typography | 1.5 line height; 1.3 for headings; explicit heading scale | Explicit stylesheet replaces Qt defaults; theme font and scale retained. |
| Lists | 20px indentation, 4px item separation, nested markers | Explicit supported styling; task-list controls/advanced marker behavior remain different. |
| Inline code | Smaller mono text, muted surface | Explicit mono size and surface. Qt does not reproduce CSS rounded inline borders. |
| Code block | Compact toolbar, independent code body, copy/run, syntax colors | Compact cells, mono body and surface implemented; existing safe copy/run preserved. Syntax highlighting and horizontal code-wrap toggle remain unported. |
| Tables | Header hierarchy, row separators, compact cells, horizontal overflow and expansion | Explicit supported table typography/cell styling; Qt rich text still wraps rather than providing native independent horizontal scrolling/expansion. |
| Quotes | Muted text, left indent/border | Supported muted/indent styling; native CSS border treatment is not pixel-identical. |
| Links | GitHub brand, web favicon/globe, file icons, no permanent underline | GitHub/file/thread/web glyphs, visible separator and no permanent underline. Generic web remains offline globe. |
| Copy/search | Source Markdown; decorations are excluded from search metadata | Preserved and tested against actual selection positions. |
| History | Virtualized rows, retained viewport, prepend older history | Existing anchoring and pagination retained; regression tests run. |
| Prompt overlay | Timeline travels underneath prompt; bottom inset preserves final row | Retained and tested after geometry changes. |
| Streaming | Incremental content without resetting user scroll | Existing stable delegate update/tail policy retained. Native one-shot block fade not added. |
| Media | Measured aspect ratios, author/user variants, signed native resources | Existing private signed image/HTML pipeline retained; fixed thumbnail frames still differ from native aspect-ratio grids. |
| HTML | Inline interactive page with expansion | Existing isolated renderer retained; no browser loaded into shell. |
| Changes | Native per-file review and diff affordances | Existing summary/embedded diff retained; detailed per-file navigation remains separate. |
| Context | Compaction/handoff boundary with provider/model identities | Existing divider retained and tested. |
| Long user text | Native collapsible 176px body, reveal selected find match | Not yet ported; text remains fully visible, preserving search access. |
| Rich extensions | Mermaid/math, alerts, footnotes, citations, inline context chips | Not all ported. Qt Markdown supports the existing safe subset; no untrusted HTML enabled. |
| Working detail | Native grouped expandable work/thinking rows | Existing command groups preserved; raw tool-output inspection remains a larger projection feature. |

## Sidebar

| Area | Native behavior | O3 result |
| --- | --- | --- |
| Active/Working | 78px card plus 4px wrapper | Retained 82px. |
| Snoozed/Settled | Single 36px slim row | Corrected from 82px full cards; icon, title, PR and time/actions share the row. |
| Project/title | 16px project icon; title in full card, one title in slim row | Corrected slim layout, native size and medium title weight. |
| Icon overrides | Monogram/emoji/Lucide override before favicon and generated identity | Overrides now render in the sidebar using bundled official Lucide assets. Native signed favicon retrieval remains used by notifications; sidebar favicon images still fall back to identity. |
| Read state | Done marks unseen completion, not every completed thread | Corrected: read completions show relative time. |
| Working state | Static dashed circle; background work recedes except selected/attention rows | Corrected icon and emphasis; no continuous spinner on every sidebar row. |
| Input/approval | Distinct native status labels/icons/colors | Attention keeps priority and a question icon; normalized request data does not yet expose every native approval/input distinction. |
| Draft | Pen and subtle surface only on another thread with unsent text/files | Corrected selected-thread marker; draft protection and discard semantics retained. |
| Pin | Persistent marker, unpin when capability allows | Added visible marker and scoped unpin control. |
| Branch/worktree | Middle elision; worktree marker; scratch machine fallback | Existing behavior preserved and tested. |
| PR | Badge remains visible and clickable during hover, including slim rows | Added slim badge slot; shared native routes retained. |
| Snoozed time | Countdown until return, not last activity | Corrected countdown; hover offers Wake rather than Settle. |
| Settled time | Settlement/history timestamp | Corrected timestamp source. |
| Surface | Hover/selected/focus; parked history recedes | Existing theme surfaces retained, with keyboard ring and revised emphasis. |
| Shelves | 32px label, inline rule, chevron | Corrected geometry. User's stronger collapse persistence and hide-empty rules retained. |
| Shelf counts | Native hides the count on expanded sections | O3 continues showing counts in expanded headers; this small label difference remains. |
| Keyboard | One traversal order including shelf markers | Existing Ctrl+K, arrows, expand/collapse retained and tested with slim rows. |
| Hover details | Delayed, scoped detail; closes on scroll/pool/hide | Preserved; actual pointer/quick-action regression coverage included. |
| Search | Search field and compact project/new controls | Clear button and Escape-to-clear added; existing filter/new actions retained. Native result snippets/inline rename are not all ported. |
| Ordering | Native activity anchors and pinned precedence | Existing portable ordering retained. |
| Scroll topology | One ordered list, bottom shelves via flex spacers | O3 keeps separate virtualized shelf viewports. This is an architectural difference, not just spacing; no replacement of the persisted per-shelf anchors in this pass. |
| Pointer workflows | Drag reorder, multiselect, sweep actions, file drops | Not all ported; context menus and safe individual actions remain. |
| Wake/terminal/provider history | Woke indicators, live terminal controls, stacked provider history | Not all available in the plugin's current snapshot/presentation contract. |
| Branding/footer | Native desktop chrome, update/account controls | Omarchy branding, connection controls, quotas and locale picker intentionally remain desktop-plugin specific. |

## Limits and preservation

A native completion's final/commentary distinction is derived from its projected
row/run metadata. The desktop currently uses adjacent timeline rows. That
heuristic is preserved; it does not claim to replace native attempt/turn folds.

This audit does not mark deferred behaviors as implemented. In particular,
Chromium-only Markdown layout, richer native event projection and a unified
sidebar scroller need their own implementations and regression coverage.

User-specific rules take precedence over native defaults: collapsed shelves must
stay collapsed across selection/search/reopen, empty shelves stay hidden, and
Settled threads never inflate the bar widget counters. No repository/chat state
is mutated to obtain a visual match.

Bubble width uses `FontMetrics.advanceWidth`, not a hidden Text item. The hidden
measurement introduced a layout feedback loop with a loaded long history in a
layer panel; fixed-width A/B isolation and the history prepend/search regression
confirmed the font-metrics solution. Fullscreen composer retains full width.

## Verification in this pass

28 focused QML checks passed across the changed surfaces: `visual-parity`,
`markdown-links`, `timeline-spacing`, `compact-sidebar`, `sidebar-navigation`,
`sidebar-pr`, `thread-menu`, `thread-hover`, `project-menu`, `context-divider`,
`code-actions`, `copy-actions`, `composer-overlay`, `send-scroll`, `live-media`,
`history-find`, `panel-reopen`, `window-cycle`, `layout`, `editor`, `project-drafts`,
`provider-subagent`, `inline-html`, `work-details`, `interaction`, `i18n`,
`scroll-stability` and `upward-scroll`. Window-cycle uses the active Hyprland
instance, and validates actual compositor state. Translation checks cover
20 locales × 406 keys; plugin validation and whitespace checks pass.

Synthetic captures were inspected for the current desktop theme, including a
260px sidebar and a wider chat window. These are geometry/interaction tests and
manual rendering checks, not automated pixel comparisons against Electron or
proof of every light/high-contrast/font configuration. The exhaustive source
matrix above explicitly records the remaining engine and feature differences.
