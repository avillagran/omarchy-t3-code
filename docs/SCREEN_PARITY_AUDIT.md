# T3 Code → ot3: complete screen and section audit

Reviewed **2026-10-17**. **ot3 does not yet have full functional or visual parity.**
All **28 native route files** were inventoried, including layouts, redirects and conditional entry points. Detailed source review records **168 comparison rows**: 83 settings, 57 chat/sidebar/composer/workspace, 28 auxiliary/right-panel/PR. These are grouped comparisons, **not 168 independent screens** or a percentage score. Several rows intentionally discuss the same cross-cutting workflow from different surfaces.

Native reference: local checkout `5a4e9980533705774dc3f45b35cacf76c3d78dbc`. Target: current working tree, including existing uncommitted changes. Installed native web assets identify as `0.0.46-nightly.20261013.3042`; screenshots illustrate that installed build and do not prove the pinned checkout has identical pixels. No upstream fetch was performed.

## Results and detailed matrices

- [Settings: 83 comparison rows](audit/SETTINGS_PARITY.md): every settings leaf route, scope/search and Usage.
- [Chat and workspace: 57 comparison rows](audit/CHAT_PARITY.md): sidebar/menus, projects, conversation/renderers, approvals, questions, queue/tasks, composer/editor/drafts, models/modes, worktrees, details and terminal.
- [Other surfaces: 28 comparison rows](audit/SURFACES_PARITY.md): onboarding/auth, app coordinators, browser/file/device panels and PR workflows.
- [Route inventory JSON](audit/routes.json), [native runtime observations](audit/native-runtime.json), [dark captures](audit/native-dark-captures.json).

The settings tree has **14 leaf routes**. ot3 exposes **nine leaf-route counterparts** (eight navigation categories plus the licenses drill-down), with partial coverage; **five have no dedicated equivalent page**: Integrations, Scheduled Tasks, Storage, Diagnostics and SnapShots. Projects now has scoped settings, while full project-management sections remain partial. Native categories and controls can be conditional on platform, scope or capability; the audit does not assume all are visible in every installation.

## Implementation follow-up

The current source adds native default model/options, permissions and workspace selectors to General, with shared composer controls and native project-default resolution for new threads. Settings sidebar/card geometry and section order are closer to native. Multi-target environment/project/checkout axes, mixed/source metadata, inherited reset and explicit partial-write reports are now implemented for these server fields; native inheritance-chain popovers and omitted controls remain pending.

Native tool details are fetched only after activity activation, preserving inherited source identity; per-file checkpoint review has file navigation, unified/split hunks and whitespace refetch. Isolated and panel-level QML tests cover rendering, source scope, stale results and close. These are source/test results; the earlier screenshot set predates this batch. See [inspector contract](TOOL_INSPECTOR.md) and [diff contract](DIFF_REVIEW.md) for exact remaining differences.

Installed follow-up validation: release build and 18 focused Rust integration tests (5 diff, 9 inspector, 4 settings), plus one grouped-source projection test passed. Serialized QML checks: `diff-review`, `tool-inspector`, `activity-inspector`, `work-details`, `preferences-defaults`, `preferences-design`, `connection-routing`, `composer`, `send-new-thread`. Translation catalog now 20 locales × 618 keys; no warnings. Plugin validation and whitespace checks passed. Scoped plugin update backed up to `/tmp/ot3-before-coding-parity-3ghfve5g`; 13 draft hashes, selected thread and open panel preserved; local/node ready. Read-only checks against the running native backend also passed: tool item available, three default settings fields returned, and a 39-file checkpoint diff parsed without retaining the raw patch. No account/server-setting/message/Git writes were exercised in these checks.

Scoped-settings follow-up is installed and verified: five Rust scope tests plus native settings/runtime-model checks passed; QML `preferences-scope`, `project-preferences`, `preferences`, `preferences-defaults`, `preferences-design`, `preferences-source-control` and `project-menu` passed serially. Live read-only local environment and project queries each returned 27 fields without errors and with the actual settings-write permission. The UI capture `/tmp/ot3-preferences-scoped.png` was inspected. Catalog: 20 locales × 639 keys, no warnings. Backup `/tmp/ot3-before-scoped-settings-eu7m68lp`; 13 drafts, selection and open panel preserved, both environments ready. No live server-setting writes were made. See [scope contract](SCOPED_SETTINGS.md).

