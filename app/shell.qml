//@ pragma OmarchyLoadPatch
//@ pragma AppId io.github.avillagran.omarchy-t3-code
// Omarchy T3 Code as a desktop app, run by bin/ot3 in its own Quickshell
// process (never inside the Omarchy shell). The optional bar widget
// (ui/qml/BarWidget.qml) only shows the status this app publishes and asks
// bin/ot3 to show or hide the window, so a stuck bar cannot take the app down.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import qs.Commons
import "ui/qml"
import "ui/js/SidebarModel.js" as Sidebar

ShellRoot {
  id: app
  // No hot reload: an update under a running app waits for `ot3 restart`.
  settings.watchFiles: false
  readonly property var backend: T3Backend
  readonly property var counts: Sidebar.counts(backend.state.threads)
  readonly property bool opened: panel.opened
  readonly property bool focused: !!(panel.managedToplevel && panel.managedToplevel.activated)
  readonly property string statusPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/omarchy-t3-code/status.json"
  readonly property bool bridgeIndicator: backend.viewState.omabridgeIndicator === true
  property bool quitting: false

  T3Panel { id: panel; bridge: app.backend; standalone: true; panelAvailable: !!app.anchor; anchorItem: app.ghostAnchor; bar: ghostBar }

  // Panel mode, as when ot3 lived in the bar: the bar widget sends where its
  // button is (per screen), and a transparent, click-through twin of that bar
  // carries the anchor the popup (PinnedKeyboardPanel) is placed under.
  readonly property string anchorsPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/omarchy-t3-code/anchors.json"
  property var anchors: ({})
  property string anchorScreen: ""
  property real anchorSetAt: 0
  readonly property var anchor: anchors[anchorScreen] || anchors[(Hyprland.focusedMonitor || {}).name] || anchors[Object.keys(anchors)[0]] || null
  function setAnchor(json) {
    var a; try { a = JSON.parse(json) } catch (e) { return }
    var num = function(v) { return typeof v === "number" && isFinite(v) && v >= 0 && v < 100000 }
    if (!a || typeof a.screen !== "string" || !a.screen || ["top", "bottom", "left", "right"].indexOf(a.position) < 0
        || !["barSize", "windowWidth", "windowHeight", "x", "y", "width", "height"].every(function(k) { return num(a[k]) })) return
    var next = Object.assign({}, anchors); next[a.screen] = a; anchors = next; anchorScreen = a.screen; anchorSetAt = Date.now()
    anchorsFile.setText(JSON.stringify(next))
  }
  FileView { id: anchorsFile; path: app.anchorsPath; blockWrites: true; printErrors: false
    onLoaded: { try { var a = JSON.parse(text() || "{}"); if (a && typeof a === "object") app.anchors = a } catch (e) {} } }
  // The keyboard opens the panel on the focused monitor's bar when it has one.
  function anchorForKeyboard() { var m = (Hyprland.focusedMonitor || {}).name; if (m && anchors[m]) anchorScreen = m }
  // One twin per screen: a layer surface stays on the output it was mapped on.
  Variants {
    id: ghosts
    model: panel.windowMode === "panel" ? Object.keys(app.anchors) : []
    PanelWindow {
      id: ghostWindow
      required property string modelData
      readonly property var a: app.anchors[modelData] || ({ position: "top", windowWidth: 0, windowHeight: 0, x: 0, y: 0, width: 0, height: 0 })
      readonly property bool vertical: a.position === "left" || a.position === "right"
      readonly property Item anchorItem: ghostAnchor
      screen: Quickshell.screens.find(function(s) { return s.name === ghostWindow.modelData }) || null
      color: "transparent"
      exclusionMode: ExclusionMode.Ignore
      WlrLayershell.layer: WlrLayer.Background
      WlrLayershell.namespace: "ot3-bar-anchor"
      mask: Region {}
      anchors { top: a.position !== "bottom"; bottom: a.position !== "top"; left: a.position !== "right"; right: a.position !== "left" }
      implicitWidth: vertical ? Math.max(1, a.windowWidth) : 1
      implicitHeight: vertical ? 1 : Math.max(1, a.windowHeight)
      Item { id: ghostAnchor; x: ghostWindow.a.x; y: ghostWindow.a.y; width: ghostWindow.a.width; height: ghostWindow.a.height }
    }
  }
  readonly property var ghostAnchor: { var g = ghosts.instances.find(function(w) { return app.anchor && w.modelData === app.anchor.screen }); return g ? g.anchorItem : null }
  // Moving to another screen's bar while open: close and reopen there.
  property string shownScreen: ""
  onAnchorChanged: {
    var screen = anchor ? anchor.screen : ""
    if (panel.opened && panel.windowMode === "panel" && shownScreen && screen !== shownScreen) { panel.close(); Qt.callLater(panel.open) }
    shownScreen = screen
  }
  // What PinnedKeyboardPanel asks of the bar: its edge, size and popout coordinator.
  QtObject {
    id: ghostBar
    readonly property string position: app.anchor ? app.anchor.position : "top"
    readonly property real barSize: app.anchor ? app.anchor.barSize : 0
    property var activePopout: null
    property var clickTargets: []
    function requestPopout(key) { activePopout = key }
    function releasePopout(key) { if (activePopout === key) activePopout = null }
  }
  onOpenedChanged: { backend.setPanelVisible(opened); publish.restart() }
  Component.onCompleted: publish.restart()

  // A show or toggle right after the bar sent its anchor comes from that bar.
  function fromKeyboard() { if (Date.now() - anchorSetAt > 1500) anchorForKeyboard() }
  function show() { fromKeyboard(); panel.open() }
  function hide() { panel.close() }
  // Super+Shift+A and the bar: a focused window hides, any other state brings it up.
  // The panel has no window focus to go by: it simply opens or closes.
  function toggle() {
    if (panel.windowMode === "panel") { if (panel.opened) panel.close(); else { fromKeyboard(); panel.open() } return }
    if (panel.opened && focused) panel.close()
    else if (panel.opened && panel.managedToplevel) panel.managedToplevel.activate()
    else panel.open()
  }
  function quit() {
    quitting = true
    if (panel.opened) panel.close()
    backend.persistViewNow(); backend.flushDrafts()
    status.setText(JSON.stringify({ running: false, updatedAt: Date.now() }))
    Qt.callLater(Qt.quit)
  }

  // What the bar widget shows, written on change and every 30 s: the bar
  // treats a status older than 90 s as a stopped app.
  readonly property var statusValue: ({
    running: true, pid: Quickshell.processId, opened: app.opened, counts: app.counts,
    stale: backend.stale, indicatorStyle: backend.viewState.barIndicator || "orbit",
    tooltip: T3I18n.tr("Working: {working} · Ready: {ready} · Waiting for input: {waiting}", { working: app.counts.working, ready: app.counts.done, waiting: app.counts.waiting }) + (backend.stale ? " · " + T3I18n.tr("Offline") : ""),
    omabridge: app.bridgeIndicator ? bridgeStatus.value : null
  })
  onStatusValueChanged: publish.restart()
  FileView { id: status; path: app.statusPath; blockWrites: true; atomicWrites: true; printErrors: false }
  Timer { id: publish; interval: 120; onTriggered: if (!app.quitting) status.setText(JSON.stringify(Object.assign({ updatedAt: Date.now() }, app.statusValue))) }
  Timer { interval: 30000; repeat: true; running: true; onTriggered: publish.restart() }

  // The bar's OmaBridge indicator: the same state the app's OmaBridge page reads.
  Item {
    id: bridgeStatus
    visible: false
    readonly property var quick: bridgeLoader.item
    readonly property var value: quick ? ({
      linked: !!((quick.manage.snapshot || {}).published || {}).active || (((quick.manage.snapshot || {}).peers) || []).length > 0,
      pending: quick.manage.pendingCount, text: quick.statusText()
    }) : null
    Loader { id: bridgeLoader; active: app.bridgeIndicator; sourceComponent: Component { T3OmaBridgeQuick { bridge: app.backend } } }
    Timer { interval: 60000; repeat: true; triggeredOnStart: true; running: !!bridgeStatus.quick; onTriggered: if (!bridgeStatus.quick.manage.busy) bridgeStatus.quick.manage.refresh() }
  }

  IpcHandler {
    target: "io.github.avillagran.omarchy-t3-code"
    function ping(): bool { return true }
    // Not show/hide: `quickshell ipc call` reads "show" as its own subcommand.
    function openWindow(): void { app.show() }
    function hideWindow(): void { app.hide() }
    function toggleWindow(): void { app.toggle() }
    function setAnchor(json: string): void { app.setAnchor(json) }
    // Diagnostics: where the panel popup and its anchor are.
    function panelGeometry(): string { var p = panel.panelPopup; var c = p.contentItem; return JSON.stringify({ open: p.open, visible: p.visible, screen: p.screen ? p.screen.name : null, origin: p.cardOrigin, contentWidth: p.contentWidth, contentHeight: p.contentHeight, barW: p.barW, barH: p.barH, anchor: app.anchor, ghosts: ghosts.instances.map(function(g) { return g.modelData }), surface: { w: p.surfaceItem.width, h: p.surfaceItem.height, children: p.surfaceItem.children.length, opacity: p.surfaceItem.opacity, visible: p.surfaceItem.visible } }) }
    // "panel" (under the bar's T3 button), "floating" or "tiled".
    function setWindowMode(mode: string): void { if (["floating", "tiled"].indexOf(mode) >= 0 || (mode === "panel" && panel.panelAvailable)) panel.setWindowMode(mode) }
    function quit(): void { app.quit() }
    function state(): string {
      var b = app.backend, p = panel
      return JSON.stringify({ running: true, pid: Quickshell.processId, opened: app.opened, focused: app.focused, panelLoaded: true, detached: true, windowMode: p.windowMode,
        localBackend: b.localBackend, compaction: b.compactionStates,
        nativeScroll: b.scrollPolicy, modelFavorites: (b.catalog.favorites || []).length, modelCount: (b.catalog.models || []).length,
        inlineHtml: { visible: Object.keys(b.inlineDocuments).length, loaded: Object.keys(b.inlineDocuments).filter(function(k) { return !!b.inlineDocuments[k].frameUrl }).length, errors: Object.keys(b.inlineDocuments).map(function(k) { return b.inlineDocuments[k].error }).filter(function(e) { return !!e }) },
        liveConnected: b.liveConnected, loadedImages: Object.keys(b.imageStates).filter(function(k) { return !!b.imageStates[k].url }).length,
        pollInterval: b.pollInterval, working: b.state.working,
        language: T3I18n.locale, languageChoice: T3I18n.choice, statusCounts: app.counts, pending: b.state.pending, stale: b.stale,
        threads: b.state.threads.length, messages: b.transcript.length,
        messageError: b.messageError, loadingMessages: b.loadingMessages,
        pendingApprovals: b.approvals.length,
        history: { hasEarlier: b.hasEarlier, loadedRows: b.olderTranscript.length, busy: !!b.historyOperation.busy, error: b.historyOperation.error || "" },
        settled: b.state.threads.filter(function(t) { return !!t.settled }).length,
        filter: p.statusFilter,
        sidebarGrouping: { enabled: p.sidebarCompact, inbox: p.activeRows.length, working: p.workingRows.length, snoozed: p.snoozedRows.length, settled: p.settledRows.length },
        visibleThreads: p.sidebarRows.filter(function(t) { return !t.sectionHeader }).length,
        taskCount: b.tasks ? b.tasks.total : 0, completedTasks: b.tasks ? b.tasks.completed : 0,
        queuedMessages: b.queuedItems.length, queueHeld: !!b.queue.held, editingQueuedMessage: !!b.draft.queueEdit,
        changedFileCards: b.transcript.filter(function(m) { return m.role === "changes" }).length,
        activityRows: b.transcript.filter(function(m) { return m.role === "activity" }).length,
        draftState: b.draft.saveState || "",
        usageCollapsed: p.usageCollapsed, sidebarWidth: p.sidebarWidth, composerExpanded: p.composerExpanded,
        activityItems: b.activity.length, sidebarCollapsed: b.sidebarCollapsed,
        selectedKey: b.selectedKey, viewState: b.viewState, filtersOpen: p.filtersOpen,
        environments: b.state.environments,
        quotaWindows: b.providers.map(function(q) { return { provider: q.provider, weekly: !!q.weekly, shortWindow: !!q.shortWindow } }) })
    }
    function submitMessage(): bool {
      if (!app.opened || !panel.shortcutActive) return false
      panel.submitCurrent(); return true
    }
    function readThread(environment: string, threadId: string): void { app.show(); app.backend.selectThread(environment, threadId) }
    function filterState(value: string): void {
      if (["all", "working", "done", "failed", "pending", "settled"].indexOf(value) < 0) return
      app.show(); panel.statusFilter = value
    }
    function showSettled(value: bool): void { app.show(); panel.settledExpanded = value }
    function toggleSidebar(): void { app.show(); panel.toggleSidebar() }
    function closePanel(): void { app.hide() }
    // Opens the side panel on a tab ("browser", "renders" or "devices"); an empty tab closes it.
    function sidePanel(tab: string): void { app.show(); if (["browser", "renders", "devices"].indexOf(tab) >= 0) panel.openSide(tab); else panel.closeSide() }
    // Diagnostics: scroll the open conversation for a few seconds and report
    // the presented frame intervals (read the result with profileResult).
    function profileScroll(): void { panel.profileScroll() }
    function profileResult(): string { return JSON.stringify(panel.profile) }
    function skyState(): string { var s = panel.starfield; return JSON.stringify({ mode: s.effectiveMode, busy: s.busy, energy: s.energy, reach: s.meteorReach, meteorNow: s.meteorNow, clock: s.clock, ticker: T3Ticker.interval, sidebar: panel.sidebarCollapsed, degraded: s.degraded, paused: s.paused, visible: s.visible }) }
    function usageCollapsed(value: bool): void { app.show(); panel.usageCollapsed = value }
    function filters(value: bool): void { app.show(); if (value) panel.showFilters(); else panel.hideFilters() }
    function refresh(): void { app.backend.refresh() }
    function preferences(page: string): void { app.show(); Qt.callLater(function() { panel.openPreferencesPage(page || "general") }) }
    function openProject(environment: string, projectId: string): void { app.show(); app.backend.openProject(environment, projectId) }
  }
}
