import QtQuick 6.11
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "../js/SidebarModel.js" as Sidebar

BarWidget {
  id: root
  moduleName: "io.github.avillagran.omarchy-t3-code"
  readonly property var counts:Sidebar.counts(t3Backend.state.threads)
  implicitWidth: root.vertical ? Style.bar.statusSlot : status.implicitWidth+Style.space(6)
  implicitHeight: Style.bar.statusSlot
  readonly property bool working: t3Backend.state.working > 0
  property bool panelRequested: false
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened : false
  function open() {
    unloadPanel.stop()
    if (panelLoader.item) panelLoader.item.open()
    else panelRequested = true
  }
  function close() { if (panelLoader.item) panelLoader.item.close() }
  function togglePanel() { opened ? close() : open() }
  readonly property var t3Backend: T3Backend
  onOpenedChanged: t3Backend.setPanelVisible(opened)
  Loader {
    id: panelLoader
    active: root.panelRequested
    sourceComponent: Component {
      T3Panel { anchorItem: button; bar: root.bar; bridge: t3Backend }
    }
    onLoaded: item.open()
  }
  Connections {
    target: panelLoader.item
    function onOpenedChanged() { if (!root.opened) unloadPanel.restart() }
  }
  Timer {
    id: unloadPanel
    interval: 250
    onTriggered: if (!root.opened) root.panelRequested = false
  }
  IpcHandler {
    target: "io.github.avillagran.omarchy-t3-code"
    function state(): string {
      return JSON.stringify({ opened: root.opened, panelLoaded: panelLoader.item !== null,detached:panelLoader.item?panelLoader.item.detached:!!t3Backend.viewState.detached,
        localBackend:t3Backend.localBackend,
        compaction:t3Backend.compactionStates,
        nativeScroll:t3Backend.scrollPolicy,modelFavorites:(t3Backend.catalog.favorites || []).length,modelCount:(t3Backend.catalog.models || []).length,
        inlineHtml:{visible:Object.keys(t3Backend.inlineDocuments).length,loaded:Object.keys(t3Backend.inlineDocuments).filter(function(k){return !!t3Backend.inlineDocuments[k].frameUrl}).length,errors:Object.keys(t3Backend.inlineDocuments).map(function(k){return t3Backend.inlineDocuments[k].error}).filter(function(e){return !!e})},
        liveConnected:t3Backend.liveConnected,loadedImages:Object.keys(t3Backend.imageStates).filter(function(k){return !!t3Backend.imageStates[k].url}).length,
        pollInterval: t3Backend.pollInterval, working: t3Backend.state.working,
        language:T3I18n.locale, languageChoice:T3I18n.choice, statusCounts:root.counts,pending: t3Backend.state.pending, stale: t3Backend.stale,
        threads: t3Backend.state.threads.length, messages: t3Backend.transcript.length,
        messageError: t3Backend.messageError, loadingMessages: t3Backend.loadingMessages,
        pendingApprovals:t3Backend.approvals.length,
        history:{hasEarlier:t3Backend.hasEarlier,loadedRows:t3Backend.olderTranscript.length,busy:!!t3Backend.historyOperation.busy,error:t3Backend.historyOperation.error || ""},
        settled: t3Backend.state.threads.filter(function(t) { return !!t.settled }).length,
        filter: panelLoader.item ? panelLoader.item.statusFilter : "all",
        sidebarGrouping:panelLoader.item?({enabled:panelLoader.item.sidebarCompact,inbox:panelLoader.item.activeRows.length,working:panelLoader.item.workingRows.length,snoozed:panelLoader.item.snoozedRows.length,settled:panelLoader.item.settledRows.length}):({enabled:!!t3Backend.viewState.sidebarCompact}),
        visibleThreads: panelLoader.item ? panelLoader.item.sidebarRows.filter(function(t) { return !t.sectionHeader }).length : 0,
        taskCount:t3Backend.tasks ? t3Backend.tasks.total : 0,completedTasks:t3Backend.tasks ? t3Backend.tasks.completed : 0,
        queuedMessages:t3Backend.queuedItems.length,queueHeld:!!t3Backend.queue.held,editingQueuedMessage:!!t3Backend.draft.queueEdit,
        changedFileCards:t3Backend.transcript.filter(function(m){return m.role==="changes"}).length,
        activityRows: t3Backend.transcript.filter(function(m){return m.role === "activity"}).length,
        draftState: t3Backend.draft.saveState || "",
        usageCollapsed: panelLoader.item ? panelLoader.item.usageCollapsed : t3Backend.viewState.usageExpanded !== true,
        sidebarWidth: panelLoader.item ? panelLoader.item.sidebarWidth : t3Backend.viewState.sidebarWidth || 300,
        composerExpanded: panelLoader.item ? panelLoader.item.composerExpanded : !!t3Backend.viewState.composerExpanded,
        activityItems: t3Backend.activity.length, sidebarCollapsed: t3Backend.sidebarCollapsed,
        selectedKey: t3Backend.selectedKey, viewState: t3Backend.viewState,
        filtersOpen: panelLoader.item ? panelLoader.item.filtersOpen : false,
        environments: t3Backend.state.environments,
        quotaWindows: t3Backend.providers.map(function(p) { return {provider:p.provider, weekly:!!p.weekly, shortWindow:!!p.shortWindow} }) })
    }
    function submitMessage(): bool {
      if(!root.opened || (panelLoader.item&&!panelLoader.item.shortcutActive))return false
      if(panelLoader.item)panelLoader.item.submitCurrent();return true
    }
    function readThread(environment: string, threadId: string): void {
      root.open()
      t3Backend.selectThread(environment, threadId)
    }
    function filterState(value: string): void {
      if (["all","working","done","failed","pending","settled"].indexOf(value) < 0) return
      root.open(); if (panelLoader.item) panelLoader.item.statusFilter = value
    }
    function showSettled(value: bool): void { root.open(); if (panelLoader.item) panelLoader.item.settledExpanded = value }
    function toggleSidebar(): void { root.open(); if (panelLoader.item) panelLoader.item.toggleSidebar() }
    function closePanel(): void { root.close() }
    function usageCollapsed(value: bool): void { root.open(); if (panelLoader.item) panelLoader.item.usageCollapsed = value }
    function filters(value: bool): void { root.open(); if (panelLoader.item) { if (value) panelLoader.item.showFilters(); else panelLoader.item.hideFilters() } }
    function refresh(): void { t3Backend.refresh() }
    function openProject(environment: string, projectId: string): void {root.open();t3Backend.openProject(environment,projectId)}
  }
  BarIconButton {
    id: button
    anchors.centerIn: parent
    bar: root.bar
    active: root.opened
    slotSize: root.vertical ? Style.bar.statusSlot : root.implicitWidth
    tooltipText:T3I18n.tr("Working: {working} · Ready: {ready} · Waiting for input: {waiting}",{working:root.counts.working,ready:root.counts.done,waiting:root.counts.waiting})+(t3Backend.stale?" · "+T3I18n.tr("Offline"):"")
    iconComponent: Component { Item {} }
    T3StatusBadges{id:status;anchors.centerIn:parent;counts:root.counts;stale:t3Backend.stale;width:implicitWidth;height:Style.bar.statusSlot;scale:root.vertical?Math.min(1,(root.barSize-Style.space(4))/implicitWidth):1}
    onPressed: function(mouseButton) {
      if (mouseButton === Qt.LeftButton) root.togglePanel()
      else if (mouseButton === Qt.MiddleButton) t3Backend.refresh()
    }
  }
}