Plan/Build follow-up is installed: native client opt-in with CAS, provider/permission gating, draft mode/shortcuts, historical plan cards and ready follow-up. New-thread implementation uses native metadata-create → durable dispatch with sourcePlanRef, retains the original project draft, and navigates after acceptance. Six Rust suites (21 tests) and serial QML workflow/preferences regressions passed; the loopback verifies mode-before-send and unchanged retry IDs/payload. Spanish fixture capture `/tmp/ot3-plan-flow.png` inspected. Catalog20locales ×694 keys warning-free. Backup `/tmp/ot3-before-plan-workflow-va7rcwht`;13draft hashes/selection/open panel preserved, local/node ready. No live user message, Plan opt-in, file restore or Git mutation was made. Live provider Plan behavior and exact native visual parity remain unverified. See [Plan contract](PLAN_WORKFLOW.md).

Git/scripts follow-up is installed: original-thread scoped native status/settings, actual auth permissions, confirmed commit/push/pull/PR/composite actions and resolved script execution. `git.runStackedAction` uses the native Effect stream and correlated terminal events; private atomic per-action/checkout journals precede dispatch and prevent duplicate replay, including older actions after newer ones. Unknown outcomes retain the checkout guard across navigation/restart; explicit native-T3 reconciliation releases it without replay. Scripts open/write the exact native terminal/env and attach the existing external frontend to that session; delivery is distinguished from script completion. Ten Rust tests and serial QML Git/panel/chat/terminal/PR/reconnect regressions pass. Local and one available node checkout pass live native reads; another node thread returns rpc_rejected, cause unproven and no root-path fallback. Native Git/script mutations were tested only with isolated loopback/stub fixtures. Catalog20locales ×697keys, no warnings. Final backup `/tmp/ot3-before-git-controls-knyjv88a`;13draft hashes/selection/open panel retained, local/node ready. Checkout patch review is integrated below; adaptive header, branch/setup and script administration remain partial. See [Git contract](GIT_CONTROLS.md), [Spanish Git fixture](audit/screenshots/ot3-git-controls-fixture.png), [Plan fixture](audit/screenshots/ot3-plan-flow-fixture.png); these fixtures are not a native pixel comparison.

User-message checkpoint editing is installed with the native confirmation/keep-files choice, original environment/thread durable guard, exact checkpoint.rollback, explicit same-ID uncertain replay, correlated native completion/failure and once-only latest draft+attachment recovery. Preflight downloads native signed assets and creates fresh uploads before rollback. Private draft markers prevent stale autosaves; late older operations cannot replace a newer thread guard. Native turn-item intent is joined correctly (conversation messages do not carry it), and reverted-run rows disappear on read-back. Confirmation closes after a choice, progress/recovery remains by the composer. Focused26Rust tests and12serial QML regressions pass;20locales ×700keys/no warnings. A real settled project thread returned a valid checkpoint preview through the running native backend; no actual rollback/file restore was executed. Final backup `/tmp/ot3-before-checkpoint-revert-xe73733v`;13draft hashes/selection/open panel preserved, local/node ready. See [checkpoint contract](CHECKPOINT_REVERT.md), [Spanish fixture](audit/screenshots/ot3-checkpoint-confirm-fixture.png). Real provider rollback and exact native pixels remain unverified; this does not establish complete screen parity.

## Most consequential gaps

