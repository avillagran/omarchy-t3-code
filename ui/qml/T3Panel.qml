import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui
import "../js/SidebarModel.js" as Sidebar
import "../js/ProjectModel.js" as Projects
import "../js/WheelMotion.js" as Motion

Item {
  id: root
  property var anchorItem: null
  property var bar: null
  required property var bridge
  property var prTarget:null
  property bool pullRequestsOpen:false
  onPullRequestsOpenChanged:{if(!pullRequestsOpen)prTarget=null;if(initialized){if(pullRequestsOpen)conversation.stopWheel();else if(opened){restoringConversation=true;restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false})}}}
  function submitCurrent(){if(!pullRequestsOpen&&!documentPopup.visible)bridge.submit()}
  function selectSidebarThread(env,id){pullRequestsOpen=false;bridge.selectThread(env,id)}
  property bool initialized: false
  readonly property var conversationView: conversation
  readonly property var composerView: composerPanel
  readonly property var searchInput: searchField
  readonly property var sidebarView: threads
  property bool usageCollapsed: bridge.viewState.usageExpanded !== true
  onUsageCollapsedChanged: if(initialized && bridge.viewState.usageExpanded !== !usageCollapsed) bridge.rememberView({usageExpanded:!usageCollapsed,quotaCollapsed:usageCollapsed})
  property real sidebarWidth: Math.max(260,Math.min(600,bridge.viewState.sidebarWidth || 300))
  function resizeSidebar(width) { sidebarWidth=Math.max(260,Math.min(600,width));bridge.rememberView({sidebarWidth:sidebarWidth}) }
  property bool composerExpanded: !!bridge.viewState.composerExpanded
  property real panelWidth: bridge.viewState.panelWidth || 1120
  property real panelHeight: bridge.viewState.panelHeight || 780
  function resizePanel(w,h) {
    panelWidth=Math.max(760,Math.min(4000,w));panelHeight=Math.max(360,Math.min(3000,h))
    bridge.rememberView({panelWidth:panelWidth,panelHeight:panelHeight})
  }
  function expandComposer(focusEditor) {
    conversation.stopWheel();if(!composerExpanded)captureState()
    composerExpanded=!composerExpanded;bridge.rememberView({composerExpanded:composerExpanded})
    if(!composerExpanded) {restoringConversation=true;restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false});restoringSidebar=true;restorePosition(threads,activeRows,"@sidebar",false,function(){root.restoringSidebar=false})}
    if(focusEditor !== false)Qt.callLater(function(){composerPanel.inputField.forceActiveFocus()})
  }
  function focusSidebar() { if(composerExpanded)expandComposer(false);if(sidebarCollapsed)toggleSidebar(false);var choices=[{view:threads,rows:activeRows},{view:workingThreads,rows:visibleWorkingRows},{view:snoozedThreads,rows:visibleSnoozedRows},{view:settledThreads,rows:visibleSettledRows}],choice=choices.find(function(c){return c.rows.some(function(t){return t.key===root.bridge.selectedKey})}) || choices.find(function(c){return c.rows.length}) || choices[0];choice.view.forceActiveFocus();choice.view.currentIndex=Math.max(0,choice.rows.findIndex(function(t){return t.key===root.bridge.selectedKey})) }
  function navigateThread(step) { var rows=activeRows.concat(visibleWorkingRows).concat(visibleSnoozedRows).concat(visibleSettledRows);if(!rows.length)return;var index=rows.findIndex(function(t){return t.key===root.bridge.selectedKey});var t=rows[(index+step+rows.length)%rows.length];bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus() }
  Component.onCompleted: { initialized = true;syncConversation() }
  readonly property bool opened: popup.open
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.57)
  readonly property color divider: Qt.rgba(fg.r, fg.g, fg.b, 0.09)
  readonly property color surface: Qt.rgba(fg.r, fg.g, fg.b, 0.045)
  readonly property color healthy: Color.accent
  property string statusFilter: bridge.viewState.statusFilter || "all"
  property string projectFilter: bridge.viewState.projectFilter || ""
  onStatusFilterChanged: if (initialized && bridge.viewState.statusFilter !== statusFilter) bridge.rememberView({statusFilter:statusFilter})
  onProjectFilterChanged: if (initialized && bridge.viewState.projectFilter !== projectFilter) bridge.rememberView({projectFilter:projectFilter})
  readonly property bool sidebarCollapsed: bridge.sidebarCollapsed
  readonly property bool filtersOpen: filterMenu.visible
  function toggleSidebar(focusTarget) {
    if(composerExpanded){expandComposer();return}
    bridge.sidebarCollapsed = !bridge.sidebarCollapsed
    if(focusTarget !== false) Qt.callLater(function() { if (!root.opened) return; if (root.sidebarCollapsed) { if (root.bridge.selectedThread) composerPanel.inputField.forceActiveFocus() } else searchField.forceActiveFocus() })
  }
  function showFilters() { filterMenu.open() }
  function hideFilters() { filterMenu.close() }
  property bool sidebarCompact: !!bridge.viewState.sidebarCompact
  property bool workingExpanded: !!bridge.viewState.workingExpanded
  onSidebarCompactChanged:if(initialized && bridge.viewState.sidebarCompact!==sidebarCompact)bridge.rememberView({sidebarCompact:sidebarCompact})
  onWorkingExpandedChanged:if(initialized && bridge.viewState.workingExpanded!==workingExpanded)bridge.rememberView({workingExpanded:workingExpanded})
  property bool restoringWorking:true
  function toggleCompact(){setSidebarGrouped(!sidebarCompact)}
  function setSidebarGrouped(value){
    captureState();sidebarCompact=value
    if(sidebarCompact){workingExpanded=false;snoozedExpanded=false;settledExpanded=false}
    restoreSidebarShelves()
  }
  function restoreSidebarShelves(){
    restoringSidebar=true;restorePosition(threads,activeRows,"@sidebar",false,function(){root.restoringSidebar=false})
    restoringWorking=true;restorePosition(workingThreads,visibleWorkingRows,"@working",false,function(){root.restoringWorking=false})
    restoringSnoozed=true;restorePosition(snoozedThreads,visibleSnoozedRows,"@snoozed",false,function(){root.restoringSnoozed=false})
    restoringSettled=true;restorePosition(settledThreads,visibleSettledRows,"@settled",false,function(){root.restoringSettled=false})
  }
  property bool settledExpanded: !!bridge.viewState.settledExpanded
  onSettledExpandedChanged: if (initialized && bridge.viewState.settledExpanded !== settledExpanded) bridge.rememberView({settledExpanded:settledExpanded})
  property bool restoringConversation: true
  property bool restoringSidebar: true
  property bool restoringSettled: true
  property bool restoringSnoozed: true
  property bool snoozedExpanded: !!bridge.viewState.snoozedExpanded
  onSnoozedExpandedChanged: if(initialized && bridge.viewState.snoozedExpanded!==snoozedExpanded)bridge.rememberView({snoozedExpanded:snoozedExpanded})
  function capturePosition(view, rows, key) {
    if (!opened || !view.visible || !view.count || !rows.length) return
    var index = view.indexAt(1,view.contentY+1)
    if (index < 0) index = view.indexAt(1,view.contentY+Style.space(8))
    if (index < 0) return
    var item = view.itemAtIndex(index)
    if (!item || !rows[index]) return
    bridge.rememberPosition(key,{id:rows[index].key || rows[index].id,offset:view.contentY-item.y,atEnd:(view===conversation&&followingTail) || view.atYEnd || view.originY+view.contentHeight-view.height-view.contentY < Style.space(24)})
  }
  // An accepted send resumes live-edge following until the reader navigates away.
  property string submissionFollowKey: ""
  function jumpTimelineItem(entry) {
    if (!entry || !bridge.transcript.some(function(r){return r.id===entry.id})) return
    conversation.stopWheel();detachConversation();restoringConversation=true
    bridge.rememberPosition(bridge.selectedKey,{id:entry.id,offset:0,atEnd:false})
    restorePosition(conversation,bridge.transcript,bridge.selectedKey,false,function(){root.restoringConversation=false})
  }
  function followSubmittedMessage(key) {
    if (key !== bridge.selectedKey) return
    conversation.stopWheel();submissionFollowKey = key;followingTail=true
    bridge.rememberPosition(key,{id:"",offset:0,atEnd:true})
    settleSubmittedTail()
  }
  function settleSubmittedTail() {
    if (!(followingTail || submissionFollowKey===bridge.selectedKey) || !opened || !conversation.visible) return
    restoringConversation = true
    restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false})
  }
  ListModel { id: conversationRows; dynamicRoles:true }
  property var conversationIds:[]
  property var conversationFingerprints:({})
  property string modelKey:""
  function syncConversation() {
    var rows=bridge.transcript, ids=conversationIds.slice(), fingerprints=Object.assign({},conversationFingerprints)
    if(modelKey!==bridge.selectedKey){conversationRows.clear();ids=[];fingerprints={};modelKey=bridge.selectedKey}
    var wanted={};rows.forEach(function(r){wanted[r.id]=true})
    for(var n=ids.length-1;n>=0;n--)if(!wanted[ids[n]]){conversationRows.remove(n);delete fingerprints[ids[n]];ids.splice(n,1)}
    for(var i=0;i<rows.length;i++){
      var row=rows[i],found=ids.indexOf(row.id),print=JSON.stringify(row)
      if(found<0){conversationRows.insert(i,{row:row});ids.splice(i,0,row.id)}
      else {if(found!==i){conversationRows.move(found,i,1);ids.splice(i,0,ids.splice(found,1)[0])}if(fingerprints[row.id]!==print)conversationRows.setProperty(i,"row",row)}
      fingerprints[row.id]=print
    }
    conversationIds=ids;conversationFingerprints=fingerprints
  }
  property bool followingTail:true
  property string positionedKey:""
  property var scrollRestore: null
  function cancelScrollRestore() { scrollSettle.stop();scrollRestore=null;restoringConversation=false }
  function detachConversation() { submissionFollowKey="";followingTail=false;cancelScrollRestore() }
  Timer {
    id:scrollSettle;interval:16;repeat:true
    onTriggered:{
      var job=root.scrollRestore
      if(!job||!root.opened||job.key!==root.bridge.selectedKey||job.rows!==root.bridge.transcript){root.cancelScrollRestore();return}
      var v=conversation;if(!v.visible || !v.count){stop();root.scrollRestore=null;job.done();return}
      v.forceLayout()
      var p=job.position;var index=p?job.rows.findIndex(function(r){return r.id===p.id}):-1
      if(!p||p.atEnd||index<0){v.positionViewAtEnd();root.followingTail=true}
      else{var item=v.itemAtIndex(index);if(!item){v.positionViewAtIndex(index,ListView.Beginning);v.forceLayout();item=v.itemAtIndex(index)}if(item)v.contentY=Math.max(v.originY,Math.min(v.originY+Math.max(0,v.contentHeight-v.height),item.y+p.offset))}
      if(Math.abs(v.contentHeight-job.height)<1 && Math.abs(v.contentY-job.y)<1)job.stable++;else job.stable=0
      job.height=v.contentHeight;job.y=v.contentY;job.frames++
      if(job.stable>=3||job.frames>=24){stop();root.scrollRestore=null;job.done()}
    }
  }
  function restorePosition(view, rows, key, tail, done) {
    if(view===conversation){
      scrollSettle.stop()
      if(positionedKey!==key){positionedKey=key;var previous=bridge.sessionPositions[key];followingTail=!previous || previous.atEnd}
      if(!opened||!view.visible||key!==bridge.selectedKey||rows!==bridge.transcript){done();return}
      scrollRestore={key:key,rows:rows,position:(followingTail || submissionFollowKey === key) ? {atEnd:true} : bridge.sessionPositions[key] || null,done:done,height:-1,y:-1,stable:0,frames:0}
      scrollSettle.start();return
    }
    Qt.callLater(function() {
      if (!view) return
      if (view === conversation && (key !== root.bridge.selectedKey || rows !== root.bridge.transcript)) return
      if (!view.visible || !view.count) { done(); return }
      var p = (root.bridge.viewState.positions || {})[key]
      if (!p || (tail && p.atEnd)) { if (tail) view.positionViewAtEnd(); else view.positionViewAtBeginning(); done(); return }
      var index = rows.findIndex(function(row) { return (row.key || row.id) === p.id })
      if (index < 0) { if(tail)view.positionViewAtEnd();else view.positionViewAtBeginning(); done(); return }
      view.positionViewAtIndex(index,ListView.Beginning)
      Qt.callLater(function() {
        if (!view) return
        var item = view.itemAtIndex(index)
        if (item) view.contentY = Math.max(view.originY,Math.min(view.originY+Math.max(0,view.contentHeight-view.height),item.y+p.offset))
        done()
      })
    })
  }
  function captureState() {
    if (!restoringSidebar) capturePosition(threads,activeRows,"@sidebar")
    if (!restoringSettled) capturePosition(settledThreads,visibleSettledRows,"@settled")
    if (!restoringSnoozed) capturePosition(snoozedThreads,visibleSnoozedRows,"@snoozed")
    if (!restoringWorking) capturePosition(workingThreads,visibleWorkingRows,"@working")
    if (!restoringConversation && bridge.selectedKey) capturePosition(conversation,bridge.transcript,bridge.selectedKey)
    bridge.persistViewNow()
  }
  onOpenedChanged: {
    if (opened) {
      restoringSidebar = true; restorePosition(threads,activeRows,"@sidebar",false,function(){root.restoringSidebar=false})
      restoringSettled = true; restorePosition(settledThreads,visibleSettledRows,"@settled",false,function(){root.restoringSettled=false})
      restoringSnoozed = true; restorePosition(snoozedThreads,visibleSnoozedRows,"@snoozed",false,function(){root.restoringSnoozed=false})
      restoringWorking = true; restorePosition(workingThreads,visibleWorkingRows,"@working",false,function(){root.restoringWorking=false})
      restoringConversation = true; restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false})
    } else {documentPopup.close();bridge.closeDocument();conversation.stopWheel();captureState()}
  }
  Component.onDestruction: captureState()
  property real clockNow: Date.now()
  Timer { interval: 1000; repeat: true; running: root.opened && root.bridge.state.threads.some(function(t){return !!t.working}); onTriggered: root.clockNow = Date.now() }
  function duration(start) {
    if (!start) return ""
    var seconds = Math.max(0,Math.floor((clockNow-start)/1000))
    return seconds < 60 ? seconds + "s" : Math.floor(seconds/60) + "m " + (seconds%60) + "s"
  }
  readonly property var projectGroups:Projects.groups(bridge.state.projects,Object.assign({},bridge.viewState,{nativeProjectGrouping:bridge.state.nativeProjectGrouping}))
  readonly property var projectScope:Projects.scope(projectFilter,projectGroups)
  readonly property var sidebarRows: Sidebar.rows(bridge.state.threads, statusFilter, projectScope, settledExpanded, bridge.query)
  readonly property bool settledShown: settledExpanded || statusFilter === "settled" || bridge.query.length > 0
  readonly property var inboxRows: sidebarRows.filter(function(t) { return !t.sectionHeader && !t.settled && !t.snoozed })
  readonly property var activeRows:inboxRows.filter(function(t){return !root.sidebarCompact || !Sidebar.inWorkingShelf(t)}).sort(function(a,b){if(!!a.pinned!==!!b.pinned)return a.pinned?-1:1;if(a.pinned&&b.pinned)return String(a.pinOrderKey || "").localeCompare(String(b.pinOrderKey || "")) || (a.pinnedAt || 0)-(b.pinnedAt || 0);return 0})
  readonly property var workingRows:sidebarCompact ? inboxRows.filter(Sidebar.inWorkingShelf).sort(function(a,b){return (b.lastSentAt || b.startedAt || b.updatedAt || 0)-(a.lastSentAt || a.startedAt || a.updatedAt || 0)}) : []
  readonly property var visibleWorkingRows:Sidebar.shelfRows(workingRows,workingExpanded,bridge.selectedKey,bridge.query.length>0 || statusFilter==="working")
  readonly property var visibleSnoozedRows:Sidebar.shelfRows(snoozedRows,snoozedExpanded,bridge.selectedKey,bridge.query.length>0)
  readonly property var visibleSettledRows:Sidebar.shelfRows(settledRows,settledShown,bridge.selectedKey,false)
  readonly property var settledRows: Sidebar.rows(bridge.state.threads,statusFilter,projectScope,true,bridge.query).filter(function(t) { return !t.sectionHeader && t.settled && !t.snoozed })
  readonly property var snoozedRows: Sidebar.rows(bridge.state.threads,statusFilter,projectFilter,true,bridge.query).filter(function(t){return !t.sectionHeader && !!t.snoozed})
  readonly property var projectItems: [{key: "", title: T3I18n.tr("All projects")}].concat(projectGroups)
  function kind(t) { return Sidebar.state(t) }
  function stateColor(t) { return T3Palette.stateColor(kind(t)) }
  function open() { popup.open = true }
  function close() { documentPopup.close();imagePopup.close();diffPopup.close();captureState(); popup.open = false }
  function toggle() { popup.open = !popup.open }
  function date(ms) { return ms ? new Date(ms).toLocaleString(Qt.locale(T3I18n.locale), "dd MMM · HH:mm") : "—" }
  function age(ms) {
    if (!ms) return "—"
    var mins = Math.max(0, Math.floor((Date.now() - ms) / 60000))
    return mins < 1 ? T3I18n.tr("now") : mins < 60 ? mins + "m" : mins < 1440 ? Math.floor(mins / 60) + "h" : Math.floor(mins / 1440) + "d"
  }
  function shortcutHelp(){
    var shortcuts=[["Tab / Shift+Tab","Navigate controls"],["Ctrl+F","Search threads"],["Ctrl+K","Chat list"],["Ctrl+L","Write message"],["Super+B","Chat list"],["Super+A / Super+Z / Super+Shift+Z","Edit"],["Super+E","Expand / return to chat"],["Super+PgUp / Super+PgDown","Switch chat"],["Ctrl+Shift+A","Attach files"],["Ctrl+S","Save draft"],["Super+Enter","Send"],["↑ / ↓","Prompt history"],["PageUp / PageDown","Read conversation"],["Ctrl+Alt+Arrows","Resize"],["Esc","Return / close"]]
    return shortcuts.map(function(p){return T3I18n.tr("{shortcut} · {action}",{shortcut:p[0],action:T3I18n.tr(p[1])})}).join("\n")
  }
  function providerIcon(provider) {
    var p = String(provider || "").toLowerCase()
    return p.indexOf("codex") >= 0 || p.indexOf("openai") >= 0 ? "openai" : p.indexOf("claude") >= 0 ? "claude" : p.indexOf("cursor") >= 0 ? "cursor" : "acp"
  }
  function status(t) {
    return kind(t) === "waiting" ? T3I18n.tr("Waiting") : kind(t) === "working" ? T3I18n.tr("Working") : kind(t) === "failed" ? T3I18n.tr("Failed") : kind(t) === "done" ? T3I18n.tr("Done") : ""
  }
  Component {
    id: threadDelegate
    T3ThreadRow {
      required property var modelData
      required property int index
      thread: modelData; bridge: root.bridge
      width: ListView.view ? ListView.view.width : 0
      projectScoped:!!root.projectFilter&&(Array.isArray(root.projectScope)?root.projectScope.indexOf(modelData.projectKey)>=0:root.projectScope===modelData.projectKey)
      panelVisible: root.opened
      keyboardCurrent: ListView.view ? ListView.view.activeFocus && ListView.view.currentIndex===index : false
      stateColor:root.stateColor(modelData);stateLabel:root.status(modelData)
      timeLabel:modelData.working ? root.duration(modelData.startedAt) : root.age(modelData.updatedAt)
      providerGlyph:root.providerIcon(modelData.provider)
      onPullRequestsRequested:function(source,reference,multiple){root.bridge.selectThread(source.environmentId,source.id);root.prTarget=Object.assign({},reference,{environmentId:source.environmentId,multiple:multiple,references:Array.prototype.slice.call(source.pullRequestBadge.links)});root.pullRequestsOpen=true}
      onThreadActivated:root.pullRequestsOpen=false
      onFilterProjectRequested:function(key){var group=root.projectGroups.find(function(g){return g.members.some(function(p){return p.key===key})});if(group)root.projectFilter=root.projectFilter===group.key?"":group.key}
      onProjectSettingsRequested:function(key){projectMenu.configureProject(key)}
      onNewThreadOnBranch:function(source){sidebarActions.newOnBranch(source)}
      onProjectContextDismissed:projectMenu.dismissContext()
      onProjectContextRequested:function(item,px,py){projectMenu.showProject(modelData.projectKey,item,px,py)}
      ListView.onPooled:pooled=true
      ListView.onReused:pooled=false
    }
  }
  T3ProjectMenu{id:projectMenu;bridge:root.bridge;onGroupingChanged:function(previous,current){if(root.projectFilter===previous||root.projectFilter===projectMenu.project.key)root.projectFilter=current}}
  T3CompactionSettings{id:compactionSettings;bridge:root.bridge}
  PinnedKeyboardPanel {
    id: popup
    anchorItem: root.anchorItem; bar: root.bar; owner: root
    contentWidth: Math.min(Style.space(root.panelWidth), availableCardWidth - padding * 2)
    contentHeight: Math.min(Style.space(root.panelHeight), availableCardHeight - padding * 2)
    resizable: true
    onResized: function(w,h) { root.resizePanel(w/Style.space(1),h/Style.space(1)) }
    padding: 0
    focusTarget: root.sidebarCollapsed || root.composerExpanded ? (root.bridge.selectedThread ? composerPanel.inputField : null) : searchField
    RowLayout {
      id:panelLayout;objectName:"t3PanelLayout"
      LayoutMirroring.enabled:T3I18n.rtl;LayoutMirroring.childrenInherit:true
      anchors.fill: parent; spacing: 0
      Shortcut { sequence:"Meta+B";enabled:root.opened;onActivated:root.toggleSidebar() }
      Shortcut { sequence:"Ctrl+F";enabled:root.opened;onActivated:{if(root.composerExpanded)root.expandComposer(false);if(root.sidebarCollapsed)root.toggleSidebar(false);searchField.forceActiveFocus();searchField.selectAll()} }
      Shortcut { sequence:"Ctrl+K";enabled:root.opened;onActivated:root.focusSidebar() }
      Shortcut { sequence:"Ctrl+L";enabled:root.opened;onActivated:composerPanel.inputField.forceActiveFocus() }
      Shortcut { sequence:"Meta+E";enabled:root.opened && !!root.bridge.selectedThread;onActivated:root.expandComposer() }
      Shortcut { sequence:"Meta+Return";enabled:root.opened;onActivated:root.submitCurrent() }
      Shortcut { sequence:"Ctrl+Shift+A";enabled:root.opened;onActivated:composerPanel.attachFiles() }
      Shortcut { sequence:"Ctrl+S";enabled:root.opened;onActivated:root.bridge.flushDrafts() }
      Shortcut { sequence:"Meta+PgDown";enabled:root.opened;onActivated:root.navigateThread(1) }
      Shortcut { sequence:"Meta+PgUp";enabled:root.opened;onActivated:root.navigateThread(-1) }
      Shortcut { sequence:"Ctrl+Alt+Right";enabled:root.opened;onActivated:root.resizePanel(root.panelWidth+40,root.panelHeight) }
      Shortcut { sequence:"Ctrl+Alt+Left";enabled:root.opened;onActivated:root.resizePanel(root.panelWidth-40,root.panelHeight) }
      Shortcut { sequence:"Ctrl+Alt+Up";enabled:root.opened;onActivated:root.resizePanel(root.panelWidth,root.panelHeight-30) }
      Shortcut { sequence:"Ctrl+Alt+Down";enabled:root.opened;onActivated:root.resizePanel(root.panelWidth,root.panelHeight+30) }
      Shortcut { sequence:"F1";enabled:root.opened;onActivated:keyHelp.open() }
      Controls.Popup {
        id:imagePopup
        objectName:"t3ImagePopup"
        property var attachment:({})
        property string assetKey:""
        readonly property var imageState:root.bridge.imageStates[assetKey] || ({})
        x:Style.space(20);y:Style.space(20);width:Math.max(100,parent.width-Style.space(40));height:Math.max(100,parent.height-Style.space(40));padding:Style.space(14)
        modal:true;focus:true
        onClosed:{assetKey="";attachment=({})}
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        ColumnLayout {
          objectName:"t3ImageContents"
          anchors.fill:parent
          RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:imagePopup.attachment.name || "";color:root.fg;elide:Text.ElideMiddle;font.pixelSize:Style.font.body;textFormat:Text.PlainText}T3CopyButton{objectName:"copyFullImage";bridge:root.bridge;attachment:imagePopup.attachment}T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:imagePopup.close()}}
          Image{objectName:"t3FullImage";Layout.fillWidth:true;Layout.fillHeight:true;source:imagePopup.visible?imagePopup.imageState.url || "":"";asynchronous:true;cache:false;sourceSize:Qt.size(1920,1080);fillMode:Image.PreserveAspectFit}
          WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!imagePopup.imageState.loading;active:visible&&imagePopup.visible}
          Text{Layout.fillWidth:true;visible:!!imagePopup.imageState.error;text:T3I18n.error(imagePopup.imageState.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
        }
      }
      Controls.Popup {
        id:diffPopup
        objectName:"t3DiffPopup"
        onClosed:root.bridge.diffView=({loading:false,text:"",error:"",requestId:""})
        focus:true;modal:true
        x:Style.space(24);y:Style.space(24)
        width:Math.max(100,parent.width-Style.space(48));height:Math.max(100,parent.height-Style.space(48))
        padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        ColumnLayout {
          objectName:"t3DiffContents"
          anchors.fill:parent
          RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr("Changed files");color:root.fg;font.pixelSize:Style.font.body}T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:diffPopup.close()}}
          Text{Layout.fillWidth:true;visible:root.bridge.diffView.loading || !!root.bridge.diffView.error;text:root.bridge.diffView.loading?T3I18n.tr("Loading diff…"):T3I18n.error(root.bridge.diffView.error);color:root.bridge.diffView.error?Color.urgent:root.dim;wrapMode:Text.Wrap}
          ListView {
            id:diffList;objectName:"t3DiffLines"
            T3Wheel {flickable:diffList}
            Layout.fillWidth:true;Layout.fillHeight:true;clip:true;reuseItems:false
            model:diffPopup.visible && !root.bridge.diffView.loading?root.bridge.diffView.text.split("\n"):[]
            Controls.ScrollBar.vertical:Controls.ScrollBar{}
            delegate:Rectangle {
              required property string modelData
              width:ListView.view.width;height:diffLine.implicitHeight+Style.space(3)
              color:modelData.startsWith("+")?Qt.rgba(T3Palette.done.r,T3Palette.done.g,T3Palette.done.b,0.08):modelData.startsWith("-")?Qt.rgba(Color.urgent.r,Color.urgent.g,Color.urgent.b,0.08):"transparent"
              TextEdit{id:diffLine;width:parent.width;text:modelData;textFormat:TextEdit.PlainText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;font.family:"monospace";font.pixelSize:Style.font.caption;color:modelData.startsWith("+")?T3Palette.done:modelData.startsWith("-")?Color.urgent:root.fg}
            }
          }
        }
      }
      Controls.Popup {
        id:documentPopup;objectName:"t3EmbeddedDocumentPopup";focus:true;modal:true
        x:Style.space(18);y:Style.space(18);width:Math.max(100,parent.width-Style.space(36));height:Math.max(100,parent.height-Style.space(36));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        onClosed:root.bridge.closeDocument()
        contentItem:Loader{active:documentPopup.visible;sourceComponent:Component{T3DocumentView{bridge:root.bridge;onClosed:documentPopup.close()}}}
      }
      Connections{target:root.bridge;function onDocumentOpenChanged(){if(root.opened&&root.bridge.documentOpen.requestId&&!documentPopup.visible)documentPopup.open()}}
      Controls.Popup {
        id:keyHelp; focus:true;x:Math.max(0,(parent.width-width)/2);y:Style.space(60);width:Style.space(420);padding:Style.space(18)
        background:Rectangle {color:Color.popups.background;border.color:root.divider;border.width:1;radius:Style.space(10)}
        contentItem:Text {text:root.shortcutHelp();color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      }
      Keys.onEscapePressed: {if(root.composerExpanded)root.expandComposer();else root.close()}
      T3SidebarActions { id: sidebarActions; bridge: root.bridge; projects: root.bridge.state.projects || [] }
      Controls.Popup {
        id: filterMenu
        focus: true
        parent: filterButton
        width: Style.space(260); padding: Style.space(10)
        x: Math.min(0, sidebar.width-filterButton.x-width-Style.space(12)); y: filterButton.height+Style.space(5)
        closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside
        background: Rectangle { color: Color.popups.background; radius: Style.space(9); border.width: 1; border.color: root.divider }
        contentItem: ColumnLayout {
          spacing: Style.space(4)
          Text { text: T3I18n.tr("Status"); color: root.dim; font.pixelSize: Style.font.caption; Layout.margins: Style.space(5) }
          Repeater {
            model: [{id:"all",es:"Todos",en:"All"},{id:"working",es:"Activos",en:"Working"},{id:"done",es:"Listos",en:"Done"},{id:"failed",es:"Fallidos",en:"Failed"},{id:"pending",es:"Pendientes",en:"Pending"},{id:"settled",es:"Settled",en:"Settled"}]
            Controls.ItemDelegate {
              required property var modelData
              Layout.fillWidth: true; implicitHeight: Style.space(31)
              readonly property color tint: modelData.id === "pending" ? T3Palette.waiting : ["all","settled"].indexOf(modelData.id) >= 0 ? root.dim : T3Palette.stateColor(modelData.id)
              contentItem: RowLayout {
                T3Icon { name: "circle-check"; implicitWidth: Style.space(13); tint: parent.parent.tint; opacity: root.statusFilter === modelData.id ? 1 : 0 }
                Text { Layout.fillWidth: true; text: T3I18n.tr(modelData.en); color: parent.parent.tint; font.pixelSize: Style.font.caption }
                Text { text: Sidebar.count(root.bridge.state.threads,modelData.id,root.projectScope); color: parent.parent.tint; font.pixelSize: Style.font.caption }
              }
              background: Rectangle { radius: Style.space(5); color: parent.hovered || root.statusFilter === modelData.id ? root.surface : "transparent" }
              onClicked: { root.statusFilter = modelData.id; filterMenu.close() }
            }
          }
          Rectangle { Layout.fillWidth: true; height: 1; color: root.divider }
          Text { text: T3I18n.tr("Project"); color: root.dim; font.pixelSize: Style.font.caption; Layout.margins: Style.space(5) }
          T3Select {
            Layout.fillWidth: true; popupAbove: false
            model: root.projectItems; textRole: "title"
            currentIndex: Math.max(0,root.projectItems.findIndex(function(p) { return p.key === root.projectFilter || (p.members || []).some(function(m){return m.key===root.projectFilter}) }))
            onActivated: function(index) { root.projectFilter = root.projectItems[index].key; filterMenu.close() }
          }
        }
      }
      Rectangle {
        id: sidebar
        Layout.preferredWidth: Math.min(Style.space(root.sidebarWidth),Math.max(Style.space(260),panelLayout.width-Style.space(420))); Layout.fillHeight: true
        visible: !root.sidebarCollapsed && !root.composerExpanded
        color: Qt.darker(Color.popups.background, 1.10)
        ColumnLayout {
          anchors.fill: parent; spacing: 0
          Rectangle {
            id: brand
            Layout.fillWidth: true; Layout.preferredHeight: Style.space(76)
            clip: true
            gradient: Gradient {
              orientation: Gradient.Horizontal
              GradientStop { position: 0; color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.12) }
              GradientStop { position: 1; color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.025) }
            }
            Repeater {
              model: 16
              Rectangle {
                required property int index
                x: ((index * 47 + 19) % 291) * brand.width / 300
                y: ((index * 23 + 9) % 65) * brand.height / 76
                width: index % 3 === 0 ? 2 : 1; height: width; radius: 1
                color: root.fg; opacity: 0.12 + (index % 4) * 0.08
              }
            }
            RowLayout {
              anchors.fill: parent; anchors.margins: Style.space(18); spacing: Style.space(10)
              T3IconButton { name: "panel-left-close"; tooltipText: T3I18n.tr("Hide sidebar"); onClicked: root.toggleSidebar() }
              Column {
                spacing: Style.space(3)
                Row {
                  LayoutMirroring.enabled:false;layoutDirection:Qt.LeftToRight
                  spacing: Style.space(4)
                  T3Icon { name: "t3-logo"; implicitWidth: Style.space(32); implicitHeight: Style.space(20); tint: root.fg }
                  Text { text: "Code"; font.pixelSize: Style.space(18); font.bold: true; color: root.fg }
                }
                Text { text: "OMARCHY"; font.pixelSize: Style.space(8); font.letterSpacing: 2; color: root.dim }
              }
              Item { Layout.fillWidth: true }
              T3IconButton{objectName:"t3PullRequestsToggle";name:"git-pull-request";tooltipText:T3I18n.tr("Pull requests");tint:root.pullRequestsOpen?Color.accent:root.dim;onClicked:root.pullRequestsOpen=!root.pullRequestsOpen}
              T3IconButton {
                objectName:"t3UsageToggle";name:"chart-no-axes-column"
                tint:root.usageCollapsed ? root.dim : Color.accent
                tooltipText:root.usageCollapsed ? T3I18n.tr("Show provider usage") : T3I18n.tr("Hide provider usage")
                onClicked:root.usageCollapsed=!root.usageCollapsed
              }
              Rectangle { width: Style.space(6); height: width; radius: width / 2; color: root.bridge.stale ? Color.urgent : root.healthy }
            }
          }
          RowLayout {
            Layout.fillWidth: true; Layout.margins: Style.space(12); spacing: Style.space(7)
            T3Icon { name: "search"; tint: root.dim }
            Controls.TextField {
              id: searchField
              Layout.fillWidth: true
              text: root.bridge.query
              placeholderText: T3I18n.tr("Search threads…")
              color: root.fg; placeholderTextColor: root.dim
              font.pixelSize: Style.font.body
              background: Rectangle { color: "transparent" }
              onTextEdited: root.bridge.search(text)
              Keys.onEscapePressed: root.close()
              Keys.onDownPressed: root.focusSidebar()
            }
            T3IconButton {
              id: filterButton
              name: "list-filter"; tooltipText: T3I18n.tr("Filter by state and project")
              tint: root.statusFilter !== "all" || root.projectFilter ? Color.accent : root.dim
              onClicked: filterMenu.visible ? filterMenu.close() : filterMenu.open()
            }
            T3IconButton { name: "folder-plus"; tooltipText: T3I18n.tr("Add project"); onClicked: sidebarActions.addProject() }
            T3IconButton { name: "square-pen"; tooltipText: T3I18n.tr("New chat"); onClicked: sidebarActions.newThread() }
          }
          RowLayout {
            Layout.fillWidth: true; Layout.leftMargin: Style.space(12); Layout.rightMargin: Style.space(12)
            visible: !!root.bridge.managementError
            Text { Layout.fillWidth: true; text: T3I18n.error(root.bridge.managementError); color: Color.urgent; font.pixelSize: Style.font.caption; elide: Text.ElideRight }
            T3IconButton { name: "refresh-cw"; visible: !!root.bridge.managementRequest; tooltipText: T3I18n.tr("Retry action"); onClicked: root.bridge.manage({}) }
          }
          ListView {
            id: threads
            T3Wheel {flickable:threads}
            activeFocusOnTab: true
            highlight: Rectangle {color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.09);visible:threads.activeFocus;radius:Style.space(8)}
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: activeFocus
            Keys.onReturnPressed: {var t=root.activeRows[currentIndex];if(t){root.selectSidebarThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
            Keys.onEnterPressed: {var t=root.activeRows[currentIndex];if(t){root.bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
            objectName: "threadList"
            Layout.fillWidth: true; Layout.fillHeight: true
            Layout.leftMargin: Style.space(8); Layout.rightMargin: Style.space(8)
            clip: true; reuseItems: true; cacheBuffer: 0
            visible: root.activeRows.length > 0 || (!root.workingRows.length && !root.snoozedRows.length && !root.settledRows.length)
            model: root.activeRows
            onContentYChanged: if (!root.restoringSidebar) root.capturePosition(threads,root.activeRows,"@sidebar")
            spacing: 0
            Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
            add: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 140 } }
            delegate: threadDelegate
            Text {
              anchors.centerIn: parent; width: parent.width - Style.space(28)
              visible: threads.count === 0
              text: root.bridge.stale ? T3I18n.tr("T3 is offline") : T3I18n.tr("No threads here")
              color: root.dim; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap
            }
          }
          Rectangle {
            objectName:"workingShelfToggle"
            Layout.fillWidth:true;Layout.preferredHeight:Style.space(34);Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12)
            color:"transparent";visible:root.sidebarCompact;activeFocusOnTab:true
            Accessible.role:Accessible.Button;Accessible.name:T3I18n.tr("Working ({count})",{count:root.workingRows.length})
            Keys.onReturnPressed:root.workingExpanded=!root.workingExpanded
            Keys.onSpacePressed:root.workingExpanded=!root.workingExpanded
            RowLayout{anchors.fill:parent;anchors.margins:Style.space(5);Text{Layout.fillWidth:true;text:parent.parent.Accessible.name;color:T3Palette.working;font.pixelSize:Style.font.caption}T3Icon{name:root.workingExpanded?"chevron-down":"chevron-up";tint:root.dim;implicitWidth:Style.space(12)}}
            MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:root.workingExpanded=!root.workingExpanded}
          }
          ListView {
            id:workingThreads;objectName:"workingThreadList"
            T3Wheel{flickable:workingThreads}
            Layout.fillWidth:true;Layout.fillHeight:root.activeRows.length===0
            Layout.preferredHeight:Math.min(contentHeight,Style.space(220));Layout.maximumHeight:root.activeRows.length>0?Style.space(220):Infinity
            Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
            visible:root.visibleWorkingRows.length>0
            activeFocusOnTab:true;keyNavigationEnabled:true;clip:true;reuseItems:true;cacheBuffer:0;spacing:0
            highlightFollowsCurrentItem:activeFocus
            highlight:Rectangle{color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.09);visible:workingThreads.activeFocus;radius:Style.space(8)}
            Keys.onReturnPressed:{var t=root.visibleWorkingRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
            Keys.onEnterPressed:{var t=root.visibleWorkingRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
            model:root.visibleWorkingRows;delegate:threadDelegate
            onContentYChanged:if(!root.restoringWorking)root.capturePosition(workingThreads,root.visibleWorkingRows,"@working")
            Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
          }
          Rectangle {
            Layout.fillWidth:true;Layout.preferredHeight:Style.space(34);Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12)
            objectName:"snoozedShelfToggle"
            color:"transparent";visible:root.sidebarCompact || root.snoozedRows.length>0;activeFocusOnTab:true
            Accessible.role:Accessible.Button;Accessible.name:T3I18n.tr("Snoozed ({count})",{count:root.snoozedRows.length})
            Keys.onReturnPressed:root.snoozedExpanded=!root.snoozedExpanded
            Keys.onSpacePressed:root.snoozedExpanded=!root.snoozedExpanded
            RowLayout{anchors.fill:parent;anchors.margins:Style.space(5);Text{Layout.fillWidth:true;text:parent.parent.Accessible.name;color:root.dim;font.pixelSize:Style.font.caption}T3Icon{name:root.snoozedExpanded?"chevron-down":"chevron-up";tint:root.dim;implicitWidth:Style.space(12)}}
            MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:root.snoozedExpanded=!root.snoozedExpanded}
          }
          ListView {
            id:snoozedThreads;objectName:"snoozedThreadList"
            T3Wheel{flickable:snoozedThreads}
            Layout.fillWidth:true;Layout.preferredHeight:Math.min(contentHeight,Style.space(164));Layout.maximumHeight:Style.space(164)
            Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
            visible:root.visibleSnoozedRows.length>0
            activeFocusOnTab:true;keyNavigationEnabled:true;clip:true;reuseItems:true;cacheBuffer:0;spacing:0
            highlightFollowsCurrentItem:activeFocus
            Keys.onReturnPressed:{var t=root.visibleSnoozedRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
            Keys.onEnterPressed:{var t=root.visibleSnoozedRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
            model:root.visibleSnoozedRows;delegate:threadDelegate
            onContentYChanged:if(!root.restoringSnoozed)root.capturePosition(snoozedThreads,root.visibleSnoozedRows,"@snoozed")
            Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
          }
          Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: Style.space(34)
            activeFocusOnTab: true
            Accessible.role: Accessible.Button
            objectName:"settledShelfToggle"
            Accessible.name: T3I18n.tr("Settled ({count})",{count:root.settledRows.length})
            Keys.onReturnPressed: root.settledExpanded = !root.settledExpanded
            Keys.onSpacePressed: root.settledExpanded = !root.settledExpanded
            Layout.leftMargin: Style.space(12); Layout.rightMargin: Style.space(12)
            visible: root.sidebarCompact || root.settledRows.length > 0
            color: "transparent"
            RowLayout {
              anchors.fill: parent; anchors.leftMargin: Style.space(5); anchors.rightMargin: Style.space(5)
              Text { Layout.fillWidth: true; text:T3I18n.tr("Settled ({count})",{count:root.settledRows.length}); color: root.dim; font.pixelSize: Style.font.caption }
              T3Icon { name: root.settledShown ? "chevron-down" : "chevron-up"; tint: root.dim; implicitWidth: Style.space(12) }
            }
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.settledExpanded = !root.settledExpanded }
          }
          ListView {
            id: settledThreads
            T3Wheel {flickable:settledThreads}
            activeFocusOnTab: true
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: activeFocus
            highlight: Rectangle {color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.09);visible:settledThreads.activeFocus;radius:Style.space(8)}
            Keys.onReturnPressed: {var t=root.visibleSettledRows[currentIndex];if(t){root.bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
            Keys.onEnterPressed: {var t=root.visibleSettledRows[currentIndex];if(t){root.bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
            Layout.fillWidth: true
            Layout.fillHeight: root.activeRows.length === 0
            Layout.preferredHeight: Math.min(contentHeight,Style.space(220))
            Layout.maximumHeight: root.activeRows.length > 0 ? Style.space(220) : Infinity
            Layout.leftMargin: Style.space(8); Layout.rightMargin: Style.space(8)
            visible: root.visibleSettledRows.length > 0
            clip: true; reuseItems: true; cacheBuffer: 0; spacing: 0
            model: root.visibleSettledRows; delegate: threadDelegate
            onContentYChanged: if (!root.restoringSettled) root.capturePosition(settledThreads,root.visibleSettledRows,"@settled")
            Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
          }
          Rectangle { Layout.fillWidth: true; height: 1; color: root.divider }
          RowLayout {
            objectName:"t3SidebarFooter"
            Layout.fillWidth:true;Layout.margins:Style.space(10);spacing:Style.space(3)
            T3Icon {
              objectName:"connectionStatusIcon";name:root.bridge.stale?"circle-alert":"cloud";implicitWidth:Style.space(18)
              readonly property string statusText:root.bridge.stale?((root.bridge.state.environments || []).some(function(e){return e.state==="ready"})?T3I18n.tr("Partially connected"):T3I18n.tr("Offline")):T3I18n.tr("Direct connection")
              tint:root.bridge.stale?Color.urgent:root.dim;Accessible.name:statusText
              HoverHandler{id:connectionStatusHover}
              Controls.ToolTip.visible:connectionStatusHover.hovered
              Controls.ToolTip.text:statusText
            }
            Item{Layout.fillWidth:true}
            T3Icon {
              objectName:"localBackendGuiIndicator";name:"monitor";implicitWidth:Style.space(20)
              visible:root.bridge.localBackend.owner==="gui";tint:root.dim
              Accessible.name:T3I18n.tr("Managed by T3 Code GUI")
              HoverHandler{id:guiOwnerHover}
              Controls.ToolTip.visible:guiOwnerHover.hovered
              Controls.ToolTip.text:T3I18n.tr("Managed by T3 Code GUI")
            }
            T3IconButton {
              id:localBackendControl;objectName:"startLocalBackend"
              onVisibleChanged:if(!visible&&localBackendMenu)localBackendMenu.close()
              visible:root.bridge.localBackend.owner!=="gui"
              name:root.bridge.backendStart.busy?"loader-circle":root.bridge.localBackend.running?"server":"play"
              tint:root.bridge.backendStart.error?Color.urgent:root.bridge.localBackend.running?Color.accent:root.dim
              tooltipText:root.bridge.backendStart.error?T3I18n.error(root.bridge.backendStart.error):root.bridge.localBackend.error?T3I18n.error(root.bridge.localBackend.error):T3I18n.tr(root.bridge.backendStart.busy?"Loading…":root.bridge.localBackend.running?"Local backend controls":"Start local backend")
              enabled:root.bridge.localBackend.canControl&&!root.bridge.backendStart.busy
              onClicked:root.bridge.localBackend.running?localBackendMenu.popup():root.bridge.startBackend()
              Controls.Menu {
                id:localBackendMenu;objectName:"localBackendMenu";width:Style.space(260)
                palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
                background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:root.dim}
                Controls.MenuItem{objectName:"localBackendStart";text:T3I18n.tr("Start local backend");enabled:root.bridge.localBackend.canControl&&!root.bridge.localBackend.running&&!root.bridge.backendStart.busy;onTriggered:root.bridge.startBackend()}
                Controls.MenuItem{objectName:"localBackendRestart";text:T3I18n.tr("Restart local backend");enabled:root.bridge.localBackend.canControl&&root.bridge.localBackend.running&&!root.bridge.backendStart.busy;onTriggered:root.bridge.controlBackend("restart")}
                Controls.MenuItem{objectName:"localBackendStop";text:T3I18n.tr("Stop local backend");enabled:root.bridge.localBackend.canControl&&root.bridge.localBackend.running&&!root.bridge.backendStart.busy;onTriggered:root.bridge.controlBackend("stop")}
              }
            }
            T3IconButton {objectName:"sidebarCompactToggle";name:root.sidebarCompact?"check":"list-ordered";tint:root.sidebarCompact?Color.accent:root.dim;tooltipText:T3I18n.tr("Group working, snoozed and settled threads");onClicked:root.toggleCompact()}
            T3IconButton {name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:root.bridge.refresh()}
            T3Select {id:languagePicker;objectName:"t3LanguagePicker";Layout.preferredWidth:Math.min(languageText.width+Style.space(36),root.sidebarWidth*Style.space(0.5));popupAbove:true;model:T3I18n.languages;textRole:"name";currentIndex:Math.max(0,model.findIndex(function(l){return l.id===T3I18n.choice}));onActivated:function(index){root.bridge.rememberView({language:model[index].id})}}
            TextMetrics{id:languageText;text:languagePicker.displayText;font:languagePicker.font}
          }

        }
      }
      Item {
        id:sidebarResizer
        Layout.fillHeight:true;Layout.preferredWidth:Style.space(6)
        visible:!root.sidebarCollapsed && !root.composerExpanded
        activeFocusOnTab:true
        Accessible.role:Accessible.Slider
        Accessible.name:T3I18n.tr("Sidebar width")
        Keys.onLeftPressed:root.resizeSidebar(sidebar.width/Style.space(1)-10)
        Keys.onRightPressed:root.resizeSidebar(sidebar.width/Style.space(1)+10)
        Rectangle{anchors.centerIn:parent;width:sidebarDrag.containsMouse || sidebarDrag.pressed || parent.activeFocus ? Style.space(2):1;height:parent.height;color:sidebarDrag.containsMouse || sidebarDrag.pressed || parent.activeFocus ? Color.accent : root.divider}
        MouseArea{
          id:sidebarDrag;objectName:"t3SidebarResizer"
          anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.SizeHorCursor
          property real startX:0;property real startWidth:300
          onPressed:function(mouse){startX=mapToGlobal(mouse.x,mouse.y).x;startWidth=sidebar.width/Style.space(1)}
          onPositionChanged:function(mouse){if(pressed)root.resizeSidebar(startWidth+(T3I18n.rtl?-1:1)*(mapToGlobal(mouse.x,mouse.y).x-startX)/Style.space(1))}
          onDoubleClicked:root.resizeSidebar(300)
        }
      }
      Rectangle {
        objectName:"t3MainChatArea"
        Layout.fillWidth: true; Layout.fillHeight: true
        color: Color.popups.background
        Loader{objectName:"t3PullRequestsView";anchors.fill:parent;active:root.pullRequestsOpen&&root.opened;sourceComponent:Component{T3PullRequests{bridge:root.bridge;initialEntry:root.prTarget;onClosed:root.pullRequestsOpen=false}}}
        ColumnLayout {
          visible:!root.pullRequestsOpen
          anchors.fill: parent; spacing: 0
          RowLayout {
            objectName:"t3ChatHeader"
            Layout.fillWidth: true; Layout.preferredHeight: Style.space(58)
            Layout.leftMargin: Style.space(24); Layout.rightMargin: Style.space(14)
            spacing: Style.space(10)
            T3IconButton { visible: root.sidebarCollapsed; name: "panel-left"; tooltipText: T3I18n.tr("Open sidebar"); onClicked: root.toggleSidebar() }
            Text { text: root.bridge.selectedProject || "T3 Code"; color: root.dim; elide: Text.ElideRight; Layout.maximumWidth: Style.space(180); font.pixelSize: Style.font.body; textFormat: Text.PlainText }
            Text { text: "/"; color: root.dim; visible: !!root.bridge.selectedThread }
            Text { Layout.fillWidth: true; text: root.bridge.selectedTitle || T3I18n.tr("Your workspace"); color: root.fg; font.pixelSize: Style.font.body; elide: Text.ElideRight; textFormat: Text.PlainText }
            T3IconButton { name: "external-link"; enabled: !!root.bridge.selectedThread; tooltipText: T3I18n.tr("Open in T3"); onClicked: root.bridge.openThread(root.bridge.selectedEnvironment, root.bridge.selectedThread) }
            T3IconButton { name: "x"; tooltipText: T3I18n.tr("Close"); onClicked: root.close() }
            T3IconButton{name:"shrink";tooltipText:T3I18n.tr("Automatic compaction");onClicked:compactionSettings.open()}
            Controls.Button {text:"?";implicitWidth:Style.space(28);Accessible.name:T3I18n.tr("Keyboard shortcuts");onClicked:keyHelp.open()}
          }
          RowLayout{
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(14)
            visible:(root.bridge.state.environments || []).some(function(e){return e.id==="local"&&e.state!=="ready"}) || !!root.bridge.backendStart.error
            Text{Layout.fillWidth:true;text:root.bridge.backendStart.error?T3I18n.error(root.bridge.backendStart.error):T3I18n.tr("Local backend is offline");color:root.bridge.backendStart.error?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
          }
          Rectangle { Layout.fillWidth: true; height: 1; color: root.divider }
          QuotaStrip {
            id:usage
            objectName:"t3HeaderUsage"
            Layout.fillWidth:true
            Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24)
            Layout.topMargin:root.usageCollapsed?0:Style.space(10)
            Layout.bottomMargin:root.usageCollapsed?0:Style.space(10)
            providers:root.bridge.providers
            showHeader:false;collapsed:root.usageCollapsed
          }
          Item {
            Layout.fillWidth: true; Layout.fillHeight: true
            visible: !root.composerExpanded
            T3TimelineMinimap {
              anchors.fill:parent;z:3
              rows:root.bridge.transcript;view:conversation;threadKey:root.bridge.selectedKey
              active:root.opened && !root.pullRequestsOpen && !root.composerExpanded
              onSelected:function(entry){root.jumpTimelineItem(entry)}
              onWheelRequested:function(angle,pixels,touchpad){conversation.wheelScroll(angle,pixels,touchpad)}
            }
            ListView {
              id: conversation
              activeFocusOnTab: true
              Keys.onPressed: function(event) {
                if([Qt.Key_PageUp,Qt.Key_PageDown,Qt.Key_Home,Qt.Key_End].indexOf(event.key)<0)return
                conversation.stopWheel();root.detachConversation()
                if(event.key===Qt.Key_PageUp){contentY=Math.max(originY,contentY-height*0.8);event.accepted=true}
                else if(event.key===Qt.Key_PageDown){contentY=Math.min(originY+Math.max(0,contentHeight-height),contentY+height*0.8);event.accepted=true}
                else if(event.key===Qt.Key_Home){positionViewAtBeginning();event.accepted=true}
                else if(event.key===Qt.Key_End){root.followingTail=true;root.settleSubmittedTail();event.accepted=true}
              }
              objectName: "conversationList"
              anchors.fill: parent; anchors.margins: Style.space(26)
              model: conversationRows
              property bool userGesture:false
              onDraggingChanged: if(dragging){userGesture=true;conversation.stopWheel();root.detachConversation()}
              onFlickStarted: {userGesture=true;root.detachConversation()}
              onContentHeightChanged: if(root.followingTail && !root.restoringConversation && !conversationWheel.running) Qt.callLater(root.settleSubmittedTail)
              onHeightChanged: if(root.followingTail && !root.restoringConversation) Qt.callLater(root.settleSubmittedTail)
              onVisibleChanged: {if(!visible)stopWheel();else if(root.followingTail)Qt.callLater(root.settleSubmittedTail)}
              onMovementEnded:{if(userGesture){userGesture=false;root.followingTail=atYEnd}root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}
              onContentYChanged: if (!root.restoringConversation && root.bridge.selectedKey) root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)
              // Match Ares ChatTranscript: virtualize, but do not pool heterogeneous
              // rows. Reusing a tall message for a short row transiently changes
              // geometry above the viewport and jumps backwards while scrolling up.
              clip: true; reuseItems: false; cacheBuffer: 0
              boundsBehavior:Flickable.StopAtBounds
              boundsMovement:Flickable.StopAtBounds
              flickDeceleration:3500
              maximumFlickVelocity:6000
              function minY(){return originY}
              function maxY(){return originY+Math.max(0,contentHeight-height)}
              property real wheelTarget:0
              property var wheelMotionState:({})
              function stopWheel(){conversationWheel.stop();wheelMotionState=({})}
              function wheelScroll(angle,pixel,touchpad){
                var delta=pixel!==0?pixel:angle*1.8
                if(delta===0)return
                var local=!(touchpad||pixel!==0?root.bridge.scrollPolicy.touchpadNative:root.bridge.scrollPolicy.mouseNative)
                var step=local?Motion.step(wheelMotionState,angle,pixel,touchpad||pixel!==0,Date.now()):null
                var from=local&&conversationWheel.running&&step.continueTarget?wheelTarget:contentY
                conversationWheel.stop();root.detachConversation()
                if(local){wheelMotionState=step.state;delta=step.delta}else wheelMotionState=({})
                wheelTarget=Math.max(minY(),Math.min(maxY(),from-delta))
                root.followingTail=wheelTarget>=maxY()-1
                if(!local){contentY=wheelTarget;root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}else conversationWheel.start()
              }
              WheelHandler {
                objectName:"t3ConversationWheel"
                target:null
                acceptedDevices:PointerDevice.Mouse|PointerDevice.TouchPad
                onWheel:function(event){if(event.angleDelta.y===0&&event.pixelDelta.y===0){event.accepted=false;return}event.accepted=true;conversation.wheelScroll(event.angleDelta.y,event.pixelDelta.y,event.device.type===PointerDevice.TouchPad)}
              }
              NumberAnimation {
                id:conversationWheel;target:conversation;property:"contentY";to:conversation.wheelTarget
                duration:Motion.glide.coasting;easing.type:Easing.OutCubic
                onFinished:{if(root.followingTail)root.settleSubmittedTail();else root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}
              }
              spacing: Style.space(22)
              Controls.ScrollBar.vertical: Controls.ScrollBar { id:conversationBar; policy: Controls.ScrollBar.AsNeeded;onPressedChanged:{if(pressed){conversation.stopWheel();root.followingTail=false;root.submissionFollowKey="";root.cancelScrollRestore()}else{root.followingTail=conversation.atYEnd;root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}} }
              // No opacity transitions on pooled rows: interrupted animations can
              // leave a recycled message or changed-file card almost invisible.
              delegate: Item {
                id: message
                required property var row
                readonly property var modelData: row
                property bool pooled:false
                ListView.onPooled:{pooled=true;opacity=1}
                ListView.onReused:{pooled=false;opacity=1}
                readonly property bool userMessage: modelData.role === "user"
                width: conversation.width
                height: body.implicitHeight + Style.space(userMessage ? 32 : 18)
                Rectangle {
                  id: bubble
                  anchors.right: parent.right
                  width: message.userMessage ? parent.width * 0.88 : parent.width
                  height: parent.height
                  radius: Style.space(16)
                  color: message.userMessage ? root.surface : "transparent"
                  Column {
                    id: body
                  anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(message.userMessage ? 16 : 0) }
                    spacing: Style.space(9)
                    Row {
                      visible: modelData.role !== "activity" && modelData.role !== "changes" && modelData.role !== "context"
                      spacing: Style.space(9)
                      Text { text: message.userMessage ? T3I18n.tr("You") : modelData.role === "assistant" ? "T3" : modelData.role; color: root.dim; font.pixelSize: Style.font.caption; font.bold: true; textFormat: Text.PlainText }
                      Text { text: root.date(modelData.createdAt); color: root.dim; font.pixelSize: Style.font.caption }
                    }
                    TextEdit {
                      id:messageText;objectName:"chatMessageText"
                      HoverHandler{objectName:"chatLinkCursor";cursorShape:messageText.hoveredLink?Qt.PointingHandCursor:Qt.IBeamCursor}
                      LayoutMirroring.enabled:false
                      visible: modelData.role !== "activity" && modelData.role !== "changes" && modelData.role !== "context"
                      width: parent.width
                      text: message.userMessage ? modelData.text || "" : (modelData.rendered ? modelData.rendered.replace(/<a /g,"<a style=\"color:"+Color.accent+";\" ") : modelData.text || "")
                      textFormat: message.userMessage || !modelData.rendered ? TextEdit.PlainText : TextEdit.RichText; readOnly: true; selectByMouse: true
                      onLinkActivated:function(link){root.bridge.openReference(link)}
                      wrapMode: TextEdit.Wrap
                      color: root.fg; selectionColor: Color.accent; selectedTextColor: Color.popups.background
                      font.pixelSize: Style.font.body
                    }
                    Flow {
                      id:attachmentFlow
                      width:parent.width;spacing:Style.space(8)
                      visible:(modelData.attachments || []).length>0
                      Repeater {
                        model:(modelData.attachments || []).filter(function(a){return a.type==="image"&&["image/png","image/jpeg","image/gif","image/webp"].indexOf(a.mimeType)>=0})
                        T3MessageImage {
                          required property var modelData
                          readonly property real viewportY:message.y+body.y+attachmentFlow.y+y-conversation.contentY
                          width:Math.min(Style.space(220),attachmentFlow.width);height:Style.space(160)
                          bridge:root.bridge;attachment:modelData
                          active:root.opened&&conversation.visible&&!message.pooled&&viewportY<conversation.height&&viewportY+height>0
                          onEnlarge:function(a){imagePopup.attachment=a;imagePopup.assetKey=root.bridge.imageKey(a);root.bridge.requestImage(a);imagePopup.open()}
                        }
                      }
                    }
                    Flow {
                      width:parent.width;spacing:Style.space(6)
                      Repeater {
                        model:modelData.fileReferences || []
                        T3FileReference {
                          required property var modelData
                          fileName:modelData.name;filePath:modelData.path
                          width:Math.min(implicitWidth,parent.width)
                          onClicked:root.bridge.openReference(modelData.path)
                        }
                      }
                      Repeater {
                        model:(modelData.attachments || []).filter(function(a){return a.type!=="image"})
                        T3FileReference {
                          required property var modelData
                          fileName:modelData.name || "HTML"
                          onClicked:root.bridge.openReference("",modelData)
                        }
                      }
                    }
                    Row {
                      visible:(modelData.role==="user"||modelData.role==="assistant")&&!!modelData.text&&modelData.streaming!==true
                      anchors.right:parent.right;spacing:Style.space(4)
                      T3CopyButton{objectName:"copyChatMessage";bridge:root.bridge;copyText:message.modelData.text || ""}
                    }
                    T3ContextDivider{visible:modelData.role==="context";width:parent.width;boundary:modelData.context || {};catalog:root.bridge.catalog;active:root.opened&&!message.pooled&&message.y<conversation.contentY+conversation.height&&message.y+message.height>conversation.contentY}
                    T3ActivityBlock{visible:modelData.role==="activity";bridge:root.bridge;activity:modelData.activity || {};active:root.opened&&!message.pooled&&message.y<conversation.contentY+conversation.height&&message.y+message.height>conversation.contentY}
                    T3Changes{visible:modelData.role==="changes";summary:modelData.changes || {};onOpenDiff:{root.bridge.requestDiff(summary);diffPopup.open()}}

                  }
                }
              }
              Connections {
                target: root.bridge
                function onMessageSubmitted(key) { root.followSubmittedMessage(key) }
                function onThreadWillChange() { root.pullRequestsOpen=false;documentPopup.close();conversation.stopWheel();root.cancelScrollRestore();imagePopup.close();diffPopup.close();root.captureState();Qt.callLater(root.restoreSidebarShelves); root.submissionFollowKey = ""; root.restoringConversation = true }
                function onSnapshotAboutToChange() {
                  if (!root.restoringSidebar) root.capturePosition(threads,root.activeRows,"@sidebar")
                  if (!root.restoringSettled) root.capturePosition(settledThreads,root.visibleSettledRows,"@settled")
                  if(!root.restoringWorking)root.capturePosition(workingThreads,root.visibleWorkingRows,"@working")
                  if(!root.restoringSnoozed)root.capturePosition(snoozedThreads,root.visibleSnoozedRows,"@snoozed")
                  root.restoringSidebar = true; root.restoringSettled = true;root.restoringWorking=true;root.restoringSnoozed=true
                }
                function onSnapshotUpdated() {
                  root.restoreSidebarShelves()
                }
                function onTranscriptChanged() {root.syncConversation()}
                function onMessagesAboutToChange() {
                  if (!root.restoringConversation) root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)
                  root.restoringConversation = true
                }
                function onMessagesUpdated() {
                  if(conversation.moving||conversationWheel.running||conversationBar.pressed){root.cancelScrollRestore();return}
                  root.restorePosition(conversation,root.bridge.transcript,root.bridge.selectedKey,true,function(){root.restoringConversation=false})
                }
              }
            }
            T3IconButton {
              objectName:"t3JumpToLatest"
              anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:Style.space(16)
              visible:conversation.count>0 && !conversation.atYEnd && !root.restoringConversation
              name:"chevron-down";tooltipText:T3I18n.tr("Jump to latest message")
              onClicked:{conversation.stopWheel();root.followingTail=true;root.cancelScrollRestore();conversation.positionViewAtEnd();root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}
            }
            Column {
              anchors.centerIn: parent
              spacing: Style.space(15)
              visible: !root.bridge.selectedThread
              Text { anchors.horizontalCenter: parent.horizontalCenter; text: "T3"; color: Color.accent; font.pixelSize: Style.space(42); font.bold: true; opacity: 0.55 }
              Text { anchors.horizontalCenter: parent.horizontalCenter; text: T3I18n.tr("Your work, within reach."); color: root.fg; font.pixelSize: Style.space(18) }
              Text { anchors.horizontalCenter: parent.horizontalCenter; text: T3I18n.tr("Choose a thread to continue the conversation"); color: root.dim; font.pixelSize: Style.font.body }
            }
            Row {
              anchors.centerIn: parent; spacing: Style.space(9)
              visible: root.bridge.loadingMessages && root.bridge.transcript.length === 0
              WorkIndicator { active: root.opened && parent.visible }
              Text { text: T3I18n.tr("Loading conversation…"); color: root.dim; font.pixelSize: Style.font.body }
            }
          }
          RowLayout {
            Layout.fillWidth: true; Layout.leftMargin: Style.space(26); Layout.rightMargin: Style.space(26); Layout.bottomMargin: Style.space(12)
            objectName:"t3LiveContext"
            visible: !!root.bridge.selectedInfo.working && !root.composerExpanded
            WorkIndicator { active: root.opened && parent.visible; tint: T3Palette.working }
            T3ActivityText {
              Layout.fillWidth: true
              active: root.opened && parent.visible
              tint: root.fg
              objectName:"t3LiveContextText"
              text: root.bridge.activityLabel()
            }
          }
          T3Queue {
            objectName:"t3QueueControl"
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(5)
            bridge:root.bridge
            onEditSelected:composerPanel.inputField.forceActiveFocus()
          }
          T3Tasks{
            objectName:"t3TasksControl"
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(5)
            bridge:root.bridge
          }
          T3Questions {
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(8)
            bridge:root.bridge
          }
          T3Composer {
            id: composerPanel
            Layout.fillWidth: true; Layout.leftMargin: Style.space(24); Layout.rightMargin: Style.space(24); Layout.bottomMargin: Style.space(18)
            Layout.fillHeight: root.composerExpanded
            Layout.preferredHeight: root.composerExpanded ? 0 : implicitHeight
            visible: !!root.bridge.selectedThread && !(root.bridge.questions || []).length
            bridge: root.bridge
            expanded: root.composerExpanded
            onToggleExpanded: root.expandComposer()
          }
        }
      }
    }
  }
}
