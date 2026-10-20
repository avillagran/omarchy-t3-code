import QtQuick 6.11
import QtQuick.Window 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import Quickshell
import Quickshell.Wayland
import qs.Commons
import qs.Ui
import "../js/SidebarModel.js" as Sidebar
import "../js/MarkdownLinks.js" as MarkdownLinks
import "../js/MarkdownStyle.js" as MarkdownStyle
import "../js/ProjectModel.js" as Projects
import "../js/WheelMotion.js" as Motion
import "../js/NativeKeybindings.js" as Native
import "../js/KeybindingLabels.js" as KeyLabels

Item {
  id: root
  property var anchorItem: null
  property var bar: null
  required property var bridge
  property var prTarget:null
  property bool pullRequestsOpen:false
  // Multi-purpose side panel next to the chat (T3SidePanel): browser,
  // HTML renders and device screens share one resizable slot, one tab at a
  // time; narrow windows show it as a sheet over the chat.
  property bool sidePanelOpen:false
  property string sideTab:"browser"
  readonly property bool sideShown:sidePanelOpen&&opened&&!!bridge.selectedThread&&!preferencesOpen&&!pullRequestsOpen&&!composerExpanded
  readonly property bool browserOpen:sidePanelOpen&&sideTab==="browser"
  function openSide(tab){sideTab=tab;sidePanelOpen=true;if(composerExpanded)expandComposer(false)}
  function closeSide(){sidePanelOpen=false}
  function toggleSide(){if(sidePanelOpen)closeSide();else openSide(sideTab)}
  readonly property var sidePanelItem:browserInline?inlineSideLoader.item:overlaySideLoader.item
  property real browserPanelWidth:Math.max(360,Math.min(1600,bridge.viewState.browserPanelWidth || 540))
  readonly property real browserAvailableWidth:panelLayout.width-(sidebarReveal.visible?sidebarReveal.width+sidebarResizer.width:0)
  readonly property bool browserInline:browserAvailableWidth>=Style.space(760)
  readonly property bool browserShown:browserOpen&&opened&&!!bridge.selectedThread&&!preferencesOpen&&!pullRequestsOpen&&!composerExpanded
  readonly property real browserMaxWidth:Math.max(360,Math.min(Math.floor(panelLayout.width/Style.space(1)*0.7),Math.floor(browserAvailableWidth/Style.space(1))-366))
  readonly property var browserWorkspaceItem:sidePanelItem?sidePanelItem.browserItem:null
  Binding{target:T3Ticker;property:"mode";value:["saver","max"].indexOf(root.bridge.viewState.animationRate)>=0?root.bridge.viewState.animationRate:"adaptive"}
  // HTML renders: inline in the conversation, or in a side panel when the
  // window is wide enough (Preferences > HTML renders).
  readonly property bool renderSide:bridge.viewState.htmlRenders==="side"&&browserInline
  property var renderAttachment:null
  property var renderList:[]
  readonly property bool renderShown:sideShown&&sideTab==="renders"&&!!renderAttachment
  property var renderSeen:({})
  function showRender(attachment){renderAttachment=attachment;openSide("renders")}
  // Renders that arrive while a thread is open go straight to the panel;
  // those already in the history only open on request.
  function followRenders(){
    var key=bridge.selectedKey;if(!key)return
    var ids=[],latest=null
    bridge.transcript.forEach(function(r){(r.attachments || []).forEach(function(a){if(a.mimeType==="text/html"){ids.push(a.id);latest=a}})})
    var seen=renderSeen[key],next=Object.assign({},renderSeen)
    if(seen&&renderSide&&latest&&seen.indexOf(latest.id)<0)showRender(latest)
    next[key]=ids;renderSeen=next
    var list=[];bridge.transcript.forEach(function(r){(r.attachments || []).forEach(function(a){if(a.mimeType==="text/html")list.push(a)})})
    if(JSON.stringify(list)!==JSON.stringify(renderList))renderList=list
    if(!renderAttachment&&latest)renderAttachment=latest
  }
  Connections{target:root.bridge;function onMessagesUpdated(){root.followRenders()}function onSelectedKeyChanged(){if(root.sideTab==="renders")root.closeSide();root.renderAttachment=null;root.renderList=[]}}
  // Diagnostics (IPC profileScroll): scroll like a touchpad (pixel deltas
  // every 8 ms through the normal wheel path) and record the intervals
  // between presented frames; profileResult reads them back.
  property var profile:({})
  property var profileFrames:[]
  property double profileLast:0
  property int profileTicks:0
  function profileScroll(){
    if(!conversation.visible||profilePad.running)return
    profileFrames=[];profileLast=0;profileTicks=0;profile={running:true}
    profilePad.start()
  }
  Connections{target:profilePad.running?panelLayout.Window.window:null;function onFrameSwapped(){var n=Date.now();if(root.profileLast)root.profileFrames.push(n-root.profileLast);root.profileLast=n}}
  Timer{id:profilePad;interval:8;repeat:true
    onTriggered:{
      conversation.wheelScroll(0,root.profileTicks<250?24:-24,true)
      if(++root.profileTicks<500)return
      stop();var a=root.profileFrames.slice().sort(function(x,y){return x-y}),sum=a.reduce(function(s,v){return s+v},0)
      root.profile={running:false,frames:a.length,fps:Math.round(1000*a.length/Math.max(1,sum)),p50:a[Math.floor(a.length*.5)],p90:a[Math.floor(a.length*.9)],p99:a[Math.floor(a.length*.99)],max:a[a.length-1],over12:a.filter(function(v){return v>12}).length,over20:a.filter(function(v){return v>20}).length}
    }
  }
  function resizeBrowser(width){browserPanelWidth=Math.max(360,Math.min(browserMaxWidth,width));bridge.rememberView({browserPanelWidth:browserPanelWidth})}
  function toggleBrowser(){if(browserOpen)closeSide();else openSide("browser")}
  function focusBrowserUrl(){if(browserWorkspaceItem)browserWorkspaceItem.focusUrl()}
  function browserKeyFocus(){var item=nativeFocusItem;while(item){if(item.objectName==="t3BrowserWorkspace")return true;item=item.parent}return false}
  readonly property bool browserFocused:browserShown&&browserKeyFocus()
  property bool preferencesOpen:false
  property string preferencesProjectKey:""
  function openProjectSettings(key){preferencesProjectKey=key;preferencesOpen=true;var loader=preferencesLoader;if(loader&&loader.item){loader.item.initialProjectKey=key;loader.item.selectInitialProject()}}
  onPreferencesOpenChanged:{if(preferencesOpen){captureState();conversation.stopWheel();pullRequestsOpen=false}else if(opened){restoringConversation=true;restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false})}}
  onPullRequestsOpenChanged:{if(!pullRequestsOpen)prTarget=null;if(initialized){if(pullRequestsOpen)conversation.stopWheel();else if(opened){restoringConversation=true;restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false})}}}
  function submitCurrent(){if(!preferencesOpen&&!pullRequestsOpen&&!documentPopup.visible)bridge.submit()}
  function selectSidebarThread(env,id){pullRequestsOpen=false;bridge.selectThread(env,id)}
  property bool initialized: false
  readonly property var conversationView: conversation
  // Native comfortable lane: 46rem, shared by timeline and composer.
  readonly property real chatContentMaxWidth:Style.space(736)
  property bool findOpen:false
  property string findQuery:""
  property int findIndex:-1
  readonly property var findMatches:{
    var q=findQuery.toLocaleLowerCase(),matches=[]
    if(!q)return matches
    bridge.transcript.forEach(function(r){if(r.role!=="user"&&r.role!=="assistant")return;var text=(r.searchText!==undefined?r.searchText:r.text || "").toLocaleLowerCase(),start=0,ordinal=0;while((start=text.indexOf(q,start))>=0){matches.push({id:r.id,ordinal:ordinal++});start+=q.length}})
    return matches
  }
  onFindQueryChanged:{findIndex=-1;findDebounce.restart()}
  function openFind(){findOpen=true;if(composerExpanded)expandComposer(false);Qt.callLater(function(){chatFindInput.forceActiveFocus();chatFindInput.selectAll()})}
  function navigateFind(step){
    if(!findMatches.length){findIndex=-1;return}
    findIndex=(findIndex+step+findMatches.length)%findMatches.length
    var match=findMatches[findIndex];jumpTimelineItem(match);findSelection.restart()
  }
  Timer{id:findDebounce;interval:140;onTriggered:if(root.findOpen)root.navigateFind(1)}
  Timer{id:findSelection;interval:90;onTriggered:{
    var m=root.findMatches[root.findIndex];if(!root.findOpen||!m)return
    if(root.restoringConversation){restart();return}
    var i=root.bridge.transcript.findIndex(function(r){return r.id===m.id}),item=conversation.itemAtIndex(i)
    if(!item||!item.textItem)return
    var edit=item.textItem,text=edit.getText(0,edit.length).toLocaleLowerCase(),q=root.findQuery.toLocaleLowerCase(),pos=-1
    for(var n=0;n<=m.ordinal;n++){
      do{pos=text.indexOf(q,pos+1)}while(pos>=0&&root.codeHeaderAt(edit,text,pos))
      if(pos<0)return
    }
    edit.select(pos,pos+q.length)
  }}
  readonly property var composerView: composerPanel
  readonly property var searchInput: searchField
  readonly property var sidebarView: threads
  property bool usageCollapsed: bridge.viewState.usageExpanded !== true
  onUsageCollapsedChanged: if(initialized && bridge.viewState.usageExpanded !== !usageCollapsed) bridge.rememberView({usageExpanded:!usageCollapsed,quotaCollapsed:usageCollapsed})
  property real sidebarWidth: Math.max(260,Math.min(600,bridge.viewState.sidebarWidth || 300))
  // Shelf heights in logical px, resized from the handle above each shelf.
  property real workingShelfHeight: Math.max(60,Math.min(1200,bridge.viewState.workingShelfHeight || 220))
  property real snoozedShelfHeight: Math.max(60,Math.min(1200,bridge.viewState.snoozedShelfHeight || 164))
  property real settledShelfHeight: Math.max(60,Math.min(1200,bridge.viewState.settledShelfHeight || 220))
  function resizeShelf(key,height) { var h=Math.round(Math.max(60,Math.min(1200,height)));root[key]=h;var v={};v[key]=h;bridge.rememberView(v) }
  function resizeSidebar(width) { sidebarWidth=Math.max(260,Math.min(600,width));bridge.rememberView({sidebarWidth:sidebarWidth}) }
  property bool composerExpanded: !!bridge.viewState.composerExpanded
  property real panelWidth: bridge.viewState.panelWidth || 1120
  property real panelHeight: bridge.viewState.panelHeight || 780
  function resizePanel(w,h) {
    if(detached){desktop.implicitWidth=Style.space(Math.max(760,w));desktop.implicitHeight=Style.space(Math.max(360,h));return}
    panelWidth=Math.max(760,Math.min(4000,w));panelHeight=Math.max(360,Math.min(3000,h))
    bridge.rememberView({panelWidth:panelWidth,panelHeight:panelHeight})
  }
  function expandComposer(focusEditor) {
    conversation.stopWheel();if(!composerExpanded)captureState()
    composerExpanded=!composerExpanded;bridge.rememberView({composerExpanded:composerExpanded})
    if(!composerExpanded) {restoringConversation=true;restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false});restoringSidebar=true;restorePosition(threads,activeRows,"@sidebar",false,function(){root.restoringSidebar=false})}
    if(focusEditor !== false)Qt.callLater(function(){composerPanel.inputField.forceActiveFocus()})
  }
  function focusSidebar() {
    if(composerExpanded)expandComposer(false)
    if(sidebarCollapsed)toggleSidebar(false)
    var items=sidebarFocusItems(),choice=items.find(function(r){return r.key===root.bridge.selectedKey})
    if(!choice){var groups=[{rows:workingRows,header:workingToggle},{rows:snoozedRows,header:snoozedToggle},{rows:settledRows,header:settledToggle}];var group=groups.find(function(g){return g.rows.some(function(t){return t.key===root.bridge.selectedKey})});if(group)choice={item:group.header,index:-1}}
    choice=choice||items[0]||{item:threads,index:0};choice.item.forceActiveFocus()
    if(choice.index>=0){choice.item.currentIndex=choice.index;choice.item.positionViewAtIndex(choice.index,ListView.Contain)}
  }
  // One focus order across cards and shelf headers, matching the native sidebar.
  function sidebarFocusItems(){
    var out=[]
    function list(view,rows){rows.forEach(function(t,i){out.push({item:view,index:i,key:t.key})})}
    list(threads,activeRows)
    if(workingRows.length){out.push({item:workingToggle,index:-1});list(workingThreads,visibleWorkingRows)}
    if(snoozedRows.length){out.push({item:snoozedToggle,index:-1});list(snoozedThreads,visibleSnoozedRows)}
    if(settledRows.length){out.push({item:settledToggle,index:-1});list(settledThreads,visibleSettledRows)}
    return out
  }
  function moveSidebarFocus(item,step){
    var rows=sidebarFocusItems(),index=rows.findIndex(function(r){return r.item===item&&(r.index<0||r.index===item.currentIndex)})
    if(!rows.length)return
    var next=rows[Math.max(0,Math.min(rows.length-1,index+step))]
    next.item.forceActiveFocus()
    if(next.index>=0){next.item.currentIndex=next.index;next.item.positionViewAtIndex(next.index,ListView.Contain)}
  }
  function navigateThread(step) { var rows=activeRows.concat(visibleWorkingRows).concat(visibleSnoozedRows).concat(visibleSettledRows);if(!rows.length)return;var index=rows.findIndex(function(t){return t.key===root.bridge.selectedKey});var t=rows[(index+step+rows.length)%rows.length];bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus() }
  readonly property bool nativeKeybindingsEnabled:typeof bridge.keybindingRulesFor==="function"
  readonly property var nativeShortcutController:nativeShortcuts
  readonly property var nativePrimaryKeys:nativeKeybindingsEnabled?bridge.keybindingRulesFor(bridge.primaryKeybindingEnvironment):[]
  readonly property var nativeScriptKeys:nativeKeybindingsEnabled?bridge.keybindingRulesFor(bridge.selectedEnvironment):[]
  readonly property var lastUserMessage:(bridge.transcript || []).filter(function(m){return m.role==="user"}).slice(-1)[0] || null
  readonly property var nativeFocusItem:panelLayout.Window.activeFocusItem
  function editableKeyFocus(){var item=nativeFocusItem;while(item){if(typeof item.cursorPosition==="number"&&item.readOnly!==true||typeof item.editText==="string")return true;item=item.parent}return false}
  readonly property var nativeShortcutContext:({terminalFocus:false,terminalOpen:false,previewFocus:root.browserFocused,previewOpen:root.browserShown,editableFocus:editableKeyFocus(),modelPickerOpen:composerPanel.modelPickerOpen,composerFocus:composerPanel.inputField.activeFocus,draftThreadRoute:false,turnRunning:!!bridge.selectedInfo.activeRunId,isWeb:false,isDesktop:true})
  readonly property bool nativeModalOpen:documentPopup.visible||imagePopup.visible||diffPopup.visible||toolPopup.visible||gitPopup.visible||keyHelp.visible||rewindWorkflow.opened||sidebarActions.dialogActive||composerPanel.permissionPickerOpen
  readonly property bool nativeKeysActive:shortcutActive&&!preferencesOpen&&!pullRequestsOpen&&!nativeModalOpen&&!(bridge.keybindingCaptureActive || false)
  Connections{target:root.bridge;ignoreUnknownSignals:true
    function onSettleNavigationPlanning(){if(root.opened)root.bridge.settleNavigationOrder=root.activeRows.concat(root.workingRows).concat(root.snoozedRows).concat(root.settledRows)}
    function onSettledLastThread(thread){
      // Existing management dialogs close on the success signal; open afterwards.
      var epoch=root.bridge.selectionEpoch
      Qt.callLater(function(){if(root.opened&&!root.bridge.selectedKey&&root.bridge.selectionEpoch===epoch)sidebarActions.newThread(null,thread.projectKey)})
    }
  }
  function nativeThreadRows(){return activeRows.concat(visibleWorkingRows).concat(visibleSnoozedRows).concat(visibleSettledRows)}
  function supportsNativeShortcut(command){
    if(!nativeKeybindingsEnabled)return false
    if(workingPopup.visible)return command==="diff.toggle"
    if(composerPanel.modelPickerOpen)return command.indexOf("modelPicker.")===0
    if(["sidebar.toggle","chat.find"].indexOf(command)>=0)return command!=="chat.find"||!!bridge.selectedThread
    if(command==="chat.new"||command==="chat.newLocal")return (bridge.state.projects || []).length>0&&!bridge.managementBusy&&!bridge.managementRequest
    if(command==="thread.previous"||command==="thread.next")return nativeThreadRows().length>1
    var jump=/^thread\.jump\.([1-9])$/.exec(command);if(jump)return nativeThreadRows().length>=Number(jump[1])
    if(command==="preview.toggle")return !!bridge.selectedThread
    if(command==="preview.focusUrl")return browserShown
    if(command==="preview.refresh")return browserShown&&!!bridge.previewState&&!!bridge.previewState.snapshot&&bridge.previewWatch.canOperate&&!bridge.previewWatch.busy
    if(["preview.zoomIn","preview.zoomOut","preview.resetZoom"].indexOf(command)>=0)return browserShown&&!!bridge.browserSurface.interactive
    if(command==="terminal.toggle")return bridge.canOpenTerminal===true
    if(command==="diff.toggle")return bridge.canOpenGit===true
    if(command==="modelPicker.toggle")return composerPanel.modelPickerAvailable
    if(command==="composer.mode")return composerPanel.permissionPickerAvailable
    if(command==="composer.effort")return !composerPanel.locked&&composerPanel.descriptors.some(function(d){return /effort|thinking|reasoning/i.test(d.id+" "+d.label)})
    if(command==="thread.undo")return bridge.canRewind===true&&!bridge.rewindPending&&!((bridge.rewindPreview || {}).busy)&&!!lastUserMessage&&lastUserMessage.canRewindMessage===true
    if(command==="thread.stop")return !!bridge.selectedInfo.activeRunId&&!bridge.draft.stopping&&!bridge.providerSubagent
    if(command==="thread.copyReference")return !!bridge.selectedThread
    if(command==="composer.stash")return !!bridge.selectedInfo.projectId&&!bridge.draft.loading&&!bridge.draft.loadError&&!bridge.draft.busy&&!bridge.draft.attempt&&!bridge.draft.queueEdit&&!((bridge.projectDrafts || {}).busy)&&(!!bridge.draft.text.trim()||bridge.draft.attachments.length>0)
    if(command==="composer.sendAndNewThread")return !composerPanel.locked&&composerPanel.hasContent&&!bridge.draft.queueEdit
    if(bridge.projectScriptId(command)!==null)return bridge.canOpenGit&&!bridge.gitBusy&&!bridge.scriptShortcutPending&&["pending","unknown"].indexOf(bridge.gitView.operation.state)<0
    if(command==="thread.steerQueuedMessage"){var q=bridge.queue.items[0];return !!q&&!q.pending&&q.available!==false&&!!bridge.queue.canSteer&&!bridge.queueOperation.busy&&!bridge.queueOperation.command&&!bridge.draft.busy&&!bridge.draft.attempt}
    if(command==="thread.editQueuedMessage"){var items=bridge.queue.items || [],q=items[items.length-1];return !!q&&!q.pending&&q.available!==false&&!bridge.draft.queueEdit&&!bridge.draft.busy&&!bridge.draft.attempt&&composerPanel.inputField.activeFocus&&composerPanel.inputField.cursorPosition===0}
    return false
  }
  function triggerNativeShortcut(command){
    if(!nativeKeysActive||!supportsNativeShortcut(command))return
    if(command==="sidebar.toggle")toggleSidebar()
    else if(command==="chat.find")openFind()
    else if(command==="chat.new"||command==="chat.newLocal")sidebarActions.newThread(command==="chat.newLocal"?{type:"root"}:null)
    else if(command==="thread.previous"||command==="thread.next")navigateThread(command==="thread.next"?1:-1)
    else if(/^thread\.jump\.[1-9]$/.test(command)){var t=nativeThreadRows()[Number(command.split(".").pop())-1];bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}
    else if(command==="preview.toggle")toggleBrowser()
    else if(command==="preview.focusUrl")focusBrowserUrl()
    else if(command==="preview.refresh"){if(browserWorkspaceItem)browserWorkspaceItem.refresh()}
    else if(command==="preview.zoomIn"||command==="preview.zoomOut"||command==="preview.resetZoom")bridge.browserControl(command==="preview.zoomIn"?"zoom-in":command==="preview.zoomOut"?"zoom-out":"zoom-reset")
    else if(command==="terminal.toggle")bridge.openTerminal()
    else if(command==="diff.toggle"){if(workingPopup.visible)workingPopup.close();else{bridge.requestWorkingReview("working-tree","",bridge.diffPreferences.ignoreWhitespace,"");workingPopup.open()}}
    else if(command.indexOf("modelPicker.")===0||command==="composer.mode"||command==="composer.effort")composerPanel.nativeModelControl(command)
    else if(command==="thread.undo")bridge.requestRewindPreview(lastUserMessage.id)
    else if(command==="thread.stop")bridge.stop()
    else if(command==="thread.copyReference"){var badge=bridge.selectedInfo.pullRequestBadge || {};bridge.copyText((badge.current || {}).url || bridge.selectedThread)}
    else if(command==="composer.stash")bridge.requestProjectDrafts("stash")
    else if(command==="composer.sendAndNewThread")bridge.submit(true)
    else if(bridge.projectScriptId(command)!==null){if(bridge.requestScriptShortcut(command))gitPopup.open()}
    else if(command==="thread.steerQueuedMessage")bridge.queueAction("steer",{runId:bridge.queue.items[0].runId,targetRunId:bridge.queue.activeRunId})
    else if(command==="thread.editQueuedMessage"){var items=bridge.queue.items;bridge.beginQueueEdit(items[items.length-1]);composerPanel.inputField.forceActiveFocus()}
  }
  function localShortcutAvailable(sequence){return !nativeKeybindingsEnabled||!nativeShortcuts.claimsSequence(sequence)}
  Component.onCompleted: { initialized = true;syncConversation() }
  property bool panelOpen:false
  property string windowMode:bridge.viewState.windowMode || (bridge.viewState.detached?"tiled":"panel")
  readonly property bool detached:windowMode!=="panel"
  readonly property bool opened:panelOpen
  readonly property var managedToplevel:ToplevelManager.toplevels.values.find(function(t){return t.title===desktop.title&&t.appId===Quickshell.appId}) || null
  onManagedToplevelChanged:if(opened&&detached&&managedToplevel){applyWindowMode.restart();Qt.callLater(function(){if(root.managedToplevel)root.managedToplevel.activate()})}
  readonly property bool shortcutActive:opened&&(!detached||!!(managedToplevel&&managedToplevel.activated))
  Connections{target:root.bridge;function onTerminalOpened(){if(!root.detached)root.close()}}
  // The bar panel covers the screen, so step aside while a screenshot is taken.
  property bool screenshotHidden:false
  Connections{target:root.bridge;function onScreenshotCapture(active){if(active){if(root.opened&&!root.detached&&root.bridge.screenshotHidesPanel){root.screenshotHidden=true;root.close()}}else if(root.screenshotHidden){root.screenshotHidden=false;root.open();Qt.callLater(function(){composerPanel.inputField.forceActiveFocus()})}}}
  readonly property var normalWindow:desktop
  function setDetached(value){setWindowMode(value?"floating":"panel")}
  function cycleWindowMode(){setWindowMode(windowMode==="panel"?"floating":windowMode==="floating"?"tiled":"panel")}
  function setWindowMode(value){
    if(windowMode===value)return;windowApplyAttempts=0
    conversation.stopWheel();captureState();documentPopup.close();imagePopup.close();diffPopup.close();workingPopup.close();toolPopup.close();gitPopup.close();rewindWorkflow.dismiss();keyHelp.close();filterMenu.close();projectMenu.dismissContext()
    restoringConversation=true;restoringSidebar=true
    windowMode=value;bridge.rememberView({windowMode:value,detached:value!=="panel"});if(detached)applyWindowMode.restart()
    Qt.callLater(function(){root.restoreSidebarShelves();root.restorePosition(conversation,bridge.transcript,bridge.selectedKey,true,function(){root.restoringConversation=false});composerPanel.inputField.forceActiveFocus()})
  }
  property int windowApplyAttempts:0
  Timer{id:applyWindowMode;interval:150;onTriggered:if(root.opened&&root.detached&&root.managedToplevel){root.windowApplyAttempts++;root.bridge.applyWindowLayout(root.windowMode,Quickshell.processId)}}
  Connections{target:root.bridge;function onWindowLayoutFinished(error){if(error==="window_not_ready"&&root.windowApplyAttempts<6&&root.opened&&root.detached)applyWindowMode.restart()}}
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.57)
  readonly property color divider: Qt.rgba(fg.r, fg.g, fg.b, 0.09)
  // Stock Controls (buttons, check boxes, text areas, popups) follow the
  // Omarchy theme through the palette instead of Qt's light default.
  readonly property color themeSurface: Color.popups.background
  palette.window:root.themeSurface;palette.windowText:root.fg;palette.base:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05));palette.alternateBase:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08));palette.text:root.fg;palette.button:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08));palette.buttonText:root.fg;palette.highlight:Color.accent;palette.highlightedText:root.themeSurface;palette.placeholderText:root.dim;palette.toolTipBase:root.themeSurface;palette.toolTipText:root.fg;palette.link:Color.accent;palette.light:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16));palette.midlight:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12));palette.mid:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2));palette.dark:Qt.darker(root.themeSurface,1.4);palette.shadow:Qt.rgba(0,0,0,0.5)
  readonly property color surface: Qt.rgba(fg.r, fg.g, fg.b, 0.045)
  readonly property color healthy: Color.accent
  property string statusFilter: bridge.viewState.statusFilter || "all"
  property string projectFilter: bridge.viewState.projectFilter || ""
  Connections{target:root.bridge;function onProjectOpened(environment,projectId,error){if(!error){root.statusFilter="all";root.projectFilter="";root.bridge.query=""}}}
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
    // Rows from the client carry a content revision; local rows fall back to
    // their JSON. Rows usually keep their order, so check in place first.
    for(var i=0;i<rows.length;i++){
      var row=rows[i],found=ids[i]===row.id?i:ids.indexOf(row.id),print=row.rev!==undefined?row.rev:JSON.stringify(row)
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
  Timer { interval: 1000; repeat: true; running: root.opened && root.bridge.state.threads.some(function(t){return !!t.working||!!t.snoozed}); onTriggered: root.clockNow = Date.now() }
  function duration(start) {
    if (!start) return ""
    var seconds = Math.max(0,Math.floor((clockNow-start)/1000))
    return seconds < 60 ? seconds + "s" : Math.floor(seconds/60) + "m " + (seconds%60) + "s"
  }
  readonly property var projectGroups:Projects.groups(bridge.state.projects,Object.assign({},bridge.viewState,{nativeProjectGrouping:bridge.state.nativeProjectGrouping}))
  readonly property var projectScope:Projects.scope(projectFilter,projectGroups)
  readonly property var sidebarRows: Sidebar.rows(bridge.state.threads, statusFilter, projectScope, settledExpanded, bridge.query)
  readonly property bool settledShown: settledExpanded
  readonly property var inboxRows: sidebarRows.filter(function(t) { return !t.sectionHeader && !t.settled && !t.snoozed })
  readonly property var activeRows:inboxRows.filter(function(t){return !root.sidebarCompact || !Sidebar.inWorkingShelf(t)}).sort(function(a,b){return Sidebar.compareActive(a,b,root.sidebarCompact,bridge.inboxReturnState.returned)})
  readonly property var workingRows:sidebarCompact ? inboxRows.filter(Sidebar.inWorkingShelf).sort(function(a,b){return (b.lastSentAt || b.startedAt || b.updatedAt || 0)-(a.lastSentAt || a.startedAt || a.updatedAt || 0)}) : []
  readonly property var visibleWorkingRows:workingExpanded ? workingRows : []
  readonly property var visibleSnoozedRows:snoozedExpanded ? snoozedRows : []
  readonly property var visibleSettledRows:settledShown ? settledRows : []
  readonly property var settledRows: Sidebar.rows(bridge.state.threads,statusFilter,projectScope,true,bridge.query).filter(function(t) { return !t.sectionHeader && t.settled && !t.snoozed }).sort(Sidebar.compareSettled)
  readonly property var snoozedRows: Sidebar.rows(bridge.state.threads,statusFilter,projectFilter,true,bridge.query).filter(function(t){return !t.sectionHeader && !!t.snoozed})
  readonly property var projectItems: [{key: "", title: T3I18n.tr("All projects")}].concat(projectGroups)
  function kind(t) { return Sidebar.state(t) }
  // Last seen state per thread and when it changed, so rows can celebrate a
  // transition even after moving to another sidebar section.
  readonly property var starfield: sky
  Binding { target: T3Motion; property: "style"; value: root.bridge.viewState.transitionStyle || "nebula" }
  Binding { target: T3Motion; property: "pace"; value: ({slow: 1.6, fast: 0.6})[root.bridge.viewState.transitionSpeed] || 1 }
  // Preferences > Appearance > Animations, resolved once for every row.
  readonly property var threadEffects: {
    var v = bridge.viewState
    return {working: v.threadWorkingEffect || "comet", input: v.threadInputEffect || "glints", change: v.threadChangeEffect || "star",
            intensity: ({subtle: 0.55, vivid: 1.6})[v.threadEffectIntensity] || 1}
  }
  // Sidebar sections are keyed models rather than plain arrays, so a state
  // change moves, inserts and removes rows instead of resetting the views.
  // Snapshot-driven changes animate: a row changing section flies from its
  // old place to its new one (T3RowFlight), the rows around both slide to
  // make room, and plain arrivals and departures fade. Preferences >
  // Animations picks the path and speed; "off" keeps every list still.
  ListModel { id: activeRowModel; dynamicRoles: true }
  ListModel { id: workingRowModel; dynamicRoles: true }
  ListModel { id: snoozedRowModel; dynamicRoles: true }
  ListModel { id: settledRowModel; dynamicRoles: true }
  readonly property string rowMoveEffect: ["arc","glide","fade","off"].indexOf(bridge.viewState.threadMoveEffect) >= 0 ? bridge.viewState.threadMoveEffect : "arc"
  readonly property int rowMoveMs: ({slow: 820, fast: 300})[bridge.viewState.threadMoveSpeed] || 520
  readonly property var sidebarSections: [activeRows, workingReveal.present ? workingRows : [], snoozedReveal.present ? snoozedRows : [], settledReveal.present ? settledRows : []]
  onSidebarSectionsChanged: Qt.callLater(syncSidebar)
  property var sidebarSyncState: [{ids: [], prints: {}}, {ids: [], prints: {}}, {ids: [], prints: {}}, {ids: [], prints: {}}]
  property bool rowMotionPending: false
  property bool rowMotion: false
  property var movingKeys: ({})
  property var leavingRows: ({})
  property var arrivingRows: []
  Connections { target: root.bridge; function onSnapshotAboutToChange() { root.rowMotionPending = true } }
  Timer { id: rowMotionHold; interval: root.rowMoveMs + 120; onTriggered: { root.rowMotion = false; root.movingKeys = ({}) } }
  function syncSidebar() {
    var models = [activeRowModel, workingRowModel, snoozedRowModel, settledRowModel], sections = sidebarSections
    var states = sidebarSyncState.map(function(st) { return {ids: st.ids.slice(), prints: Object.assign({}, st.prints)} })
    var removed = {}, added = {}, moving = {}, changed = false
    sections.forEach(function(rows, n) {
      var have = {}, want = {}
      states[n].ids.forEach(function(id) { have[id] = true })
      rows.forEach(function(r) { var id = r.key || r.id; want[id] = true; if (!have[id]) added[id] = true })
      states[n].ids.forEach(function(id) { if (!want[id]) removed[id] = true })
    })
    Object.keys(removed).forEach(function(id) { if (added[id]) moving[id] = true })
    var animate = rowMotionPending && rowMoveEffect !== "off" && opened
    rowMotionPending = false
    if (animate && (Object.keys(removed).length || Object.keys(added).length || sections.some(function(rows, n) { return rows.some(function(r, i) { return states[n].ids[i] !== (r.key || r.id) }) }))) {
      rowMotion = true; movingKeys = rowMoveEffect === "fade" ? {} : moving; rowMotionHold.restart()
    }
    sections.forEach(function(rows, n) {
      var model = models[n], ids = states[n].ids, prints = states[n].prints, wanted = {}
      rows.forEach(function(r) { wanted[r.key || r.id] = true })
      for (var k = ids.length - 1; k >= 0; k--) if (!wanted[ids[k]]) { model.remove(k); delete prints[ids[k]]; ids.splice(k, 1); changed = true }
      for (var i = 0; i < rows.length; i++) {
        var row = rows[i], id = row.key || row.id, found = ids.indexOf(id), print = JSON.stringify(row)
        if (found < 0) { model.insert(i, {row: row}); ids.splice(i, 0, id); changed = true }
        else {
          if (found !== i) { model.move(found, i, 1); ids.splice(i, 0, ids.splice(found, 1)[0]); changed = true }
          if (prints[id] !== print) model.setProperty(i, "row", row)
        }
        prints[id] = print
      }
    })
    sidebarSyncState = states
    if (changed && rowMotion) Qt.callLater(startFlights)
  }
  // Called from the row delegates as the views add and remove them.
  function rowLeaving(item) {
    if (!rowMotion) return
    var key = item.thread.key || item.thread.id
    if (movingKeys[key]) { item.flight = true; var next = Object.assign({}, leavingRows); next[key] = {item: item, rect: item.mapToItem(sidebar, 0, 0, item.width, item.height)}; leavingRows = next }
    else item.depart(Math.round(rowMoveMs * 0.6))
  }
  function rowArriving(item) {
    if (!rowMotion) return
    var key = item.thread.key || item.thread.id
    if (movingKeys[key]) { item.flight = true; arrivingRows = arrivingRows.concat([item]) }
    else item.arrive(Math.round(rowMoveMs * 0.8))
  }
  function sectionHeader(key) {
    if (workingRows.some(function(t) { return t.key === key })) return workingToggle
    if (snoozedRows.some(function(t) { return t.key === key })) return snoozedToggle
    if (settledRows.some(function(t) { return t.key === key })) return settledToggle
    return threads
  }
  function startFlights() {
    var leaving = leavingRows, arriving = arrivingRows, landed = {}
    leavingRows = ({}); arrivingRows = []
    ;[threads, workingThreads, snoozedThreads, settledThreads].forEach(function(view) { view.forceLayout() })
    arriving.forEach(function(item) {
      var key = item.thread.key || item.thread.id, from = leaving[key]
      if (!from) { item.flight = false; item.arrive(Math.round(rowMoveMs * 0.8)); return }
      landed[key] = true
      rowFlight.createObject(flightLayer, {key: key, source: from.item, target: item, landing: sectionHeader(key), start: from.rect, path: rowMoveEffect, duration: rowMoveMs})
    })
    // The destination row is not on screen (collapsed shelf or scrolled
    // away): the card sinks into its section instead.
    Object.keys(leaving).forEach(function(key) {
      if (landed[key]) return
      var from = leaving[key]
      rowFlight.createObject(flightLayer, {key: key, source: from.item, landing: sectionHeader(key), start: from.rect, path: rowMoveEffect, duration: rowMoveMs})
    })
  }
  Component { id: rowFlight; T3RowFlight {} }
  property var threadKinds: ({})
  property var stateChangedAt: ({})
  function trackThreadStates() {
    var previous = threadKinds, next = {}, changed = {}, now = Date.now(), dirty = false
    Object.keys(stateChangedAt).forEach(function(key) { if (now - stateChangedAt[key] < 5000) changed[key] = stateChangedAt[key]; else dirty = true })
    ;(bridge.state.threads || []).forEach(function(t) {
      var k = kind(t); next[t.key] = k
      if (previous[t.key] && previous[t.key] !== k) { changed[t.key] = now; dirty = true }
    })
    threadKinds = next
    if (dirty) stateChangedAt = changed
  }
  Connections { target: root.bridge; function onStateChanged() { root.trackThreadStates() } }
  function stateColor(t) { return T3Palette.stateColor(kind(t)) }
  function open() { windowApplyAttempts=0;panelOpen=true;if(detached){desktop.minimized=false;applyWindowMode.restart();Qt.callLater(function(){if(root.managedToplevel)root.managedToplevel.activate()})} }
  function close() { documentPopup.close();imagePopup.close();diffPopup.close();workingPopup.close();toolPopup.close();gitPopup.close();rewindWorkflow.dismiss();captureState(); panelOpen = false }
  function toggle() { opened ? close() : open() }
  function date(ms) { return ms ? new Date(ms).toLocaleString(Qt.locale(T3I18n.locale), "dd MMM · HH:mm") : "—" }
  function age(ms) {
    if (!ms) return "—"
    var mins = Math.max(0, Math.floor((Date.now() - ms) / 60000))
    return mins < 1 ? T3I18n.tr("now") : mins < 60 ? mins + "m" : mins < 1440 ? Math.floor(mins / 60) + "h" : Math.floor(mins / 1440) + "d"
  }
  function snoozeRemaining(until){var remaining=new Date(until).getTime()-clockNow;if(!isFinite(remaining)||remaining<=0)return T3I18n.tr("now");return remaining<3600000?Math.max(1,Math.ceil(remaining/60000))+"m":remaining<86400000?Math.ceil(remaining/3600000)+"h":Math.ceil(remaining/86400000)+"d"}
  function codeHeaderAt(edit,plain,pos){
    // QTextDocument exposes table-cell boundaries as FDD0/FDD1. Only our
    // generated internal copy anchor identifies a generated code toolbar.
    var previous=plain.lastIndexOf("\ufdd1",pos-1),start=plain.indexOf("\ufdd0",previous+1),end=plain.indexOf("\ufdd1",pos)
    if(start<0||start>pos||end<0)return false
    var label=plain.indexOf(T3I18n.tr("Copy code").toLocaleLowerCase(),start)
    if(label<0||label>=end)return false
    var rect=edit.positionToRectangle(label)
    return String(edit.linkAt(rect.x+2,rect.y+rect.height/2)).indexOf("ot3-code-copy://")===0
  }
  function renderMessage(message,availableWidth){
    if(!message.rendered)return message.text || ""
    var html=message.rendered
    html=html.replace(/<a href="ot3-image-open:\/\/([0-9]+)">([\s\S]*?)<\/a>/g,function(full,index,label){
      var a=(message.markdownImages || [])[Number(index)];if(!a)return label;
      var state=root.bridge.imageStates[root.bridge.imageKey(a)] || {};
      if(!state.url)return full;
      var width=Math.max(1,Math.min(availableWidth || root.chatContentMaxWidth,state.width || 640));
      var height=Math.max(1,Math.round(width*(state.height || 480)/(state.width || 640)));
      return '<a href="ot3-image-open://'+index+'"><img src="'+String(state.url).replace(/&/g,"&amp;").replace(/"/g,"&quot;")+'" width="'+Math.round(width)+'" height="'+height+'" /></a>';
    })
    if(message.streaming||!root.bridge.canOpenTerminal)html=html.replace(new RegExp('<a href="ot3-code-run://[0-9]+">Run in terminal</a> · ',"g"),"")
    html=html.replace(new RegExp('(href="ot3-code-run://[0-9]+">)Run in terminal(</a>)',"g"),function(_,a,b){return a+T3I18n.tr("Run in terminal")+b})
    html=html.replace(new RegExp('(href="ot3-code-copy://[0-9]+">)Copy code(</a>)',"g"),function(_,a,b){return a+T3I18n.tr("Copy code")+b})
    html=MarkdownStyle.render(html,Style.font.body,Style.spaceReal(1),String(root.fg),String(root.dim),String(root.surface),String(root.divider))
    return MarkdownLinks.decorate(html).replace(/<a /g,"<a style=\"color:"+Color.accent+";\" ")
  }
  function shortcutHelp(){
    var shortcuts=[],tr=function(key){return T3I18n.tr(key)}
    if(nativeKeybindingsEnabled)nativeShortcuts.routes.forEach(function(r){shortcuts.push([Native.nativeKeys.formatShortcutLabel(r.shortcut,"Linux"),KeyLabels.command(r.command,tr)])})
    else [["Ctrl+F","Find in conversation"],["Ctrl+J","Open native terminal"],["Ctrl+S","Save draft"]].forEach(function(p){shortcuts.push([p[0],tr(p[1])])})
    var local=[["Ctrl+Shift+F","Search threads"],["Ctrl+K","Chat list"],["Ctrl+L","Write message"],["Meta+B","Chat list"],["Meta+E","Expand / return to chat"],["Meta+PgUp","Switch chat"],["Meta+PgDown","Switch chat"],["Ctrl+Shift+A","Attach files"],["Ctrl+Alt+Left","Resize"],["Ctrl+Alt+Right","Resize"],["Ctrl+Alt+Up","Resize"],["Ctrl+Alt+Down","Resize"]]
    local.filter(function(p){return root.localShortcutAvailable(p[0])}).forEach(function(p){shortcuts.push([p[0].replace("Meta+","Super+"),tr(p[1])])})
    var fixed=[["Tab / Shift+Tab","Navigate controls"],["Super+A / Super+Z / Super+Shift+Z","Edit"],[root.composerExpanded?"Ctrl+Enter":"Enter / Ctrl+Enter","Send"],[root.composerExpanded?"Enter / Shift+Enter":"Shift+Enter","New line"],["Ctrl+Shift+Enter","Send and start new thread"],["↑ / ↓","Prompt history"],["PageUp / PageDown","Read conversation"],["Esc","Return / close"]]
    fixed.forEach(function(p){shortcuts.push([p[0],tr(p[1])])})
    return shortcuts.map(function(p){return T3I18n.tr("{shortcut} · {action}",{shortcut:p[0],action:p[1]})}).join("\n")
  }
  function providerIcon(provider) {
    var p = String(provider || "").toLowerCase()
    return p.indexOf("codex") >= 0 || p.indexOf("openai") >= 0 ? "openai" : p.indexOf("claude") >= 0 ? "claude" : p.indexOf("cursor") >= 0 ? "cursor" : "acp"
  }
  function status(t) {
    return kind(t) === "waiting" ? T3I18n.tr("Waiting") : kind(t) === "working" ? T3I18n.tr("Working") : kind(t) === "failed" ? T3I18n.tr("Failed") : kind(t) === "done" && t.pending ? T3I18n.tr("Done") : ""
  }
  // Rows around a moving one slide to open or close its gap; a leaving row
  // is kept alive while it fades or flies (see rowLeaving).
  component RowRemove: Transition { enabled: root.rowMotion; PauseAnimation { duration: root.rowMoveMs + 60 } }
  component RowDisplaced: Transition {
    enabled: root.rowMotion
    NumberAnimation { properties: "x,y"; duration: root.rowMoveMs * 0.85; easing.type: Easing.OutCubic }
  }
  component RowMove: Transition {
    enabled: root.rowMotion
    NumberAnimation { properties: "x,y"; duration: root.rowMoveMs; easing.type: Easing.InOutCubic }
  }
  Component {
    id: threadDelegate
    T3ThreadRow {
      id: sidebarRow
      required property var row
      required property int index
      readonly property var modelData: row
      thread: modelData; bridge: root.bridge
      width: ListView.view ? ListView.view.width : 0
      projectScoped:!!root.projectFilter&&(Array.isArray(root.projectScope)?root.projectScope.indexOf(modelData.projectKey)>=0:root.projectScope===modelData.projectKey)
      panelVisible: root.opened
      keyboardCurrent: ListView.view ? ListView.view.activeFocus && ListView.view.currentIndex===index : false
      stateColor:root.stateColor(modelData);stateLabel:root.status(modelData)
      sky:root.starfield;effects:root.threadEffects;stateChangedAt:root.stateChangedAt[modelData.key] || 0
      timeLabel:modelData.working ? root.duration(modelData.startedAt) : modelData.snoozed&&modelData.snoozedUntil ? root.snoozeRemaining(modelData.snoozedUntil) : root.age(modelData.settled?modelData.settledSortAt || modelData.updatedAt:modelData.updatedAt)
      providerGlyph:root.providerIcon(modelData.provider)
      onPullRequestsRequested:function(source,reference,multiple){root.bridge.selectThread(source.environmentId,source.id);root.prTarget=Object.assign({},reference,{environmentId:source.environmentId,multiple:multiple,references:Array.prototype.slice.call(source.pullRequestBadge.links)});root.pullRequestsOpen=true}
      onThreadActivated:{root.pullRequestsOpen=false;root.preferencesOpen=false}
      onFilterProjectRequested:function(key){var group=root.projectGroups.find(function(g){return g.members.some(function(p){return p.key===key})});if(group)root.projectFilter=root.projectFilter===group.key?"":group.key}
      onProjectSettingsRequested:function(key){root.openProjectSettings(key)}
      onNewThreadOnBranch:function(source){sidebarActions.newOnBranch(source)}
      onProjectContextDismissed:projectMenu.dismissContext()
      onProjectContextRequested:function(item,px,py){projectMenu.showProject(modelData.projectKey,item,px,py)}
      ListView.onPooled:pooled=true
      ListView.onReused:pooled=false
      ListView.onAdd:root.rowArriving(sidebarRow)
      ListView.onRemove:root.rowLeaving(sidebarRow)
    }
  }
  T3ProjectMenu{id:projectMenu;bridge:root.bridge;scopedSettings:true;onSettingsRequested:function(key){root.openProjectSettings(key)};onGroupingChanged:function(previous,current){if(root.projectFilter===previous||root.projectFilter===projectMenu.project.key)root.projectFilter=current}}
  T3CompactionSettings{id:compactionSettings;bridge:root.bridge}
  FloatingWindow {
    id:desktop
    title:"Omarchy T3 Code"
    visible:root.panelOpen&&root.detached
    color:Color.popups.background
    // An opaque surface lets Hyprland skip drawing and blurring what lies
    // behind the window on every frame (~25% less compositor CPU while it
    // animates). Translucent themes keep the blended surface.
    surfaceFormat.opaque:Color.popups.background.a>=0.999
    implicitWidth:Style.space(root.bridge.viewState.windowWidth || root.panelWidth)
    implicitHeight:Style.space(root.bridge.viewState.windowHeight || root.panelHeight)
    minimumSize:Qt.size(Style.space(760),Style.space(360))
    onClosed:if(root.detached)root.close()
    onWidthChanged:if(visible&&root.detached)saveWindowSize.restart()
    onHeightChanged:if(visible&&root.detached)saveWindowSize.restart()
  }
  Timer{id:saveWindowSize;interval:200;onTriggered:if(desktop.visible&&root.windowMode==="floating")root.bridge.rememberView({windowWidth:desktop.width/Style.space(1),windowHeight:desktop.height/Style.space(1)})}
  Component{id:sidePanelComponent;T3SidePanel{bridge:root.bridge;browserComponent:browserWorkspaceComponent;tab:root.sideTab;renders:root.renderList;renderAttachment:root.renderAttachment;onTabRequested:function(tab){root.sideTab=tab};onRenderRequested:function(a){root.renderAttachment=a};onClosed:root.closeSide();onIntegrationsRequested:{root.preferencesOpen=true;Qt.callLater(function(){if(preferencesLoader.item)preferencesLoader.item.page="integrations"})}}}
  Component{id:browserWorkspaceComponent;T3BrowserWorkspace{bridge:root.bridge;onClosed:root.closeSide();onPreferencesRequested:{root.preferencesOpen=true;Qt.callLater(function(){if(preferencesLoader.item)preferencesLoader.item.page="integrations"})}}}
  PinnedKeyboardPanel {
    id: popup
    open:root.panelOpen&&!root.detached
    suspended:root.detached
    anchorItem: root.anchorItem; bar: root.bar; owner: root
    contentWidth: Math.min(Style.space(root.panelWidth), availableCardWidth - padding * 2)
    contentHeight: Math.min(Style.space(root.panelHeight), availableCardHeight - padding * 2)
    resizable: true
    onResized: function(w,h) { root.resizePanel(w/Style.space(1),h/Style.space(1)) }
    padding: 0
    focusTarget: root.sidebarCollapsed || root.composerExpanded ? (root.bridge.selectedThread ? composerPanel.inputField : null) : searchField
    T3Starfield {
      id:sky;objectName:"t3Starfield"
      parent:root.detached?desktop.contentItem:popup.surfaceItem
      anchors.fill:parent;z:-1
      mode:root.bridge.viewState.starfieldMode || "auto"
      // Any working chat counts, including runs started from another device.
      busy:!!root.bridge.selectedInfo.working||(root.bridge.state.working||0)>0
      paused:!root.opened
      brightness:0.5;meteorRate:0.3
      // The sidebar header sky (meteorRate 0.85) follows this clock.
      meteorReach:effectiveMode==="off"?0:Math.min(1,(sidebarReveal.visible?0.85:effectiveMode==="full"?0.3:0)*meteorScale)
      speedScale:({slow:0.5,fast:1.8})[root.bridge.viewState.starfieldSpeed] || 1
      meteorScale:({none:0,few:0.4,many:2.2})[root.bridge.viewState.starfieldMeteors] ?? 1
      Connections{target:conversation;function onContentYChanged(){sky.follow(conversation.contentY)}}
    }
    T3FpsMeter {
      objectName:"t3FpsMeter"
      parent:root.detached?desktop.contentItem:popup.surfaceItem
      anchors.left:parent.left;anchors.bottom:parent.bottom;anchors.margins:Style.space(8);z:100
      active:root.opened&&root.bridge.viewState.fpsMeter===true
    }
    RowLayout {
      id:panelLayout;objectName:"t3PanelLayout"
      parent:root.detached?desktop.contentItem:popup.surfaceItem
      LayoutMirroring.enabled:T3I18n.rtl;LayoutMirroring.childrenInherit:true
      anchors.fill: parent; spacing: 0
      palette.window:root.themeSurface;palette.windowText:root.fg;palette.base:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05));palette.alternateBase:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08));palette.text:root.fg;palette.button:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08));palette.buttonText:root.fg;palette.highlight:Color.accent;palette.highlightedText:root.themeSurface;palette.placeholderText:root.dim;palette.toolTipBase:root.themeSurface;palette.toolTipText:root.fg;palette.link:Color.accent;palette.light:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16));palette.midlight:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12));palette.mid:Qt.tint(root.themeSurface,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2));palette.dark:Qt.darker(root.themeSurface,1.4);palette.shadow:Qt.rgba(0,0,0,0.5)
      T3ShortcutRouter{id:nativeShortcuts;active:root.nativeKeybindingsEnabled&&root.nativeKeysActive;primary:root.nativePrimaryKeys;scripts:root.nativeScriptKeys;context:root.nativeShortcutContext;localOverrides:root.findOpen?["Ctrl+G","Ctrl+Shift+G"]:[];supports:function(command){return root.supportsNativeShortcut(command)};onTriggered:function(command){root.triggerNativeShortcut(command)}}
      Shortcut { sequence:"Meta+B";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Meta+B");onActivated:root.toggleSidebar() }
      Shortcut { sequence:"Ctrl+F";enabled:!root.nativeKeybindingsEnabled&&(root.opened&&!root.preferencesOpen&&!root.pullRequestsOpen)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+F");onActivated:root.openFind() }
      Shortcut { sequence:"Ctrl+Shift+F";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Shift+F");onActivated:{if(root.composerExpanded)root.expandComposer(false);if(root.sidebarCollapsed)root.toggleSidebar(false);searchField.forceActiveFocus();searchField.selectAll()} }
      Shortcut { sequence:"Ctrl+G";enabled:(root.opened&&root.findOpen)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+G");onActivated:root.navigateFind(1) }
      Shortcut { sequence:"Ctrl+Shift+G";enabled:(root.opened&&root.findOpen)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Shift+G");onActivated:root.navigateFind(-1) }
      Shortcut{sequence:"Ctrl+Shift+J";enabled:!root.nativeKeybindingsEnabled&&root.shortcutActive&&!!root.bridge.selectedThread&&root.localShortcutAvailable("Ctrl+Shift+J");onActivated:root.toggleBrowser()}
      Shortcut{sequence:"Ctrl+R";enabled:!root.nativeKeybindingsEnabled&&root.shortcutActive&&root.browserFocused&&root.localShortcutAvailable("Ctrl+R");onActivated:if(root.browserWorkspaceItem)root.browserWorkspaceItem.refresh()}
      Shortcut{sequence:"Ctrl+J";enabled:!root.nativeKeybindingsEnabled&&(root.shortcutActive&&root.bridge.canOpenTerminal)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+J");onActivated:root.bridge.openTerminal()}
      Shortcut { sequence:"Ctrl+K";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+K");onActivated:root.focusSidebar() }
      Shortcut { sequence:"Ctrl+L";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+L");onActivated:{if(root.browserFocused)root.focusBrowserUrl();else composerPanel.inputField.forceActiveFocus()} }
      Shortcut { sequence:"Meta+E";enabled:(root.opened && !!root.bridge.selectedThread)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Meta+E");onActivated:root.expandComposer() }
      Shortcut { sequence:"Meta+Return";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Meta+Return");onActivated:root.submitCurrent() }
      Shortcut { sequence:"Ctrl+Shift+A";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Shift+A");onActivated:composerPanel.attachFiles() }
      Shortcut { sequence:"Ctrl+S";enabled:!root.nativeKeybindingsEnabled&&(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+S");onActivated:root.bridge.flushDrafts() }
      Shortcut { sequence:"Meta+PgDown";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Meta+PgDown");onActivated:root.navigateThread(1) }
      Shortcut { sequence:"Meta+PgUp";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Meta+PgUp");onActivated:root.navigateThread(-1) }
      Shortcut { sequence:"Ctrl+Alt+Right";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Alt+Right");onActivated:root.resizePanel(root.panelWidth+40,root.panelHeight) }
      Shortcut { sequence:"Ctrl+Alt+Left";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Alt+Left");onActivated:root.resizePanel(root.panelWidth-40,root.panelHeight) }
      Shortcut { sequence:"Ctrl+Alt+Up";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Alt+Up");onActivated:root.resizePanel(root.panelWidth,root.panelHeight-30) }
      Shortcut { sequence:"Ctrl+Alt+Down";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("Ctrl+Alt+Down");onActivated:root.resizePanel(root.panelWidth,root.panelHeight+30) }
      Shortcut { sequence:"F1";enabled:(root.opened)&&!(root.bridge.keybindingCaptureActive || false)&&root.localShortcutAvailable("F1");onActivated:keyHelp.open() }
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
        id:diffPopup;objectName:"t3DiffPopup"
        property string initialPath:""
        onClosed:root.bridge.dismissDiff()
        onOpened:diffReview.forceActiveFocus()
        focus:true;modal:true
        x:Style.space(24);y:Style.space(24)
        width:Math.max(100,parent.width-Style.space(48));height:Math.max(100,parent.height-Style.space(48));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        T3DiffReview {
          id:diffReview;objectName:"t3DiffContents";anchors.fill:parent;rendererBridge:root.bridge
          diffLayout:root.bridge.diffPreferences.layout;wordWrap:root.bridge.diffPreferences.wordWrap
          view:root.bridge.diffView;summary:root.bridge.diffView.summary || {};initialPath:diffPopup.initialPath
          ignoreWhitespace:root.bridge.diffView.ignoreWhitespace!==false
          onClosed:diffPopup.close()
          onRetryRequested:{if(selectedPath)diffPopup.initialPath=selectedPath;root.bridge.requestDiff(summary,ignoreWhitespace)}
          onWhitespaceChanged:function(value){if(selectedPath)diffPopup.initialPath=selectedPath;root.bridge.requestDiff(summary,value)}
          onFileRequested:function(path){diffPopup.initialPath=path;root.bridge.requestDiff(summary,ignoreWhitespace)}
        }
      }
      Controls.Popup {
        id:workingPopup;objectName:"t3WorkingReviewPopup";focus:true;modal:true
        x:Style.space(24);y:Style.space(24);width:Math.max(100,parent.width-Style.space(48));height:Math.max(100,parent.height-Style.space(48));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        onClosed:root.bridge.dismissWorkingReview()
        onOpened:workingContents.forceActiveFocus()
        T3WorkingReview {
          id:workingContents;anchors.fill:parent;rendererBridge:root.bridge;view:root.bridge.workingReview;refs:root.bridge.reviewRefs
          onClosed:workingPopup.close()
          onRequested:function(kind,base,whitespace,path){root.bridge.requestWorkingReview(kind,base,whitespace,path)}
          onRefsRequested:function(query,kind,cursor){root.bridge.requestReviewRefs(query,kind,cursor)}
          onRefsDismissed:root.bridge.dismissReviewRefs()
        }
      }
      T3CheckpointRevert {
        id:rewindWorkflow;bridge:root.bridge;popupHost:parent;closeAfterConfirmation:true
        environment:root.bridge.selectedEnvironment;threadId:root.bridge.selectedThread
        canOperate:root.bridge.catalog.canWrite===true;connected:root.bridge.rewindConnected || false
        supportsRollback:!!root.bridge.rollbackProvider&&root.bridge.rollbackProvider.supportsConversationRollback!==false
        working:!!root.bridge.selectedInfo.working;sendBusy:!!root.bridge.draft.busy||!!root.bridge.draft.attempt||!!root.bridge.draft.queueEdit
        connecting:!!root.bridge.loadingMessages;attachmentsPending:(root.bridge.draft.attachments || []).some(function(f){return f.uploading})
        parentPending:root.bridge.rewindPending || false;canRestoreFiles:!!root.bridge.selectedInfo.worktreePath
        onRequested:function(request){root.bridge.requestRewind(request)}
        onRefreshRequested:function(request){root.bridge.refreshRewind(request)}
        onReviewRequested:function(messageId){root.bridge.requestRewindPreview(messageId)}
        onRecoverRequested:function(request){root.bridge.recoverRewind(request)}
        onReconcileRequested:function(request){root.bridge.reconcileRewind(request)}
      }
      Connections{target:root.bridge;ignoreUnknownSignals:true
        function onRewindReady(reference,canRestoreFiles){rewindWorkflow.canRestoreFiles=canRestoreFiles;rewindWorkflow.begin(reference)}
        function onRewindReceipt(receipt){rewindWorkflow.accept(receipt)}
        function onRewindResume(request,state){rewindWorkflow.resume(request,state)}
        function onRewindRestored(key){if(root.opened&&key===root.bridge.selectedKey&&!root.preferencesOpen&&!root.pullRequestsOpen)composerPanel.inputField.forceActiveFocus()}
      }
      Controls.Popup {
        id:gitPopup;objectName:"t3GitPopup";focus:true;modal:true
        x:Math.max(Style.space(20),parent.width-width-Style.space(20));y:Style.space(65)
        width:Math.min(Style.space(700),parent.width-Style.space(40));height:Math.max(100,parent.height-Style.space(90));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        onClosed:if(root.bridge.dismissGit)root.bridge.dismissGit()
        T3GitControls {
          anchors.fill:parent;scope:(root.bridge.gitView || {}).scope || {};view:root.bridge.gitView || {loading:false,error:"",data:null}
          canSourceControlWrite:!!(snapshot && snapshot.permissions && snapshot.permissions.sourceControlWrite)
          canReviewChanges:!!(snapshot && snapshot.permissions && snapshot.permissions.filesystemRead)
          canTerminalWrite:!!(snapshot && snapshot.permissions && snapshot.permissions.terminalWrite)
          busy:root.bridge.gitBusy || false
          operationState:((root.bridge.gitView || {}).operation || {}).state || ""
          operationError:(root.bridge.gitView || {}).operationError || ((root.bridge.gitView || {}).operation || {}).error || ""
          operation:(root.bridge.gitView || {}).operation || {}
          onClosed:gitPopup.close()
          onRefreshRequested:root.bridge.requestGit()
          onChangesRequested:function(path){gitPopup.close();root.bridge.requestWorkingReview("working-tree","",root.bridge.diffPreferences.ignoreWhitespace,path);workingPopup.open()}
          onActionRequested:function(intent){root.bridge.requestGitAction(intent)}
          onTerminalRequested:root.bridge.openGitTerminal()
          onReconcileRequested:root.bridge.reconcileGitOperation()
          onNativeRequested:root.bridge.openThread(root.bridge.selectedEnvironment,root.bridge.selectedThread)
          onPullRequestRequested:function(pr){var entry=snapshot.pullRequestReference;if(!entry)return;root.prTarget=entry;gitPopup.close();root.pullRequestsOpen=true}
        }
      }
      Controls.Popup {
        id:toolPopup;objectName:"t3ToolPopup";focus:true;modal:true
        x:Style.space(24);y:Style.space(24);width:Math.max(100,parent.width-Style.space(48));height:Math.max(100,parent.height-Style.space(48));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        onClosed:toolInspector.dismiss(false)
        onOpened:toolInspector.forceActiveFocus()
        T3ToolInspector {
          id:toolInspector;anchors.fill:parent;bridge:root.bridge;environment:root.bridge.selectedEnvironment;threadId:root.bridge.selectedThread
          onRequested:function(token,scope){root.bridge.send(Object.assign({op:"tool-inspector",thread:scope.threadId,requestId:token},scope))}
          onClosed:toolPopup.close()
        }
        Connections {target:root.bridge;function onToolInspectorFinished(requestId,result,error){toolInspector.accept(requestId,result,error)}}
      }
      Controls.Popup {
        id:documentPopup;objectName:"t3EmbeddedDocumentPopup";focus:true;modal:true
        x:Style.space(18);y:Style.space(18);width:Math.max(100,parent.width-Style.space(36));height:Math.max(100,parent.height-Style.space(36));padding:Style.space(14)
        background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.divider}
        onClosed:root.bridge.closeDocument()
        contentItem:Loader{active:documentPopup.visible;sourceComponent:Component{T3DocumentView{bridge:root.bridge;onClosed:documentPopup.close()}}}
      }
      Connections{target:root.bridge;function onDocumentOpenChanged(){if(root.opened&&root.bridge.documentOpen.requestId&&!documentPopup.visible)documentPopup.open();else if(!root.bridge.documentOpen.requestId&&documentPopup.visible)documentPopup.close()}function onDocumentOpenedExternally(){if(!root.detached)root.close()}}
      Controls.Popup {
        id:keyHelp; focus:true;x:Math.max(0,(parent.width-width)/2);y:Style.space(60);width:Style.space(420);padding:Style.space(18)
        height:Math.min(parent.height-Style.space(80),helpText.implicitHeight+2*padding)
        background:Rectangle {color:Color.popups.background;border.color:root.divider;border.width:1;radius:Style.space(10)}
        contentItem:Controls.ScrollView{id:helpScroll;clip:true;contentWidth:availableWidth
          Text{id:helpText;width:helpScroll.availableWidth;text:root.shortcutHelp();color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
        }
      }
      Keys.onEscapePressed: {if(root.preferencesOpen)root.preferencesOpen=false;else if(root.sideShown){root.closeSide()}else if(root.findOpen){root.findOpen=false;root.findQuery=""}else if(root.composerExpanded)root.expandComposer();else root.close()}
      T3WorkspaceConfirmation {popupHost:panelLayout;bridge:root.bridge}
      T3SidebarActions { id: sidebarActions; popupHost:panelLayout; bridge: root.bridge; projects: root.bridge.state.projects || []; preferredProjectKey:Array.isArray(root.projectScope)?root.projectScope.length===1?root.projectScope[0]:"":root.projectScope }
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
      // Collapsing animates with the transition chosen in Preferences; the
      // preferences view still swaps instantly.
      T3Reveal {
        id: sidebarReveal; objectName: "sidebarReveal"
        horizontal: true; Layout.fillHeight: true
        size: Math.min(Style.space(root.sidebarWidth),Math.max(Style.space(260),panelLayout.width-Style.space(420)))
        shown: !root.sidebarCollapsed && !root.composerExpanded
        visible: present && !root.preferencesOpen
        Rectangle {
          id: sidebar
          anchors.fill: parent
          color: Qt.rgba(0, 0, 0, 0.09)
          // Same sky as the panel, at night: darker base, brighter stars and most
          // of the shooting stars while working. Solid behind the header, then it
          // dissolves behind the search row into the panel sky.
          T3Starfield {
            width: parent.width; height: searchRow.y + searchRow.height + Style.space(12)
            leader: sky; header: true
            origin: Qt.vector2d(sidebar.x, 0)
            base: Qt.darker(Color.popups.background, 2.4)
            brightness: 1.25; nebula: 0.55; meteorRate: 0.85
            fade: height > 0 ? 1 - brand.height * 0.85 / height : 0
          }
          // Rows flying between sections are drawn here, above every list.
          Item { id: flightLayer; objectName: "sidebarFlightLayer"; anchors.fill: parent; z: 10 }
          ColumnLayout {
            anchors.fill: parent; spacing: 0
            Rectangle {
              id: brand
              Layout.fillWidth: true; Layout.preferredHeight: Style.space(62)
              clip: true
              color: "transparent"
              RowLayout {
                anchors.fill: parent; anchors.leftMargin: Style.space(18); anchors.rightMargin: Style.space(18); anchors.topMargin: Style.space(11); anchors.bottomMargin: Style.space(11); spacing: Style.space(10)
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
                T3IconButton{objectName:"t3PullRequestsToggle";name:"git-pull-request";tooltipText:T3I18n.tr("Pull requests");tint:root.pullRequestsOpen?Color.accent:root.dim;onClicked:{root.preferencesOpen=false;root.pullRequestsOpen=!root.pullRequestsOpen}}
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
              id: searchRow
              Layout.fillWidth: true; Layout.margins: Style.space(12); spacing: Style.space(7); Layout.topMargin: Style.space(4)
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
                Keys.onEscapePressed: if(root.bridge.query)root.bridge.search("");else root.close()
                Keys.onDownPressed: root.focusSidebar()
              }
              T3IconButton{objectName:"clearThreadSearch";name:"x";implicitWidth:Style.space(20);implicitHeight:implicitWidth;visible:!!root.bridge.query;tooltipText:T3I18n.tr("Clear search");onClicked:{root.bridge.search("");searchField.forceActiveFocus()}}
              T3IconButton {
                id: filterButton
                name: "list-filter"; tooltipText: T3I18n.tr("Filter by state and project")
                tint: root.statusFilter !== "all" || root.projectFilter ? Color.accent : root.dim
                onClicked: filterMenu.visible ? filterMenu.close() : filterMenu.open()
              }
              T3IconButton { objectName:"newProjectButton"; name: "folder-plus"; tooltipText: T3I18n.tr("Add project"); onClicked: sidebarActions.addProject() }
              T3IconButton { objectName:"newChatButton"; name: "square-pen"; tooltipText: T3I18n.tr("New chat"); onClicked: sidebarActions.newThread() }
            }
            RowLayout {
              Layout.fillWidth: true; Layout.leftMargin: Style.space(12); Layout.rightMargin: Style.space(12)
              visible: !!root.bridge.managementError
              Text { Layout.fillWidth: true; text: T3I18n.error(root.bridge.managementError); color: Color.urgent; font.pixelSize: Style.font.caption; elide: Text.ElideRight }
              T3IconButton { name: "refresh-cw"; visible: !!root.bridge.managementRequest; tooltipText: T3I18n.tr("Retry action"); onClicked: root.bridge.manage({}) }
            }
            ListView {
              id: threads
              Keys.onUpPressed:root.moveSidebarFocus(threads,-1)
              Keys.onDownPressed:root.moveSidebarFocus(threads,1)
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
              model: activeRowModel
              remove: RowRemove {}
              displaced: RowDisplaced {}
              move: RowMove {}
              onContentYChanged: if (!root.restoringSidebar) root.capturePosition(threads,root.activeRows,"@sidebar")
              spacing: 0
              Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
              delegate: threadDelegate
              Text {
                anchors.centerIn: parent; width: parent.width - Style.space(28)
                visible: threads.count === 0
                text: root.bridge.stale ? T3I18n.tr("T3 is offline") : T3I18n.tr("No threads here")
                color: root.dim; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap
              }
            }
            T3ShelfResizer{objectName:"workingShelfResizer";label:"Working threads height";visible:workingReveal.shown&&!workingReveal.moving;value:root.workingShelfHeight;defaultValue:220;onResized:function(h){root.resizeShelf("workingShelfHeight",h)}}
            // Section headers come and go with the Show and hide transition.
            T3Reveal {
              objectName:"workingShelfHeader";shown:root.workingRows.length>0
              Layout.fillWidth:true;Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              Rectangle {
                width:parent.width;implicitHeight:Style.space(32);height:implicitHeight
                id:workingToggle;objectName:"workingShelfToggle"
                border.width:activeFocus?1:0;border.color:Color.accent;radius:Style.space(5)
                color:"transparent";activeFocusOnTab:true
                Accessible.role:Accessible.Button;Accessible.name:T3I18n.tr("Working ({count})",{count:root.workingRows.length})
                Keys.onReturnPressed:root.workingExpanded=!root.workingExpanded
                Keys.onSpacePressed:root.workingExpanded=!root.workingExpanded
                Keys.onUpPressed:root.moveSidebarFocus(workingToggle,-1)
                Keys.onDownPressed:root.moveSidebarFocus(workingToggle,1)
                Keys.onLeftPressed:root.workingExpanded=false
                Keys.onRightPressed:root.workingExpanded=true
                RowLayout{anchors.fill:parent;anchors.margins:Style.space(5);Text{text:parent.parent.Accessible.name;color:T3Palette.working;font.pixelSize:Style.font.caption}Rectangle{objectName:"workingShelfDivider";Layout.fillWidth:true;Layout.minimumWidth:Style.space(8);height:1;color:root.divider}T3Icon{name:root.workingExpanded?"chevron-down":"chevron-up";tint:root.dim;implicitWidth:Style.space(12)}}
                MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:root.workingExpanded=!root.workingExpanded}
              }
            }
            T3Reveal {
              id:workingReveal;shown:root.visibleWorkingRows.length>0
              size:Math.min(workingThreads.contentHeight,Style.space(root.workingShelfHeight))
              Layout.fillWidth:true;Layout.fillHeight:root.activeRows.length===0&&shown&&!moving
              Layout.maximumHeight:root.activeRows.length>0?Style.space(root.workingShelfHeight):Infinity
              Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              ListView {
                id:workingThreads;objectName:"workingThreadList"
                anchors.fill:parent
                Keys.onUpPressed:root.moveSidebarFocus(workingThreads,-1)
                Keys.onDownPressed:root.moveSidebarFocus(workingThreads,1)
                T3Wheel{flickable:workingThreads}
                activeFocusOnTab:true;keyNavigationEnabled:true;clip:true;reuseItems:true;cacheBuffer:0;spacing:0
                highlightFollowsCurrentItem:activeFocus
                highlight:Rectangle{color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.09);visible:workingThreads.activeFocus;radius:Style.space(8)}
                Keys.onReturnPressed:{var t=root.visibleWorkingRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
                Keys.onEnterPressed:{var t=root.visibleWorkingRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
                model:workingRowModel;delegate:threadDelegate
                remove:RowRemove{}
                displaced:RowDisplaced{}
                move:RowMove{}
                onContentYChanged:if(!root.restoringWorking)root.capturePosition(workingThreads,root.visibleWorkingRows,"@working")
                Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
              }
            }
            T3ShelfResizer{objectName:"snoozedShelfResizer";label:"Snoozed threads height";visible:snoozedReveal.shown&&!snoozedReveal.moving;value:root.snoozedShelfHeight;defaultValue:164;onResized:function(h){root.resizeShelf("snoozedShelfHeight",h)}}
            T3Reveal {
              objectName:"snoozedShelfHeader";shown:root.snoozedRows.length>0
              Layout.fillWidth:true;Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              Rectangle {
                width:parent.width;implicitHeight:Style.space(32);height:implicitHeight
                id:snoozedToggle;objectName:"snoozedShelfToggle"
                border.width:activeFocus?1:0;border.color:Color.accent;radius:Style.space(5)
                color:"transparent";activeFocusOnTab:true
                Accessible.role:Accessible.Button;Accessible.name:T3I18n.tr("Snoozed ({count})",{count:root.snoozedRows.length})
                Keys.onReturnPressed:root.snoozedExpanded=!root.snoozedExpanded
                Keys.onSpacePressed:root.snoozedExpanded=!root.snoozedExpanded
                Keys.onUpPressed:root.moveSidebarFocus(snoozedToggle,-1)
                Keys.onDownPressed:root.moveSidebarFocus(snoozedToggle,1)
                Keys.onLeftPressed:root.snoozedExpanded=false
                Keys.onRightPressed:root.snoozedExpanded=true
                RowLayout{anchors.fill:parent;anchors.margins:Style.space(5);Text{text:parent.parent.Accessible.name;color:root.dim;font.pixelSize:Style.font.caption}Rectangle{objectName:"snoozedShelfDivider";Layout.fillWidth:true;Layout.minimumWidth:Style.space(8);height:1;color:root.divider}T3Icon{name:root.snoozedExpanded?"chevron-down":"chevron-up";tint:root.dim;implicitWidth:Style.space(12)}}
                MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:root.snoozedExpanded=!root.snoozedExpanded}
              }
            }
            T3Reveal {
              id:snoozedReveal;shown:root.visibleSnoozedRows.length>0
              size:Math.min(snoozedThreads.contentHeight,Style.space(root.snoozedShelfHeight))
              Layout.fillWidth:true;Layout.maximumHeight:Style.space(root.snoozedShelfHeight)
              Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              ListView {
                id:snoozedThreads;objectName:"snoozedThreadList"
                anchors.fill:parent
                Keys.onUpPressed:root.moveSidebarFocus(snoozedThreads,-1)
                Keys.onDownPressed:root.moveSidebarFocus(snoozedThreads,1)
                T3Wheel{flickable:snoozedThreads}
                activeFocusOnTab:true;keyNavigationEnabled:true;clip:true;reuseItems:true;cacheBuffer:0;spacing:0
                highlightFollowsCurrentItem:activeFocus
                Keys.onReturnPressed:{var t=root.visibleSnoozedRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
                Keys.onEnterPressed:{var t=root.visibleSnoozedRows[currentIndex];if(t)root.bridge.selectThread(t.environmentId,t.id)}
                model:snoozedRowModel;delegate:threadDelegate
                remove:RowRemove{}
                displaced:RowDisplaced{}
                move:RowMove{}
                onContentYChanged:if(!root.restoringSnoozed)root.capturePosition(snoozedThreads,root.visibleSnoozedRows,"@snoozed")
                Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
              }
            }
            T3ShelfResizer{objectName:"settledShelfResizer";label:"Settled threads height";visible:settledReveal.shown&&!settledReveal.moving;value:root.settledShelfHeight;defaultValue:220;onResized:function(h){root.resizeShelf("settledShelfHeight",h)}}
            T3Reveal {
              objectName:"settledShelfHeader";shown:root.settledRows.length>0
              Layout.fillWidth:true;Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              Rectangle {
                width:parent.width;implicitHeight:Style.space(32);height:implicitHeight
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                id:settledToggle;objectName:"settledShelfToggle"
                border.width:activeFocus?1:0;border.color:Color.accent;radius:Style.space(5)
                Accessible.name: T3I18n.tr("Settled ({count})",{count:root.settledRows.length})
                Keys.onReturnPressed: root.settledExpanded = !root.settledExpanded
                Keys.onSpacePressed: root.settledExpanded = !root.settledExpanded
                Keys.onUpPressed:root.moveSidebarFocus(settledToggle,-1)
                Keys.onDownPressed:root.moveSidebarFocus(settledToggle,1)
                Keys.onLeftPressed:root.settledExpanded=false
                Keys.onRightPressed:root.settledExpanded=true
                color: "transparent"
                RowLayout {
                  anchors.fill: parent; anchors.leftMargin: Style.space(5); anchors.rightMargin: Style.space(5)
                  Text { text:T3I18n.tr("Settled ({count})",{count:root.settledRows.length}); color: root.dim; font.pixelSize: Style.font.caption }
                  Rectangle {objectName:"settledShelfDivider";Layout.fillWidth:true;Layout.minimumWidth:Style.space(8);height:1;color:root.divider}
                  T3Icon { name: root.settledShown ? "chevron-down" : "chevron-up"; tint: root.dim; implicitWidth: Style.space(12) }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.settledExpanded = !root.settledExpanded }
              }
            }
            T3Reveal {
              id:settledReveal;shown:root.visibleSettledRows.length>0
              size:Math.min(settledThreads.contentHeight,Style.space(root.settledShelfHeight))
              Layout.fillWidth:true;Layout.fillHeight:root.activeRows.length===0&&shown&&!moving
              Layout.maximumHeight:root.activeRows.length>0?Style.space(root.settledShelfHeight):Infinity
              Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(8)
              ListView {
                id: settledThreads;objectName:"settledThreadList"
                anchors.fill:parent
                Keys.onUpPressed:root.moveSidebarFocus(settledThreads,-1)
                Keys.onDownPressed:root.moveSidebarFocus(settledThreads,1)
                T3Wheel {flickable:settledThreads}
                activeFocusOnTab: true
                keyNavigationEnabled: true
                highlightFollowsCurrentItem: activeFocus
                highlight: Rectangle {color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.09);visible:settledThreads.activeFocus;radius:Style.space(8)}
                Keys.onReturnPressed: {var t=root.visibleSettledRows[currentIndex];if(t){root.bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
                Keys.onEnterPressed: {var t=root.visibleSettledRows[currentIndex];if(t){root.bridge.selectThread(t.environmentId,t.id);composerPanel.inputField.forceActiveFocus()}}
                clip: true; reuseItems: true; cacheBuffer: 0; spacing: 0
                model: settledRowModel; delegate: threadDelegate
                remove: RowRemove {}
                displaced: RowDisplaced {}
                move: RowMove {}
                onContentYChanged: if (!root.restoringSettled) root.capturePosition(settledThreads,root.visibleSettledRows,"@settled")
                Controls.ScrollBar.vertical: Controls.ScrollBar { policy: Controls.ScrollBar.AsNeeded }
              }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: root.divider }
            RowLayout {
              objectName:"t3SidebarFooter"
              Layout.fillWidth:true;Layout.margins:Style.space(10);spacing:Style.space(3)
              T3IconButton{objectName:"openPreferences";name:"settings";tooltipText:T3I18n.tr("Preferences");onClicked:root.preferencesOpen=!root.preferencesOpen}
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
      }
      Item {
        id:sidebarResizer
        Layout.fillHeight:true;Layout.preferredWidth:Style.space(6)
        visible:!root.preferencesOpen && sidebarReveal.shown && !sidebarReveal.moving
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
        id:mainChatArea;objectName:"t3MainChatArea"
        Layout.fillWidth: true; Layout.fillHeight: true
        color: "transparent"
        Loader{id:overlaySideLoader;objectName:"t3BrowserSheet";anchors.fill:parent;z:20;active:root.sideShown&&!root.browserInline;sourceComponent:sidePanelComponent}
        Loader{objectName:"t3PullRequestsView";anchors.fill:parent;active:root.pullRequestsOpen&&root.opened&&!root.preferencesOpen;sourceComponent:Component{T3PullRequests{bridge:root.bridge;initialEntry:root.prTarget;onClosed:root.pullRequestsOpen=false}}}
        Loader{id:preferencesLoader;objectName:"t3PreferencesView";anchors.fill:parent;active:root.preferencesOpen&&root.opened;sourceComponent:Component{T3Preferences{bridge:root.bridge;initialProjectKey:root.preferencesProjectKey;shortcuts:root.shortcutHelp();onUsageRequested:function(value){root.usageCollapsed=!value};onGroupingRequested:function(value){root.setSidebarGrouped(value)};onClosed:root.preferencesOpen=false}}}
        ColumnLayout {
          visible:!root.preferencesOpen&&!root.pullRequestsOpen
          anchors.fill: parent; spacing: 0
          RowLayout {
            objectName:"t3ChatHeader"
            Layout.fillWidth: true; Layout.preferredHeight: Style.space(58)
            Layout.leftMargin: Style.space(24); Layout.rightMargin: Style.space(14)
            spacing: Style.space(10)
            T3IconButton { visible: root.sidebarCollapsed; name: "panel-left"; tooltipText: T3I18n.tr("Open sidebar"); onClicked: root.toggleSidebar() }
            Text { text: root.bridge.selectedProject || "T3 Code"; color: root.dim; elide: Text.ElideRight; Layout.maximumWidth: Style.space(180); font.pixelSize: Style.font.body; textFormat: Text.PlainText }
            Text { text: "/"; color: root.dim; visible: !!root.bridge.selectedThread }
            Text { Layout.fillWidth: true; text: root.bridge.selectedTitle || T3I18n.tr("Your workspace"); color: root.fg; font.pixelSize: Style.font.body; elide: Text.ElideRight; textFormat: Text.PlainText; MouseArea{anchors.fill:parent;enabled:root.detached;onPressed:desktop.startSystemMove();onDoubleClicked:desktop.maximized=!desktop.maximized} }
            T3IconButton {objectName:"t3OpenSidePanel";name:"panel-right";enabled:!!root.bridge.selectedThread;tint:root.sidePanelOpen?Color.accent:root.dim;tooltipText:T3I18n.tr("Side panel");onClicked:root.toggleSide()}
            T3IconButton {objectName:"t3OpenGit";name:"git-branch";enabled:root.bridge.canOpenGit || false;tooltipText:T3I18n.tr("Git & scripts");onClicked:{root.bridge.requestGit();gitPopup.open()}}
            T3IconButton {objectName:"t3OpenTerminal";name:"terminal";enabled:root.bridge.canOpenTerminal&&!root.bridge.terminalOperation.busy;tooltipText:T3I18n.tr("Open native terminal")+" · Ctrl+J"+(root.bridge.terminalOperation.error?" · "+T3I18n.error(root.bridge.terminalOperation.error):"");onClicked:root.bridge.openTerminal()}
            T3IconButton {objectName:"toggleWindowMode";name:"pin";glyph:root.windowMode==="tiled"?"":"\uf2d0";tint:root.detached?Color.accent:root.fg;tooltipText:T3I18n.tr(root.windowMode==="panel"?"Floating window":root.windowMode==="floating"?"Tiled window":"Return to panel")+(root.bridge.windowLayoutError?" · "+T3I18n.error(root.bridge.windowLayoutError):"");onClicked:root.cycleWindowMode()}
            T3IconButton { name: "external-link"; enabled: !!root.bridge.selectedThread; tooltipText: T3I18n.tr("Open in T3"); onClicked: root.bridge.openThread(root.bridge.selectedEnvironment, root.bridge.selectedThread) }
            T3IconButton{name:"shrink";tooltipText:T3I18n.tr("Automatic compaction");onClicked:compactionSettings.open()}
            T3IconButton{objectName:"t3FindInChat";name:"search";tooltipText:T3I18n.tr("Find in conversation");enabled:!!root.bridge.selectedThread;onClicked:root.openFind()}
            Controls.Button {text:"?";implicitWidth:Style.space(28);Accessible.name:T3I18n.tr("Keyboard shortcuts");onClicked:keyHelp.open()}
            T3IconButton {objectName:"t3CloseWindow";name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
          }
          RowLayout{
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(14)
            visible:(root.bridge.state.environments || []).some(function(e){return e.id==="local"&&e.state!=="ready"}) || !!root.bridge.backendStart.error
            Text{Layout.fillWidth:true;text:root.bridge.backendStart.error?T3I18n.error(root.bridge.backendStart.error):T3I18n.tr("Local backend is offline");color:root.bridge.backendStart.error?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
          }
          Rectangle { Layout.fillWidth: true; height: 1; color: root.divider }
          T3Reveal {
            id:findReveal;shown:root.findOpen;Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(14);Layout.topMargin:Style.space(6)*extent;Layout.bottomMargin:Style.space(6)*extent
          RowLayout {
            anchors.fill:parent
            T3TextField {
              id:chatFindInput;objectName:"t3ChatFindInput";Layout.fillWidth:true;text:root.findQuery;placeholderText:T3I18n.tr("Find in conversation")
              onTextEdited:root.findQuery=text
              Keys.onReturnPressed:function(event){root.navigateFind(event.modifiers&Qt.ShiftModifier?-1:1);event.accepted=true}
              Keys.onEscapePressed:function(event){root.findOpen=false;root.findQuery="";event.accepted=true}
            }
            Text{text:root.findMatches.length?((root.findIndex+1)+"/"+root.findMatches.length):T3I18n.tr("No matches");color:root.dim;font.pixelSize:Style.font.caption}
            T3IconButton{objectName:"t3ChatFindPrevious";name:"chevron-up";tooltipText:T3I18n.tr("Previous match");enabled:!!root.findMatches.length;onClicked:root.navigateFind(-1)}
            T3IconButton{objectName:"t3ChatFindNext";name:"chevron-down";tooltipText:T3I18n.tr("Next match");enabled:!!root.findMatches.length;onClicked:root.navigateFind(1)}
            T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:{root.findOpen=false;root.findQuery=""}}
          }
          }
          T3Reveal {
            id:usageReveal;objectName:"t3HeaderUsageReveal";shown:!root.usageCollapsed
            Layout.fillWidth:true
            Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24)
            Layout.topMargin:Style.space(4)*extent
            Layout.bottomMargin:Style.space(4)*extent
          QuotaStrip {
            id:usage
            objectName:"t3HeaderUsage"
            anchors.left:parent.left;anchors.right:parent.right
            providers:root.bridge.providers
            notices:root.bridge.quotaNotices || []
            external:root.bridge.quotaExternal || []
            onRefreshRequested:root.bridge.refreshQuotas()
            // The reveal does the collapsing so the strip keeps its height.
            showHeader:false;collapsed:false
          }
          }
          Item {
            Layout.fillWidth: true; Layout.fillHeight: true
            visible: !root.composerExpanded
            T3TimelineMinimap {
              anchors.fill:parent;z:3
              rows:root.bridge.transcript;view:conversation;threadKey:root.bridge.selectedKey
              active:root.opened && !root.preferencesOpen && !root.pullRequestsOpen && !root.composerExpanded
              onSelected:function(entry){root.jumpTimelineItem(entry)}
              onWheelRequested:function(angle,pixels,touchpad){conversation.wheelScroll(angle,pixels,touchpad)}
            }
            ListView {
              id: conversation
              header:Column {
                width:conversation.width
                visible:!!root.bridge.hasEarlier || !!(root.bridge.historyOperation || {}).error
                height:visible?implicitHeight:0
                Controls.Button {
                  objectName:"t3LoadEarlier";anchors.horizontalCenter:parent.horizontalCenter
                  text:(root.bridge.historyOperation || {}).busy?T3I18n.tr("Loading…"):T3I18n.tr("Load earlier messages")
                  enabled:!(root.bridge.historyOperation || {}).busy
                  onClicked:{conversation.stopWheel();root.detachConversation();root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey);var p=root.bridge.sessionPositions[root.bridge.selectedKey];if(p)root.bridge.rememberPosition(root.bridge.selectedKey,Object.assign({},p,{atEnd:false}));root.bridge.loadEarlier()}
                }
                Text{width:parent.width;visible:!!text;text:T3I18n.error((root.bridge.historyOperation || {}).error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
              }
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
              anchors.bottomMargin:-composerStack.height
              footer:Item{width:conversation.width;height:composerStack.height+Style.space(26)}
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
              property bool wheelCoasting:false
              property real wheelCoastDelta:0
              property real wheelTarget:0
              property var wheelMotionState:({})
              function stopWheel(){conversationCoastStart.stop();conversationWheel.stop();wheelCoasting=false;wheelMotionState=({})}
              function wheelScroll(angle,pixel,touchpad){
                var delta=pixel!==0?pixel:angle*1.8
                if(delta===0)return
                var local=!(touchpad||pixel!==0?root.bridge.scrollPolicy.touchpadNative:root.bridge.scrollPolicy.mouseNative)
                var step=Motion.plan(wheelMotionState,angle,pixel,touchpad||pixel!==0,!local,Date.now())
                var from=!wheelCoasting&&step.animate&&conversationWheel.running&&step.continueTarget?wheelTarget:contentY
                conversationWheel.stop();root.detachConversation()
                wheelCoasting=false;wheelMotionState=step.state;delta=step.delta;wheelCoastDelta=step.coast
                if(wheelCoastDelta!==0)conversationCoastStart.restart();else conversationCoastStart.stop()
                wheelTarget=Math.max(minY(),Math.min(maxY(),from-delta))
                root.followingTail=wheelTarget>=maxY()-1
                if(!step.animate){contentY=wheelTarget;root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}else {conversationWheel.duration=Motion.duration(wheelTarget-contentY);conversationWheel.start()}
              }
              WheelHandler {
                objectName:"t3ConversationWheel"
                target:null
                acceptedDevices:PointerDevice.Mouse|PointerDevice.TouchPad
                onWheel:function(event){if(event.angleDelta.y===0&&event.pixelDelta.y===0){event.accepted=false;return}event.accepted=true;conversation.wheelScroll(event.angleDelta.y,event.pixelDelta.y,event.device.type===PointerDevice.TouchPad)}
              }
              Timer{id:conversationCoastStart;interval:80;onTriggered:{
                conversation.wheelCoasting=true;conversationWheel.stop()
                conversation.wheelTarget=Math.max(conversation.minY(),Math.min(conversation.maxY(),conversation.wheelTarget-conversation.wheelCoastDelta))
                root.followingTail=conversation.wheelTarget>=conversation.maxY()-1
                conversationWheel.duration=Motion.glide.coasting;conversationWheel.start()
              }}
              NumberAnimation {
                id:conversationWheel;target:conversation;property:"contentY";to:conversation.wheelTarget
                duration:Motion.glide.coasting;easing.type:Easing.OutCubic
                onFinished:{if(root.followingTail)root.settleSubmittedTail();else root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}
              }
              // Native timeline spacing belongs to each row kind; work rows
              // are contiguous rather than receiving another list-wide gap.
              spacing: 0
              Controls.ScrollBar.vertical: Controls.ScrollBar { id:conversationBar; policy: Controls.ScrollBar.AsNeeded;onPressedChanged:{if(pressed){conversation.stopWheel();root.followingTail=false;root.submissionFollowKey="";root.cancelScrollRestore()}else{root.followingTail=conversation.atYEnd;root.capturePosition(conversation,root.bridge.transcript,root.bridge.selectedKey)}} }
              // No opacity transitions on pooled rows: interrupted animations can
              // leave a recycled message or changed-file card almost invisible.
              delegate: Item {
                id: message
                required property int index
                required property var row
                readonly property var modelData: row
                readonly property var textItem:messageText
                property bool pooled:false
                ListView.onPooled:{pooled=true;opacity=1}
                ListView.onReused:{pooled=false;opacity=1}
                readonly property bool userMessage: modelData.role === "user"
                readonly property bool commentary: modelData.role === "assistant" && index+1<root.bridge.transcript.length && root.bridge.transcript[index+1].role!=="user"
                readonly property real bottomGap:Style.space(modelData.role==="activity"?0:commentary?8:16)
                readonly property real verticalInset:Style.space(userMessage?12:modelData.role==="activity"?0:2)
                readonly property real laneWidth:messageLane.width
                width: conversation.width
                height: bubble.height+(messageActions.visible?messageActions.height+Style.space(6):0)+bottomGap
                HoverHandler{id:messageHover}
                FontMetrics{id:userMeasure;font:messageText.font}
                readonly property real naturalUserWidth:userMessage?String(modelData.text || "").split("\n").reduce(function(width,line){return Math.max(width,userMeasure.advanceWidth(line))},0):0
                Item {
                id:messageLane;objectName:"t3MessageLane"
                width:Math.min(message.width,root.chatContentMaxWidth);height:message.height;anchors.horizontalCenter:parent.horizontalCenter
                Rectangle {
                  id: bubble
                  objectName:"t3MessageBubble"
                  anchors.right: parent.right
                  width: message.userMessage ? Math.min(parent.width*0.8,Math.max(Style.space(80),message.naturalUserWidth+Style.space(24),(message.modelData.attachments || []).length?Style.space(240):0)) : parent.width
                  height: body.implicitHeight + 2*message.verticalInset
                  radius: Style.space(16)
                  color: message.userMessage ? root.surface : "transparent"
                  border.width:root.findOpen&&root.findMatches[root.findIndex]&&root.findMatches[root.findIndex].id===message.modelData.id?1:0
                  border.color:Color.accent
                  Column {
                    id: body
                    objectName:"t3MessageBody"
                  anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin:Style.space(message.userMessage?12:4);rightMargin:Style.space(message.userMessage?12:4);topMargin:message.verticalInset }
                    spacing: Style.space(6)
                    TextEdit {
                      id:messageText;objectName:"chatMessageText"
                      HoverHandler{objectName:"chatLinkCursor";cursorShape:messageText.hoveredLink?Qt.PointingHandCursor:Qt.IBeamCursor}
                      LayoutMirroring.enabled:false
                      visible: modelData.role !== "activity" && modelData.role !== "changes" && modelData.role !== "context" && modelData.role !== "plan"
                      width: parent.width
                      text: message.userMessage ? modelData.text || "" : root.renderMessage(modelData,width)
                      textFormat: message.userMessage || !modelData.rendered ? TextEdit.PlainText : TextEdit.RichText; readOnly: true; selectByMouse: true
                      onLinkActivated:function(link){if(String(link).indexOf("ot3-image-open://")===0){var a=(message.modelData.markdownImages || [])[Number(String(link).slice(17))];if(a){imagePopup.attachment=a;imagePopup.assetKey=root.bridge.imageKey(a);root.bridge.requestImage(a);imagePopup.open()}return}if(String(link).indexOf("ot3-code-")===0){root.bridge.codeAction(message.modelData,link);return}root.bridge.openReference(link)}
                      wrapMode: TextEdit.Wrap
                      color: root.fg; selectionColor: Color.accent; selectedTextColor: Color.popups.background
                      font.pixelSize: Style.font.body
                      Accessible.name:message.userMessage?T3I18n.tr("You"):"T3"
                    }
                    Repeater {
                      model:message.modelData.markdownImages || []
                      Item {
                        id:markdownImageRequest
                        required property var modelData
                        width:0;height:0
                        readonly property real viewportY:message.y+body.y-conversation.contentY
                        readonly property bool active:root.opened&&conversation.visible&&!message.pooled&&viewportY<conversation.height&&viewportY+message.height>0
                        function request(){if(active)root.bridge.requestImage(modelData)}
                        onActiveChanged:request()
                        onModelDataChanged:Qt.callLater(request)
                        Component.onCompleted:Qt.callLater(request)
                        Connections{target:root.bridge;function onImageStatesChanged(){if(markdownImageRequest.active&&!root.bridge.imageStates[root.bridge.imageKey(markdownImageRequest.modelData)])Qt.callLater(markdownImageRequest.request)}}
                      }
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
                    Repeater {
                      model:root.renderSide?[]:(modelData.attachments || []).filter(function(a){return a.mimeType==="text/html"})
                      T3InlineHtml {
                        required property var modelData
                        readonly property real viewportY:message.y+body.y+y-conversation.contentY
                        width:body.width;bridge:root.bridge;attachment:modelData
                        active:root.opened&&conversation.visible&&!message.pooled&&viewportY<conversation.height&&viewportY+height>0
                      }
                    }
                    Flow {
                      width:parent.width;spacing:Style.space(6);visible:root.renderSide
                      Repeater {
                        model:root.renderSide?(modelData.attachments || []).filter(function(a){return a.mimeType==="text/html"}):[]
                        T3FileReference {
                          required property var modelData
                          objectName:"t3RenderCard"
                          fileName:String(modelData.name || "HTML").replace(/\.html$/i,"");filePath:T3I18n.tr("Show in side panel")
                          border.width:root.renderShown&&root.renderAttachment&&root.renderAttachment.id===modelData.id?2:1
                          onClicked:root.showRender(modelData)
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
                        model:(modelData.attachments || []).filter(function(a){return a.type!=="image"&&a.mimeType!=="text/html"})
                        T3FileReference {
                          required property var modelData
                          fileName:modelData.name || "HTML"
                          onClicked:root.bridge.openReference("",modelData)
                        }
                      }
                    }
                    // Each row builds only the card for its own role. Hidden cards
                    // in every message made each row ~3x slower to create, and
                    // their on-screen bindings re-ran on every scroll frame.
                    Loader{
                      width:parent.width;active:message.modelData.role==="context";visible:active
                      sourceComponent:T3ContextDivider{width:parent.width;boundary:message.modelData.context || {};catalog:root.bridge.catalog;active:root.opened&&!message.pooled&&message.y<conversation.contentY+conversation.height&&message.y+message.height>conversation.contentY}
                    }
                    Loader{
                      width:parent.width;active:message.modelData.role==="activity";visible:active
                      sourceComponent:T3ActivityBlock{bridge:root.bridge;activity:message.modelData.activity || {};onInspectRequested:function(reference){toolPopup.open();toolInspector.openItem(reference)};active:root.opened&&!message.pooled&&message.y<conversation.contentY+conversation.height&&message.y+message.height>conversation.contentY}
                    }
                    Loader{
                      width:parent.width;active:message.modelData.role==="plan";visible:active
                      sourceComponent:T3PlanWorkflow{width:parent.width;planId:message.modelData.plan?message.modelData.plan.id:"";view:({plans:message.modelData.plan?[message.modelData.plan]:[],ready:null});onCopyRequested:function(markdown){root.bridge.copyText(markdown)};onLinkRequested:function(link){root.bridge.openReference(link)}}
                    }
                    Loader{
                      width:parent.width;active:message.modelData.role==="changes";visible:active
                      sourceComponent:T3Changes{summary:message.modelData.changes || {};onPreviewFile:function(filePath){root.bridge.openReference(filePath)};onOpenDiff:function(filePath){diffPopup.initialPath=filePath;root.bridge.requestDiff(summary);diffPopup.open()}}
                    }

                  }
                }
                Row {
                  id:messageActions;objectName:"t3MessageActions"
                  visible:(message.modelData.role==="user"||message.modelData.role==="assistant")&&!message.commentary&&(!!message.modelData.text||message.modelData.canRewindMessage)&&message.modelData.streaming!==true
                  anchors.top:bubble.bottom;anchors.topMargin:Style.space(6)
                  anchors.right:message.userMessage?bubble.right:undefined
                  anchors.left:message.userMessage?undefined:bubble.left
                  layoutDirection:message.userMessage?Qt.RightToLeft:Qt.LeftToRight
                  spacing:Style.space(8)
                  opacity:messageHover.hovered||messageCopy.activeFocus||messageRevert.activeFocus||messageCopy.copyState!=="idle"?1:0
                  T3IconButton{id:messageRevert;objectName:"t3EditFromHere";name:"undo-2";implicitWidth:Style.space(20);implicitHeight:implicitWidth;visible:message.userMessage&&!!message.modelData.canRewindMessage;enabled:root.bridge.canRewind===true&&!root.bridge.rewindPending&&!(root.bridge.rewindPreview || {}).busy;tooltipText:T3I18n.tr("Edit from here");onClicked:root.bridge.requestRewindPreview(message.modelData.id)}
                  T3CopyButton{id:messageCopy;objectName:"copyChatMessage";visible:!!message.modelData.text;implicitWidth:Style.space(20);implicitHeight:implicitWidth;bridge:root.bridge;copyText:message.modelData.text || ""}
                  Text{visible:!!message.modelData.createdAt;text:root.date(message.modelData.createdAt);color:root.dim;font.pixelSize:Style.font.caption;anchors.verticalCenter:parent.verticalCenter}
                }
                }
              }
              Connections {
                target: root.bridge
                function onMessageSubmitted(key) { root.followSubmittedMessage(key) }
                function onThreadWillChange() { root.findOpen=false;root.findQuery="";if(!root.bridge.connectionChanging)root.preferencesOpen=false;root.pullRequestsOpen=false;documentPopup.close();conversation.stopWheel();root.cancelScrollRestore();imagePopup.close();diffPopup.close();workingPopup.close();toolPopup.close();gitPopup.close();rewindWorkflow.dismiss();root.captureState();Qt.callLater(root.restoreSidebarShelves); root.submissionFollowKey = ""; root.restoringConversation = true }
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
          ColumnLayout {
            id:composerStack;objectName:"t3ComposerOverlay";z:5
            Layout.fillWidth:true;Layout.fillHeight:root.composerExpanded
            spacing:0
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
          RowLayout{
            objectName:"t3RewindBanner";Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(6)
            visible:root.bridge.rewindPending || !!(root.bridge.rewindPreview || {}).error || !!(root.bridge.rewindPreview || {}).busy
            WorkIndicator{visible:!!(root.bridge.rewindPreview || {}).busy||!!(root.bridge.rewindRecoveries || {})[root.bridge.selectedKey];active:visible}
            Text{Layout.fillWidth:true;text:(root.bridge.rewindPreview || {}).error?rewindWorkflow.errorText(root.bridge.rewindPreview.error):(root.bridge.rewindOperation || {}).recoveryError?rewindWorkflow.errorText(root.bridge.rewindOperation.recoveryError):T3I18n.tr((root.bridge.rewindPreview || {}).busy?"Loading…":(root.bridge.rewindOperation || {}).state==="complete"?"Restoring draft…":(root.bridge.rewindOperation || {}).state==="uncertain"?"Outcome unknown. Refresh to check before retrying the same action.":"Waiting for the thread to rewind…");color:(root.bridge.rewindOperation || {}).recoveryError?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
            T3PickerButton{primary:false;visible:root.bridge.rewindPending || false;enabled:!(root.bridge.rewindRecoveries || {})[root.bridge.selectedKey];text:T3I18n.tr((root.bridge.rewindOperation || {}).recoveryError?"Retry":"Edit from here");onClicked:if((root.bridge.rewindOperation || {}).recoveryError)root.bridge.recoverRewind(root.bridge.rewindOperation);else root.bridge.showPendingRewind()}
          }
          T3PlanWorkflow{
            objectName:"t3ReadyPlan";compact:true;Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(8)
            visible:!!root.bridge.readyPlan||!!(root.bridge.planView || {}).error||!!(root.bridge.planOperation || {}).error||!!(root.bridge.planOperation || {}).busy
            view:root.bridge.readyPlanView || ({plans:[],ready:null});modeEnabled:root.bridge.interactionModeEnabled===true;interactionMode:root.bridge.interactionMode || "default";canOperate:!root.bridge.providerSubagent&&root.bridge.catalog.canWrite!==false;busy:!!(root.bridge.planOperation || {}).busy||root.bridge.draft.busy||!!root.bridge.draft.attempt;draftText:root.bridge.draft.text;hasAttachments:root.bridge.draft.attachments.length>0
            onFollowUpRequested:function(plan,draft,newThread){root.bridge.requestPlanFollowUp(plan,draft,newThread)}
            onModeRequested:function(mode){root.bridge.chooseInteractionMode(mode)}
            onRetryRequested:root.bridge.retryPlanOperation()
            onCancelRequested:root.bridge.cancelPlanOperation()
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
          T3Approvals {
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(8)
            bridge:root.bridge
          }
          T3Questions {
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(8)
            bridge:root.bridge
          }
          ColumnLayout {
            objectName:"t3ProviderSubagentNotice"
            Layout.fillWidth:true;Layout.leftMargin:Style.space(24);Layout.rightMargin:Style.space(24);Layout.bottomMargin:Style.space(18)
            visible:root.bridge.providerSubagent===true
            Text {Layout.fillWidth:true;text:T3I18n.tr("This subagent is managed by its provider and cannot receive messages. Open the parent chat or create a new chat.");color:root.fg;wrapMode:Text.Wrap;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
            RowLayout {
              Button {objectName:"t3OpenParentThread";text:T3I18n.tr("Open parent chat");enabled:!!root.bridge.selectedInfo.parentThreadId;onClicked:root.bridge.openParentThread()}
              Button {text:T3I18n.tr("New chat");onClicked:sidebarActions.newThread()}
            }
            Button {objectName:"t3RecoverSubagentDraft";text:T3I18n.tr("Send and start new thread");visible:!!root.bridge.draft.text.trim()||root.bridge.draft.attachments.length>0;enabled:!root.bridge.draft.loading&&!root.bridge.draft.busy&&!root.bridge.draft.attempt&&!root.bridge.managementBusy;onClicked:root.bridge.submit(true)}
          }
          Item {
            id:composerSlot
            Layout.fillWidth:true;Layout.fillHeight:root.composerExpanded
            Layout.preferredHeight:composerPanel.implicitHeight+Style.space(18)
            visible:!!root.bridge.selectedThread && root.bridge.providerSubagent!==true && !(root.bridge.questions || []).length
          T3Composer {
            id: composerPanel
            anchors.horizontalCenter:parent.horizontalCenter
            width:root.composerExpanded?parent.width-Style.space(48):Math.min(parent.width-Style.space(48),root.chatContentMaxWidth)
            height:root.composerExpanded?parent.height-Style.space(18):implicitHeight
            Rectangle{anchors.top:parent.bottom;x:-composerPanel.x;width:composerSlot.width;height:Style.space(18);color:Color.popups.background}
            bridge: root.bridge
            expanded: root.composerExpanded
            onToggleExpanded: root.expandComposer()
            onPreviewAttachment:function(a){if(a.type==="image"&&["image/png","image/jpeg","image/gif","image/webp"].indexOf(a.mimeType)>=0){imagePopup.attachment=a;imagePopup.assetKey=root.bridge.imageKey(a);root.bridge.requestImage(a);imagePopup.open()}else root.bridge.openReference("",a)}
          }
          }
          }
        }
      }
      Item {
        visible:root.sideShown&&root.browserInline;Layout.fillHeight:true;Layout.preferredWidth:Style.space(6);activeFocusOnTab:true
        Accessible.role:Accessible.Slider;Accessible.name:T3I18n.tr("Side panel width")
        Keys.onLeftPressed:root.resizeBrowser(root.browserPanelWidth+10)
        Keys.onRightPressed:root.resizeBrowser(root.browserPanelWidth-10)
        Rectangle{anchors.centerIn:parent;width:browserDrag.containsMouse||browserDrag.pressed?2:1;height:parent.height;color:browserDrag.containsMouse||browserDrag.pressed?Color.accent:root.divider}
        MouseArea{id:browserDrag;objectName:"t3BrowserResizer";anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.SizeHorCursor;property real startX:0;property real startWidth:540;onPressed:function(m){startX=mapToGlobal(m.x,m.y).x;startWidth=Math.min(root.browserMaxWidth,root.browserPanelWidth)};onPositionChanged:function(m){if(pressed)root.resizeBrowser(startWidth-(T3I18n.rtl?-1:1)*(mapToGlobal(m.x,m.y).x-startX))}}
      }
      Loader{id:inlineSideLoader;objectName:"t3BrowserInline";visible:root.sideShown&&root.browserInline;active:visible;Layout.fillHeight:true;Layout.preferredWidth:Style.space(Math.min(root.browserMaxWidth,root.browserPanelWidth));sourceComponent:sidePanelComponent}
    }
  }
}