1. **Complete coding workflows:** native multi-file diff layout/collapse/search/source selection, branch/setup and script administration still need implementation. Checkpoint editing is integrated, with real provider rollback still unverified. Plan/Default and ready-plan follow-up are now implemented; live provider verification remains. Tool inspection and per-file review now work through native read RPCs; richer tool-specific layouts/highlighting and review annotations remain partial. Native diff layout persistence and initial defaults are integrated.
2. **Provider and project administration:** provider configuration/authentication, full project administration, source-control credentials, editable keybindings. Favorites/status and Add Project do not cover these pages.
3. **PR authoring/review:** comment/review forms, inline annotations, merge/update/stack actions and full filtering. ot3 currently supplies native **read-only** PR views.
4. **Application workspaces:** a native local-browser workspace is now mounted in the worktree (not installed): tabs, address/search, profiles, history controls, zoom and responsive inline/sheet presentation. Shared file/device surfaces, server streaming, automation, recording, full project browser history/discovery and complete menus remain pending. Interactive HTML documents remain separate.
5. **Advanced administration:** schedules/webhooks, retention/cleanup, diagnostics and usage history. Existing per-provider quota bars are not a complete cost/token analytics page.

Already implemented and preserved: inline interactive HTML, fullscreen composer, project drafts that load into the current thread, existing-project picker, native worktree-default confirmation and launch, access/network/CLI Connect/balancing/GitHub permissions, approvals/questions/queues/tasks and external native terminal attachment. Their remaining differences are recorded explicitly rather than relabeling them absent.

## Visual comparison and order of correction

Palette, fonts, scaling, desktop chrome, stronger shelf persistence and the requested Ctrl+Enter behavior are intentional Omarchy/user adaptations. They should remain. Missing interaction flows and layout differences are separate findings.

| Surface | Native hierarchy / design | Current ot3 difference | Closest-design target |
|---|---|---|---|
| Settings navigation | Search in sidebar, complete ordered category tree, back at bottom | Search in sidebar; eight categories with incomplete content; back near top | Match information architecture, search placement and order as pages become implemented. Avoid empty imitation pages. |
| Settings scope | Project/checkout and environment scope sentence, mixed/inherited/reset affordances | Explicit environment/project/checkout axes with mixed/source badges and inherit reset | Preserve scope and inheritance semantics before reproducing the chrome. |
| Settings content | Compact section headers, row label + description, consistent trailing controls | Large page heading, mostly label-only rows; different group/order and much less information | Restore native group/row hierarchy and explanations, then align compact control dimensions. |
| Settings controls | Scoped defaults, inline source/reset hints, well-defined three control sizes | Generic switches/selects/fields, no per-row reset/source UI | Reuse common QML controls with native tiered sizes and focus/hover/error states. |
| Chat transcript | Comfortable lane, content-sized80% user bubble, outside metadata | Core geometry now close; long-text fold, rich block controls and renderer layout differ | Preserve anchoring/streaming; add block-specific media/code/table interactions. |
| Sidebar | Card/slim rows in one sortable list; native status/drag/bulk semantics | Card/slim appearance close; independent shelf scrollers, expanded counts and simplified attention states | Keep requested collapse/empty-shelf rules; improve semantic states and workflows before structural scroll changes. |
| Thread details | Workspace/Git/scripts/relationships/automation panel | Compact header and delayed hover detail | Introduce the actual sections and responsive panel; hover metadata alone is not equivalent. |
| PR review | Dense scoped list and Summary/Guide/Timeline/Code workspace | Simpler list with Overview/Checks/Conversation/Code and read-only patch | Match content/interaction hierarchy, per-file Code and review forms before micro-spacing. |
| Browser/files/devices | Shared tabs, persistent selected surfaces, resizing and actions | Source now has a native browser-only workspace with responsive presentation and core controls; file/device/mixed surfaces remain incomplete | Preserve the native shell and separate HTML renderer; finish the remaining workspace capabilities. |

Captured comparisons (native vs ot3, both dark):

- General: [native](audit/screenshots/native-settings-general-dark.png), [ot3](audit/screenshots/ot3-settings-general.png).
- Appearance: [native](audit/screenshots/native-settings-appearance-dark.png), [ot3](audit/screenshots/ot3-settings-appearance.png).
- Source Control: [native](audit/screenshots/native-settings-source-control-dark.png), [ot3](audit/screenshots/ot3-settings-source-control.png).
- [ot3 chat](audit/screenshots/ot3-chat.png) and [narrow settings](audit/screenshots/ot3-settings-narrow.png).

Screenshots have different physical dimensions/DPR, languages, theme tokens and fixture data. These pairs are **structural visual evidence, not a pixel-diff benchmark**. The Source Control reference includes native loading skeletons; credential/discovery completion was not verified. Screenshots cover first viewports, not every scroll position/conditional dialog. All remaining subpanels were assessed from source, with that evidence level stated in the matrices.

## Verification executed in this audit

**31 existing QML scenarios passed**, exercising the real components with isolated/fake bridges or fixtures: `preferences-design`, `visual-parity`, `preferences`, `preferences-source-control`, `connection-access`, `connection-network`, `connection-cloud`, `connection-preferences`, `connection-routing`, `pull-requests`, `creation-dialogs`, `send-new-thread`, `project-open`, `project-drafts`, `editor`, `approvals`, `questions`, `tasks`, `queue`, `model-picker`, `thread-menu`, `project-menu`, `content-panels`, `inline-html`, `documents`, `history-find`, `sidebar-navigation`, `sidebar-pr`, `provider-subagent`, `copy-actions`, `native-terminal`.

Native runtime: **19 route visits** in a disposable HOME/T3 data instance bound only to loopback port 18773, with unconfigured providers. Pairing used only that temporary instance's one-time token. Public/invalid authorization, empty, loading and platform-unavailable states are identified in the observations. No real agent prompt, review, merge, login, network/settings mutation or project creation was performed against user data. Only the temporary browser origin's appearance changed to capture dark mode. Native browser click timeouts were traced to stacked notifications covering the theme control; after dismissing those notices, the observed control worked. The alternate IAB surface was unavailable. No production plugin update was required for this read-only review.

Translation validation passed:20 locales × 524 keys, no warnings. Plugin validation and `git diff --check` passed. The Rust build/tests from previous implementation work were **not rerun or counted as fresh evidence** in this audit. Tests establish the named existing behaviors, not omitted functionality or account-dependent end-to-end equivalence.

## All native route files

Status below is a **route-level summary**; partial means reachable related functionality, not full parity. Layouts/redirects are included for inventory completeness and are not counted as distinct full screens.

| Native route file | Route / entry | ot3 status | Finding |
|---|---|---|---|
| `__root.tsx` | `__root (layout)` | Adapted/partial | Omarchy shell/window lifecycle; app-wide auth/notice/right-panel coordinators differ. |
| `_chat.$environmentId.$threadId.tsx` | `/_chat/$environmentId/$threadId` | Partial | Main transcript/composer implemented; details/Plan/tool-output/diff gaps documented. |
| `_chat.draft.$draftId.tsx` | `/_chat/draft/$draftId` | Adapted/partial | Durable per-thread/project drafts and fullscreen editor; no identical native draft URL route. |
| `_chat.index.tsx` | `/_chat/` | Adapted/partial | Restores panel/selected thread rather than the identical native recent-project draft landing. |
| `_chat.new.tsx` | `/_chat/new` | Partial | New thread and native worktree launch; prompt-link import and full creation controls differ. |
| `_chat.pull-requests.tsx` | `/_chat/pull-requests` | Partial | Read-only list/detail/checks/conversation/patch; review/merge/stack workflows missing. |
| `_chat.tsx` | `/_chat` | Adapted/partial | Persistent panel and selected thread; no equivalent URL/router and full right-panel workspace. |
| `connect-agent.tsx` | `/connect-agent` | Missing | No external-agent authorization landing; chat approvals are a different feature. |
| `connect.tsx` | `/connect` | Adapted | Official browser device authorization delegated from native CLI; Electron Clerk is separate. |
| `pair.tsx` | `/pair` | Adapted/partial | Embedded Add Connection native pairing rather than a standalone web landing. |
| `projects.$projectKey.tsx` | `/projects/$projectKey` | Partial | Native redirect to project settings; ot3 project popup does not supply the full destination page. |
| `settings.appearance.tsx` | `/settings/appearance` | Adapted/partial | Omarchy theme/font; native layout, typography overrides and previews differ. |
| `settings.archived.tsx` | `/settings/archived` | Implemented core | Native grouped archive query and scoped Unarchive/Delete, native confirmation preference and durable commands; live writes and exact native pixels unverified. |
| `settings.connections.tsx` | `/settings/connections` | Partial | Native catalog/access/network/CLI Connect/balancing/sharing present; Electron discovery/auth and enrollment differ. |
| `settings.diagnostics.tsx` | `/settings/diagnostics` | Missing | No resource, process, trace or export panel. |
| `settings.general.tsx` | `/settings/general` | Partial | Native default model/options, permissions and workspace controls added; scoped environment/project/checkout editing added; many omitted behavior/client controls remain partial. |
| `settings.integrations.tsx` | `/settings/integrations` | Partial | Native browser defaults/profiles/import and device settings exist; complete runtime/automation and visual parity remain open; see INTEGRATIONS.md. |
| `settings.keybindings.tsx` | `/settings/keybindings` | Partial | Native grouped editor, recording, scoped persistence and runtime routing exist; unsupported commands and full native pixels remain partial; see KEYBINDINGS.md. |
| `settings.open-source-licenses.tsx` | `/settings/open-source-licenses` | Implemented/adapted | General drill-down, original search/key/bundle helpers,238actual app notices, count/source/one lazy selectable expansion/retry. Qt/theme/data differ; see LICENSES.md. |
| `settings.projects.tsx` | `/settings/projects` | Partial | Scoped Projects/Checkouts settings page and direct sidebar entry added; complete project-management/scripts/cleanup sections remain pending. |
| `settings.providers.tsx` | `/settings/providers` | Partial | Status/favorites; no full provider editor or authentication setup. |
| `settings.scheduled-tasks.tsx` | `/settings/scheduled-tasks` | Missing | No task list, editor, delivery or webhook UI. |
| `settings.snap-shot.tsx` | `/settings/snap-shot` | Missing/conditional | No SnapShots setup/settings; native functionality also depends on desktop/platform support. |
| `settings.source-control.tsx` | `/settings/source-control` | Partial | Many native defaults; no source-control host credential/account UI. |
| `settings.storage.tsx` | `/settings/storage` | Missing UI; backend in progress | Typed native rules, capability guards and scope-preserving updates now exist; complete page and cleanup run/report controller remain missing; see STORAGE_SETTINGS.md. |
| `settings.tsx` | `/settings` | Partial | Ten leaf counterparts (nine navigation categories plus licenses), several incomplete; native tree has14 leaf routes. |
| `usage.tsx` | `/usage` | Partial | Quota strip implemented; standalone cost/tokens/limits history/dashboard missing. |
| `welcome.tsx` | `/welcome` | Partial | Project/connection setup exposed separately; no complete onboarding/import wizard. |


Checkout review follow-up: native `review.getDiffPreview` now powers both Uncommitted and Changes sources, explicit base ref, fresh remote checkout/file admission, complete file statistics despite global patch truncation and lazy per-file patch reads. The Git popup enables review with filesystem-read permission, including clean checkout branch review. Reuses `T3DiffReview` rather than a second patch viewer. Native source/global and per-file live reads pass; isolated HTTP/Effect and panel fixtures cover late scopes and draft preservation. Native base-ref selector now uses the upstream pairing/filtering algorithm and paginated VCS reads. Checkout Pierre rendering/full-file expansion is integrated below; historical checkpoint rendering is integrated below; review annotations remain partial; see [Git contract](GIT_CONTROLS.md).


Base-ref follow-up: upstream local/remote pairing/filtering is ported with MIT attribution. Search, origin preference, remote switches, automatic base and independently paginated native refs now replace free-form base input. Actual-panel fixture and live native local/remote reads pass; native pixel comparison remains unverified. Catalog20locales ×708keys, warning-free. See [selector contract](GIT_CONTROLS.md).


Native renderer follow-up: checkout patches now use the actual pinned Pierre1.5.2 component in the existing sandboxed Qt renderer, with syntax tokens, native context expansion from authorized full-file snapshots and split/wrap changes on the same instance. No network or privileged RPC channel is exposed to the page; large HTML stays outside Quickshell's text model. Real Qt interaction and ordinary authored-HTML regressions pass. Native live added/modified file reads render correctly; modified-file full context and fresh hash/scope checks pass. Historical checkpoint rendering and native layout/defaults are integrated below. Full multi-file layout/collapse/search, annotations and full app/pixel comparison remain pending. See [renderer contract](GIT_CONTROLS.md).


Historical renderer/preferences follow-up: native checkpoint ranges now share Pierre with exact thread/range/hash revalidation and orchestration-read admission. Native T3 supplies no checkout full-file loader for historical runs, and ot3 mirrors that behavior. A shared range token supports many files without per-file cache exhaustion; path-preserving refetch/expiry and binary/metadata fallback are tested. Native layout persists with fresh CAS while wrap/whitespace remain local, initialized from native defaults. Real Qt and actual13→14native checkpoint reads pass. Full multi-file/collapse/search/source selection, annotations, administration and full app/visual verification remain pending. See [historical contract](DIFF_REVIEW.md).

Archive follow-up: [native archived-thread screen](ARCHIVE.md) now provides project grouping, relative dates, virtual rows, environment/project/checkout filtering, Unarchive and right-click Unarchive/Delete. Native deletion confirmation preference is respected. Commands are journaled before dispatch; ACK is not completion, uncertain actions use explicit same-command retry and read-only status observation. Release Rust and serial QML checks passed; live archive reads on local/node returned no archived rows. No live deletion/restoration was performed. The remaining five dedicated administration pages and full original-app visual comparison remain pending.

Keybindings follow-up: [KEYBINDINGS.md](KEYBINDINGS.md) records the native editable grouped screen, original helper reuse, recursive conditions, exact native key/file operations and multi-target partial-result Qt tests. It is now installed with original primary/active precedence, supported runtime actions, native change subscriptions, translated command labels/search and script shortcuts through the existing journaled pipeline. Actual Qt keyboard and read-only native stream checks pass;13drafts/native preferences/selection/open preserved. Unsupported workspace commands, alternate-layout physical matching and full native pixels remain partial; this does not prove complete shortcut parity.


Licenses follow-up: General → Diagnostics → View licenses now opens the native
style application notice screen, using original search/key/bundle helpers.
The compiled ARM64 manifest contains238entries for locked Rust/renderer
components, compiler-supplied standard-library notices and exact copied asset
notices. Loading/error/retry/count/source links and one selectable lazy expanded
notice are implemented; app reads require no account/environment permissions.
Rust and isolated actual-worker QML tests pass. Scoped installation preserved
13drafts/native preferences/selection/closed-panel state; both environments ready.
Full native pixel equality is not
claimed; see [license contract](LICENSES.md). Integrations is now a partial source-only page; Scheduled Tasks,
Storage, Diagnostics and SnapShots remain dedicated missing pages.

Browser workspace update: [BROWSER_RUNTIME.md](BROWSER_RUNTIME.md) records source-only mounting in the main QML panel, native session/process/status ownership,69 original URL/search fixtures, scoped controls and Qt route tests. Its prototype capture is `/tmp/ot3-browser-workspace.png`; this is an isolated state fixture, not a live installed comparison or a complete visual-parity claim. Server stream, full desktop broker/CDP, source-browser import and missing native menu/device/history/recording flows remain open.

Profile cleanup update: native Qt and server cleanup are now coordinated in source. The local profile reservation spans graceful process flush, owned storage deletion, remote acknowledgements and metadata CAS; cold profiles and partial/unknown outcomes are covered by isolated tests. See [BROWSER_RUNTIME.md](BROWSER_RUNTIME.md). This feature remains uninstalled and does not close other browser or screen parity gaps.

Import wizard update: the source-only native QML wizard now reuses original pure flow/failure helpers, with confirmed one-use worker-to-Qt import and post-verification profile registration. Isolated source-to-HTTP/worker/controller and actual tile/narrow-layout tests pass. Packaging and original visual comparison remain open; existing-page DOM/history retention now passes real Qt tests, while import completion-latency parity is still open; see BROWSER_RUNTIME.md.

Storage follow-up: the native settings foundation now matches original worktree policy resolution and partial-write completion, with original resolver fixtures and isolated native RPC readback. The complete Storage page, cleanup report/run UI and translations remain missing. See [storage contracts](STORAGE_SETTINGS.md).
