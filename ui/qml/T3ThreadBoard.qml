import QtQuick 6.11
import QtQuick.Window 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/SidebarModel.js" as Sidebar
import "../js/ProjectTree.js" as ProjectTree
import "../js/ModelSync.js" as ModelSync

Rectangle {
  id:root
  objectName:"t3ThreadBoard"
  required property var bridge
  readonly property var sky:sidebarContext?sidebarContext.starfield:null
  property var threads:[]
  property var renderer:null
  property var sidebarContext:null
  property var sectionData:null
  property var shownSections:null
  property var columnSpecs:[]
  signal pullRequestsRequested(var source,var reference,bool multiple)
  signal filterProjectRequested(string key)
  signal projectSettingsRequested(string key)
  signal newThreadOnBranch(var source)
  signal projectContextRequested(var source,var item,real px,real py)
  signal projectContextDismissed()
  signal subthreadsToggled(var source)
  signal ideaRequested(string text)
  signal closed()
  signal threadRequested(var thread)
  readonly property color fg:Color.popups.text
  readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.52)
  readonly property color line:Qt.rgba(fg.r,fg.g,fg.b,0.085)
  property bool liveMode:bridge.viewState.boardLive===true
  property var liveKeys:[]
  property string editingKey:""
  property string editingIdea:""
  property var interactions:({})
  property var shownThreads:[]
  onThreadsChanged:sourceSync.restart()
  onSectionDataChanged:sourceSync.restart()
  Timer{id:sourceSync;interval:0;onTriggered:root.syncSource()}
  function syncSource(){
    if(Object.keys(interactions).length)return
    var previousKey=navKey,previousColumn=navColumn,hadFocus=focusedInside()
    shownThreads=threads;shownSections=sectionData
    var specs=sectionData?sectionData.map(function(s){return {key:s.key,label:s.label}}):ProjectTree.states().map(function(s){return {key:s.id,label:s.label}})
    if(JSON.stringify(specs)!==JSON.stringify(columnSpecs))columnSpecs=specs
    var nextColumn=columnFor(previousKey)
    if(nextColumn){navColumn=nextColumn;navKey=previousKey;navIndex=rows(nextColumn).findIndex(function(t){return t.key===previousKey})}
    else if(previousColumn!=="ideas"&&columnSpecs.every(function(s){return s.key!==previousColumn})){navColumn="ideas";navKey="";navIndex=0}
    if(hadFocus&&!activeEditor)Qt.callLater(function(){if(root)root.restoreFocus()})
  }
  function holdRow(key,active){
    if(!!interactions[key]===active)return
    var next=Object.assign({},interactions);if(active)next[key]=true;else delete next[key];interactions=next
    if(!Object.keys(next).length)sourceSync.restart()
    if(sidebarContext&&typeof sidebarContext.holdSidebar==="function")sidebarContext.holdSidebar("@board/"+key,active)
  }
  property var visibleKeys:({})
  function watchVisible(key,visible){if(!!visibleKeys[key]===visible)return;var next=Object.assign({},visibleKeys);if(visible)next[key]=true;else delete next[key];visibleKeys=next}
  readonly property var liveThreads:{
    var shown=shownThreads.filter(function(t){return !!root.visibleKeys[t.key]&&(root.columnLive(root.columnFor(t.key))||root.liveKeys.indexOf(t.key)>=0)})
    var editor=shown.filter(function(t){return t.key===root.editingKey})
    return editor.concat(shown.filter(function(t){return t.key!==root.editingKey})).slice(0,24)
  }
  readonly property string watchKeys:liveThreads.map(function(t){return t.key}).join("\n")
  onWatchKeysChanged:watchDelay.restart()
  Timer{id:watchDelay;interval:60;onTriggered:root.bridge.watchMosaic(root.liveThreads,24)}
  Component.onDestruction:{if(sidebarContext&&typeof sidebarContext.holdSidebar==="function")Object.keys(interactions).forEach(function(key){root.sidebarContext.holdSidebar("@board/"+key,false)});bridge.watchMosaic([])}
  color:Color.popups.background
  activeFocusOnTab:true

  // Keep the opaque backing over the conversation, then repaint the panel's
  // sky above it. Following the existing leader shares its clock and settings.
  T3Starfield {
    id:boardSky;objectName:"t3BoardStarfield";anchors.fill:parent
    leader:root.sky;visible:!!leader;mode:"off";paused:!visible
    brightness:leader?leader.brightness:0.5
    nebula:leader?leader.nebula:0
    meteorRate:leader?leader.meteorRate:0
    function alignOrigin(){var p=leader&&typeof leader.mapFromItem==="function"?leader.mapFromItem(root,0,0):Qt.point(0,0);origin=Qt.vector2d(p.x,p.y)}
    onWidthChanged:alignOrigin()
    onHeightChanged:alignOrigin()
    onLeaderChanged:Qt.callLater(alignOrigin)
  }

  ListModel{id:ideas}
  function persistIdeas(){var values=[];for(var i=0;i<ideas.count;i++){var v=ideas.get(i);values.push({key:v.key,text:v.text})}bridge.rememberView({boardIdeas:values})}
  function addIdea(text){if(columnSetting("ideas").collapsed)configureColumn("ideas",{collapsed:false});var key="idea-"+Date.now()+"-"+ideas.count;ideas.append({key:key,text:text || ""});editingIdea=key;persistIdeas();navColumn="ideas";navKey=key;navIndex=ideas.count-1;Qt.callLater(function(){root.focusIdeaEditor(key)});return key}
  function updateIdea(key,text){for(var i=0;i<ideas.count;i++)if(ideas.get(i).key===key){if(ideas.get(i).text!==text){ideas.setProperty(i,"text",text);persistIdeas()}return}}
  // Keep the previous single-pad draft when moving to independent idea cards.
  Component.onCompleted:{
    syncSource()
    var saved=bridge.viewState.boardIdeas || []
    saved.forEach(function(v){if(v&&typeof v.key==="string"&&typeof v.text==="string")ideas.append({key:v.key,text:v.text})})
    Qt.callLater(function(){root.focusCell(0,0)})
    if(!saved.length&&(bridge.viewState.boardIdeaText || "").trim()){addIdea(bridge.viewState.boardIdeaText);bridge.rememberView({boardIdeaText:""})}
  }
  function bucket(t){return ProjectTree.state(t)}
  function columnSetting(key){return (bridge.viewState.boardColumns || {})[key] || {}}
  readonly property var allColumnKeys:["ideas","pinned","ready","working","input","snoozed","settled","waiting","failed","idle","done"]
  readonly property var sortOptions:[{key:"sidebar",label:"Use sidebar order"},{key:"reply",label:"Recent reply"},{key:"updated",label:"Recently updated"},{key:"title",label:"Title"},{key:"project",label:"Project"},{key:"environmentName",label:"Host"}]
  function configureAllColumns(patch){var all=Object.assign({},bridge.viewState.boardColumns || {});allColumnKeys.forEach(function(key){all[key]=Object.assign({},all[key] || {},patch)});bridge.rememberView({boardColumns:all})}
  function commonSetting(name,fallback){var keys=allColumnKeys.filter(function(k){return k!=="ideas"}),values=keys.map(function(k){var v=root.columnSetting(k)[name];return v===undefined?fallback:v});return values.every(function(v){return v===values[0]})?values[0]:undefined}
  readonly property bool filtersActive:!!bridge.query||!!sidebarContext&&(!!sidebarContext.hostFilter||!!sidebarContext.projectFilter||!!sidebarContext.threadGroupFilter||sidebarContext.statusFilter!=="all")||bridge.viewState.boardShowPinned===false
  function clearFilters(){if(sidebarContext){sidebarContext.hostFilter="";sidebarContext.projectFilter="";sidebarContext.threadGroupFilter="";sidebarContext.statusFilter="all"}bridge.search("")}
  function showGlobalOptions(){focusDelay.stop();globalOptions.open()}
  function configureColumn(key,patch){var all=Object.assign({},bridge.viewState.boardColumns || {});all[key]=Object.assign({},all[key] || {},patch);bridge.rememberView({boardColumns:all})}
  function rawRows(key){var section=(shownSections || []).find(function(s){return s.key===key});return section?section.rows:shownThreads.filter(function(t){return t.relationshipToParent!=="subagent"&&root.bucket(t)===key})}
  function rows(key){
    var input=rawRows(key),setting=columnSetting(key),order=setting.order || "sidebar"
    if(order==="sidebar"&&!setting.reverse)return input
    var compare=order==="reply"?Sidebar.compareReply:order==="updated"?function(a,b){return (b.updatedAt || 0)-(a.updatedAt || 0)}:function(a,b){return Sidebar.fold(a[order] || "").localeCompare(Sidebar.fold(b[order] || ""))}
    // Sort parents, then keep their expanded subthreads immediately underneath.
    var parents=input.filter(function(t){return !t.depth}),children={}
    input.filter(function(t){return t.depth>0}).forEach(function(t){var parent=t.parentThreadId;var owner=input.find(function(p){return p.id===parent});while(owner&&owner.depth>0){parent=owner.parentThreadId;owner=input.find(function(p){return p.id===parent})}var key=owner?owner.key:t.key;(children[key] || (children[key]=[])).push(t)})
    if(order!=="sidebar")parents.sort(function(a,b){return compare(a,b)||String(a.key).localeCompare(String(b.key))})
    if(setting.reverse)parents.reverse()
    var out=[];parents.forEach(function(t){out.push(t);out=out.concat(children[t.key] || [])});return out
  }
  function columnLive(key){var value=columnSetting(key).live;return typeof value==="boolean"?value:liveMode}
  function toggleGlobalLive(){liveMode=!liveMode;var all=Object.assign({},bridge.viewState.boardColumns || {});Object.keys(all).forEach(function(k){all[k]=Object.assign({},all[k]);delete all[k].live});bridge.rememberView({boardLive:liveMode,boardColumns:all})}
  function columnCount(key){return rawRows(key).filter(function(t){return !t.depth}).length+(key==="settled"&&sidebarContext&&!sidebarContext.projectLayout?sidebarContext.olderSettledMissing || 0:0)}
  function columnTitle(spec){return T3I18n.tr(spec.label,{count:columnCount(spec.key)})}
  function loadOlder(){if(sidebarContext)(bridge.state.environments || []).forEach(function(e){if(!root.sidebarContext.hostFilter||root.sidebarContext.hostFilter===e.id)bridge.loadOlderSettled(e.id)})}
  property string navColumn:"ideas"
  property string navKey:""
  property int navIndex:0
  property var activeEditor:null
  property var helpEditor:null
  property string helpHoldKey:""
  property bool columnMenuOpen:false
  function showHelp(){helpEditor=activeEditor;if(helpEditor){helpHoldKey="help/"+navKey;holdRow(helpHoldKey,true)}focusDelay.stop();keyboardHelp.open()}
  property string pendingFocusColumn:"ideas"
  property int pendingFocusIndex:0
  function focusedInside(){var item=root.Window.window?root.Window.window.activeFocusItem:null;while(item){if(item===root)return true;item=item.parent}return false}
  function columnFor(key){if(shownSections){var section=shownSections.find(function(s){return s.rows.some(function(t){return t.key===key})});return section?section.key:""}var t=shownThreads.find(function(t){return t.key===key});return t?bucket(t):""}
  function columnIndex(key){return key==="ideas"?0:columnSpecs.findIndex(function(s){return s.key===key})+1}
  function viewAt(index){return index===0?ideaList:sectionRepeater.itemAt(index-1)?sectionRepeater.itemAt(index-1).view:null}
  function columnAt(index){return index===0?ideasColumn:sectionRepeater.itemAt(index-1)}
  function cellRows(index){if(index===0){var out=[];for(var i=0;i<ideas.count;i++)out.push({key:ideas.get(i).key,text:ideas.get(i).text});return out}return index>0&&index<=columnSpecs.length?rows(columnSpecs[index-1].key):[]}
  function rememberFocus(column,key,index){navColumn=column;navKey=key;navIndex=index}
  function revealColumn(index){var item=columnAt(index);if(!item)return;var x=viewport.contentX;if(item.x<x)x=item.x;else if(item.x+item.width>x+viewport.width)x=item.x+item.width-viewport.width;viewport.contentX=Math.max(0,Math.min(viewport.contentWidth-viewport.width,x))}
  function focusCell(index,row){
    index=Math.max(0,Math.min(columnSpecs.length,index));var column=index===0?"ideas":columnSpecs[index-1].key,data=cellRows(index)
    if(columnSetting(column).collapsed){rememberFocus(column,"",0);revealColumn(index);pendingFocusColumn=column;pendingFocusIndex=0;focusDelay.restart();return}
    var at=data.length?Math.max(0,Math.min(data.length-1,row)):0
    rememberFocus(column,data.length?data[at].key:"",at);revealColumn(index)
    var view=viewAt(index);if(view&&data.length){view.currentIndex=at;view.positionViewAtIndex(at,ListView.Contain)}
    pendingFocusColumn=column;pendingFocusIndex=at;focusDelay.restart()
  }
  Timer{id:focusDelay;interval:16;onTriggered:{
    if(root.activeEditor)return
    var index=root.columnIndex(root.pendingFocusColumn),view=root.viewAt(index),card=view?view.itemAtIndex(root.pendingFocusIndex):null
    if(root.columnSetting(root.pendingFocusColumn).collapsed){var collapsed=root.columnAt(index);if(collapsed)collapsed.header.forceActiveFocus();return}
    if(card)card.forceActiveFocus();else if(index===0)addIdeaButton.forceActiveFocus();else{var column=root.columnAt(index);if(column)column.header.forceActiveFocus()}
  }}
  function restoreFocus(){if(!activeEditor)focusCell(columnIndex(navColumn),navIndex)}
  function focusedThread(){return shownThreads.find(function(t){return t.key===navKey}) || null}
  function openColumnMenu(){focusDelay.stop();var index=columnIndex(navColumn),column=columnAt(index);if(index>0&&column)column.menu.open();else addIdea("")}
  function contextMenu(){focusDelay.stop();if(columnSetting(navColumn).collapsed){if(navColumn==="ideas")showGlobalOptions();else openColumnMenu();return}var view=viewAt(columnIndex(navColumn)),card=view?view.itemAtIndex(navIndex):null;if(card&&card.sharedRow)card.sharedRow.openContextMenu()}
  function editFocused(){if(navColumn==="ideas"){if(!navKey){addIdea("");return}editingIdea=navKey;Qt.callLater(function(){root.focusIdeaEditor(root.navKey)})}else{var t=focusedThread();if(t)writeTo(t)}}
  function focusIdeaEditor(key){var i=cellRows(0).findIndex(function(v){return v.key===key});if(i<0)return;revealColumn(0);ideaList.positionViewAtIndex(i,ListView.Contain);Qt.callLater(function(){var card=ideaList.itemAtIndex(i);if(card)card.editor.forceActiveFocus()})}
  function editorFocus(editor,active){if(active)activeEditor=editor;else if(activeEditor===editor)activeEditor=null}
  function finishEditor(send){
    if(activeEditor&&String(activeEditor.objectName).indexOf("t3BoardIdeaEditor_")===0){editingIdea="";activeEditor=null;restoreFocus();return}
    if(send){if(bridge.selectedKey===editingKey&&!bridge.draft.loading&&!bridge.draft.busy&&!bridge.draft.attempt&&!bridge.draft.loadError&&!!bridge.draft.text.trim())bridge.submit();return}
    editingKey="";activeEditor=null;restoreFocus()
  }
  Keys.priority:Keys.AfterItem
  Keys.onPressed:function(event){
    if(columnMenuOpen||globalOptions.visible||!activeEditor&&Object.keys(interactions).length)return
    if(event.key===Qt.Key_F1){showHelp();event.accepted=true;return}
    if(activeEditor)return
    var index=columnIndex(navColumn),data=cellRows(index),at=data.findIndex(function(t){return t.key===root.navKey});if(at<0)at=navIndex
    var t=focusedThread(),control=!!(event.modifiers&Qt.ControlModifier)
    if(event.key===Qt.Key_Escape){closed();event.accepted=true}
    else if(event.key===Qt.Key_Menu||event.key===Qt.Key_F10&&event.modifiers===Qt.ShiftModifier){contextMenu();event.accepted=true}
    else if(control&&event.key===Qt.Key_L){toggleGlobalLive();event.accepted=true}
    else if(event.modifiers===Qt.NoModifier||control&&(event.key===Qt.Key_Home||event.key===Qt.Key_End)){
      switch(event.key){
        case Qt.Key_Left:focusCell(index-1,at);break
        case Qt.Key_Right:focusCell(index+1,at);break
        case Qt.Key_Up:focusCell(index,at-1);break
        case Qt.Key_Down:focusCell(index,at+1);break
        case Qt.Key_Home:focusCell(control?0:index,0);break
        case Qt.Key_End:var end=control?columnSpecs.length:index;focusCell(end,cellRows(end).length-1);break
        case Qt.Key_PageUp:focusCell(index,at-5);break
        case Qt.Key_PageDown:focusCell(index,at+5);break
        case Qt.Key_Return:case Qt.Key_Enter:if(index===0)editFocused();else if(t)threadRequested(t);else openColumnMenu();break
        case Qt.Key_Space:if(index===0)editFocused();else if(t)toggleLive(t);break
        case Qt.Key_E:case Qt.Key_F2:editFocused();break
        case Qt.Key_O:openColumnMenu();break
        case Qt.Key_G:showGlobalOptions();break
        case Qt.Key_N:addIdea("");break
        default:return
      }
      event.accepted=true
    }
  }
  Controls.Popup {
    id:globalOptions;objectName:"t3BoardGlobalPopup";parent:root
    x:Math.max(Style.space(8),root.width-width-Style.space(20));y:Style.space(52)
    width:Math.min(Style.space(380),root.width-Style.space(32));height:Math.min(root.height-Style.space(72),globalBody.implicitHeight+2*padding)
    padding:Style.space(16);focus:true
    palette.button:Qt.tint(Color.popups.background,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06));palette.placeholderText:root.dim;palette.window:Color.popups.background;palette.windowText:root.fg;palette.text:root.fg;palette.buttonText:root.fg;palette.base:Color.popups.background;palette.highlight:Color.accent;palette.highlightedText:Color.popups.background
    background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:root.line}
    onOpened:globalSearch.forceActiveFocus()
    onClosed:root.restoreFocus()
    contentItem:Controls.ScrollView {
      id:globalScroll;contentWidth:availableWidth;clip:true
      ColumnLayout {
        id:globalBody;objectName:"t3BoardGlobalBody";width:globalScroll.availableWidth;spacing:Style.space(8)
        Keys.priority:Keys.AfterItem
        Keys.onEscapePressed:function(event){globalOptions.close();event.accepted=true}
        Text{text:T3I18n.tr("Filter by state, project and host");Layout.fillWidth:true;color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
        Controls.TextField {
          id:globalSearch;objectName:"t3BoardGlobalSearch";Layout.fillWidth:true;placeholderText:T3I18n.tr("Search threads…");text:root.bridge.query
          onTextEdited:root.bridge.search(text)
        }
        Text{visible:!!root.sidebarContext;text:T3I18n.tr("Host");color:root.dim;font.pixelSize:Style.font.caption}
        Controls.ComboBox {
          objectName:"t3BoardGlobalHost";visible:!!root.sidebarContext;Layout.fillWidth:true
          model:root.sidebarContext?root.sidebarContext.hostItems:[];textRole:"name"
          currentIndex:root.sidebarContext?model.findIndex(function(h){return h.id===root.sidebarContext.hostFilter}):-1
          onActivated:function(i){root.sidebarContext.hostFilter=model[i].id}
        }
        Text{visible:!!root.sidebarContext;text:T3I18n.tr("Project");color:root.dim;font.pixelSize:Style.font.caption}
        Controls.ComboBox {
          objectName:"t3BoardGlobalProject";visible:!!root.sidebarContext;Layout.fillWidth:true
          model:root.sidebarContext?root.sidebarContext.projectItems:[];textRole:"title"
          currentIndex:root.sidebarContext?model.findIndex(function(p){return p.key===root.sidebarContext.projectFilter}):-1
          onActivated:function(i){root.sidebarContext.projectFilter=model[i].key}
        }
        Text{visible:!!root.sidebarContext;text:T3I18n.tr("Status");color:root.dim;font.pixelSize:Style.font.caption}
        Controls.ComboBox {
          objectName:"t3BoardGlobalStatus";visible:!!root.sidebarContext;Layout.fillWidth:true;textRole:"label"
          model:[{key:"all",label:T3I18n.tr("All")},{key:"working",label:T3I18n.tr("Working")},{key:"done",label:T3I18n.tr("Done")},{key:"failed",label:T3I18n.tr("Failed")},{key:"pending",label:T3I18n.tr("Pending")},{key:"waiting",label:T3I18n.tr("Waiting")},{key:"idle",label:T3I18n.tr("Idle")},{key:"settled",label:T3I18n.tr("Settled")}]
          currentIndex:root.sidebarContext?model.findIndex(function(s){return s.key===root.sidebarContext.statusFilter}):-1
          onActivated:function(i){root.sidebarContext.statusFilter=model[i].key}
        }
        T3PickerButton{objectName:"t3BoardGlobalClear";Layout.fillWidth:true;primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Clear filters");onClicked:root.clearFilters()}
        Rectangle{Layout.fillWidth:true;implicitHeight:1;color:root.line}
        Text{text:T3I18n.tr("All columns");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold}
        Text{text:T3I18n.tr("Sort by");color:root.dim;font.pixelSize:Style.font.caption}
        Controls.ComboBox {
          objectName:"t3BoardGlobalSort";Layout.fillWidth:true;textRole:"label"
          model:root.sortOptions.map(function(o){return {key:o.key,label:T3I18n.tr(o.label)}})
          currentIndex:model.findIndex(function(o){return o.key===root.commonSetting("order","sidebar")})
          displayText:currentIndex<0?"—":currentText
          onActivated:function(i){root.configureAllColumns({order:model[i].key})}
        }
        Controls.CheckBox {
          objectName:"t3BoardGlobalReverse";text:T3I18n.tr("Reverse order");tristate:true
          nextCheckState:function(){return checkState===Qt.Checked?Qt.Unchecked:Qt.Checked}
          checkState:root.commonSetting("reverse",false)===undefined?Qt.PartiallyChecked:root.commonSetting("reverse",false)?Qt.Checked:Qt.Unchecked
          onClicked:root.configureAllColumns({reverse:checkState===Qt.Checked})
        }
        Controls.CheckBox {
          objectName:"t3BoardGlobalPinned";visible:!root.sidebarContext||!root.sidebarContext.projectLayout&&root.sidebarContext.sidebarGrouped
          text:T3I18n.tr("Show pinned column");checked:root.bridge.viewState.boardShowPinned!==false
          onClicked:root.bridge.rememberView({boardShowPinned:checked})
        }
        Text{text:T3I18n.tr("Live view");color:root.dim;font.pixelSize:Style.font.caption}
        Controls.ComboBox {
          objectName:"t3BoardGlobalLive";Layout.fillWidth:true;textRole:"label"
          model:[{value:null,label:T3I18n.tr("Follow global live view")},{value:true,label:T3I18n.tr("Expand live cards")},{value:false,label:T3I18n.tr("Compact cards")}]
          currentIndex:model.findIndex(function(o){return o.value===root.commonSetting("live",null)})
          displayText:currentIndex<0?"—":currentText
          onActivated:function(i){root.configureAllColumns({live:model[i].value})}
        }
        T3PickerButton{objectName:"t3BoardGlobalCollapse";Layout.fillWidth:true;primary:false;outlined:true;implicitHeight:Style.space(30);pixelSize:Style.font.caption;text:T3I18n.tr("Collapse all columns");onClicked:root.configureAllColumns({collapsed:true})}
        T3PickerButton{objectName:"t3BoardGlobalExpand";Layout.fillWidth:true;primary:false;outlined:true;implicitHeight:Style.space(30);pixelSize:Style.font.caption;text:T3I18n.tr("Expand all columns");onClicked:root.configureAllColumns({collapsed:false})}
      }
    }
  }

  Controls.Popup {
    id:keyboardHelp;objectName:"t3BoardKeyboardHelp";
    palette.button:Qt.tint(Color.popups.background,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06));palette.placeholderText:root.dim;palette.window:Color.popups.background;palette.windowText:root.fg;palette.text:root.fg;palette.buttonText:root.fg;palette.base:Color.popups.background;palette.highlight:Color.accent;palette.highlightedText:Color.popups.background
    background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:root.line}
    parent:root;anchors.centerIn:parent;width:Math.min(root.width-Style.space(40),Style.space(480));padding:Style.space(20);modal:true;focus:true
    onOpened:contentItem.forceActiveFocus()
    onClosed:{if(root.helpEditor&&root.helpEditor.visible&&root.helpEditor.enabled)root.helpEditor.forceActiveFocus();else root.restoreFocus();root.helpEditor=null;if(root.helpHoldKey){root.holdRow(root.helpHoldKey,false);root.helpHoldKey=""}}
    contentItem:ColumnLayout {
      focus:true
      Keys.priority:Keys.BeforeItem
      Keys.onEscapePressed:function(event){keyboardHelp.close();event.accepted=true}
      spacing:Style.space(12)
      Text{text:T3I18n.tr("Keyboard shortcuts");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold}
      Repeater {
        model:[{keys:"← →",label:"Navigate columns"},{keys:"↑ ↓ · Home · End · PgUp · PgDn",label:"Navigate cards"},{keys:"Enter",label:"Open thread"},{keys:"Space",label:"Live view"},{keys:"E / F2",label:"Write message"},{keys:"Shift+F10 / Menu",label:"Thread actions"},{keys:"O",label:"Column options"},{keys:"G",label:"Filter by state, project and host"},{keys:"N",label:"New idea"},{keys:"Ctrl+L",label:"Live view"},{keys:"Ctrl+Enter",label:"Done / Send"},{keys:"Esc",label:"Close"}]
        RowLayout{required property var modelData;Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr(parent.modelData.label);color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}Text{text:parent.modelData.keys;color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}}
      }
      T3PickerButton{Layout.alignment:Qt.AlignRight;primary:false;text:T3I18n.tr("Close");onClicked:keyboardHelp.close()}
    }
  }

  function toggleLive(t){if(liveKeys.indexOf(t.key)>=0){liveKeys=liveKeys.filter(function(k){return k!==t.key});if(editingKey===t.key)editingKey=""}else{liveKeys=liveKeys.slice(-7).concat([t.key])}}
  function writeTo(t){if(liveKeys.indexOf(t.key)<0)liveKeys=liveKeys.slice(-7).concat([t.key]);editingKey=editingKey===t.key?"":t.key;if(editingKey&&bridge.selectedKey!==t.key)bridge.selectThread(t.environmentId,t.id)}

  ColumnLayout {
    anchors.fill:parent;anchors.margins:Style.space(20);spacing:Style.space(18)
    RowLayout {
      Layout.fillWidth:true;spacing:Style.space(10)
      T3Icon{name:"layout-grid";tint:root.dim;implicitWidth:Style.space(18)}
      Text{text:T3I18n.tr("Task board");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold}
      Text{text:String(root.threads.length);color:root.dim;font.pixelSize:Style.font.caption}
      Item{Layout.fillWidth:true}
      T3PickerButton{objectName:"t3BoardLiveMode";primary:false;outlined:root.liveMode;implicitHeight:Style.space(30);text:T3I18n.tr("Live view");onClicked:root.toggleGlobalLive()}
      T3IconButton{objectName:"t3BoardGlobalOptions";name:"list-filter";tooltipText:T3I18n.tr("Filter by state, project and host");tint:root.filtersActive?Color.accent:root.dim;onClicked:root.showGlobalOptions()}
      T3IconButton{name:"keyboard";tooltipText:T3I18n.tr("Keyboard shortcuts");tint:root.dim;onClicked:root.showHelp()}
      T3IconButton{objectName:"t3FocusBoardConversation";name:"x";tooltipText:T3I18n.tr("Conversation");tint:root.dim;onClicked:root.closed()}
    }
    Flickable {
      id:viewport;objectName:"t3BoardViewport"
      Layout.fillWidth:true;Layout.fillHeight:true;clip:true
      contentWidth:columns.width;contentHeight:height;flickableDirection:Flickable.HorizontalFlick;boundsBehavior:Flickable.StopAtBounds
      Controls.ScrollBar.horizontal:Controls.ScrollBar{}
      Row {
        id:columns;height:viewport.height;spacing:Style.space(14)
        readonly property real columnWidth:Math.max(Style.space(275),Math.min(Style.space(370),(viewport.width-4*spacing)/5))
        Rectangle {
          id:ideasColumn;property var header:root.columnSetting("ideas").collapsed?ideasRail:ideasHeader;width:root.columnSetting("ideas").collapsed?Style.space(44):columns.columnWidth;height:columns.height;radius:Style.space(12);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025)
          Controls.AbstractButton {
            id:ideasRail;objectName:"t3BoardRail_ideas";anchors.fill:parent;visible:!!root.columnSetting("ideas").collapsed
            activeFocusOnTab:activeFocus||root.navColumn==="ideas"&&!root.navKey
            Accessible.name:T3I18n.tr("Ideas")+" · "+T3I18n.tr("Expand column")
            Keys.onReturnPressed:function(event){event.accepted=true;ideasRail.clicked()}
            Keys.onEnterPressed:function(event){event.accepted=true;ideasRail.clicked()}
            Keys.onSpacePressed:function(event){event.accepted=true;ideasRail.clicked()}
            onClicked:{root.configureColumn("ideas",{collapsed:false});root.focusCell(0,0)}
            background:Rectangle{color:ideasRail.hovered?root.line:"transparent";radius:Style.space(10);border.width:ideasRail.activeFocus?1:0;border.color:Color.accent}
            contentItem:Item{T3Icon{anchors{horizontalCenter:parent.horizontalCenter;top:parent.top;topMargin:Style.space(12)}
name:"chevron-right";tint:root.dim;implicitWidth:Style.space(14)}Text{objectName:"t3BoardRailTitle_ideas";anchors.centerIn:parent;width:parent.height-Style.space(80);rotation:90;text:T3I18n.tr("Ideas")+" · "+ideas.count;color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}}
          }
          ColumnLayout {
            visible:!root.columnSetting("ideas").collapsed
            anchors.fill:parent;anchors.margins:Style.space(10);spacing:Style.space(10)
            RowLayout {
              id:ideasHeader
              Layout.fillWidth:true;spacing:Style.space(7)
              T3Icon{name:"sparkles";tint:root.dim;implicitWidth:Style.space(14)}
              Text{text:T3I18n.tr("Ideas");color:root.fg;font.pixelSize:Style.font.caption;font.weight:Font.DemiBold}
              Text{text:String(ideas.count);color:root.dim;font.pixelSize:Style.font.caption}
              Item{Layout.fillWidth:true}
              T3IconButton{name:"panel-left-close";tooltipText:T3I18n.tr("Collapse column");tint:root.dim;onClicked:root.configureColumn("ideas",{collapsed:true})}
              T3IconButton{id:addIdeaButton;objectName:"t3BoardAddIdea";name:"plus";tooltipText:T3I18n.tr("Add idea");tint:root.dim;onClicked:{root.addIdea("");ideaList.positionViewAtEnd()}}
            }
            ListView {
              id:ideaList;objectName:"t3BoardIdeas";Layout.fillWidth:true;Layout.fillHeight:true;model:ideas;clip:true;spacing:Style.space(8)
              T3Wheel{flickable:ideaList}
              Controls.ScrollBar.vertical:Controls.ScrollBar{}
              delegate:Rectangle {
                id:ideaCard;objectName:"t3BoardIdea_"+key
                required property string key
                required property string text
                required property int index
                readonly property bool editing:root.editingIdea===key
                property alias editor:ideaEditor
                activeFocusOnTab:activeFocus||root.navColumn==="ideas"&&root.navKey===key
                onActiveFocusChanged:if(activeFocus)root.rememberFocus("ideas",key,index)
                Accessible.role:Accessible.Button
                Accessible.name:text || T3I18n.tr("New idea")
                width:ListView.view.width;height:ideaBody.implicitHeight+Style.space(24)
                radius:Style.space(9);color:Color.popups.background;border.width:1;border.color:ideaCard.activeFocus||ideaEditor.activeFocus?Color.accent:editing?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.22):root.line
                ColumnLayout {
                  id:ideaBody;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(12)}
spacing:Style.space(10)
                  Text {
                    visible:!ideaCard.editing;Layout.fillWidth:true
                    text:ideaCard.text || T3I18n.tr("New idea");textFormat:Text.PlainText;color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;maximumLineCount:8;elide:Text.ElideRight
                    TapHandler{onTapped:root.editingIdea=ideaCard.key}
                  }
                  Controls.ScrollView {
                    id:ideaScroll;contentWidth:availableWidth;visible:ideaCard.editing;Layout.fillWidth:true;clip:true
                    Layout.preferredHeight:Math.max(Style.space(120),Math.min(Style.space(320),ideaEditor.contentHeight+Style.space(16)))
                    Controls.TextArea {
                    id:ideaEditor;objectName:"t3BoardIdeaEditor_"+ideaCard.key;implicitWidth:0;implicitHeight:0;width:Math.max(1,ideaScroll.availableWidth);height:Math.max(ideaScroll.availableHeight,contentHeight+topPadding+bottomPadding)
                    onActiveFocusChanged:root.editorFocus(this,activeFocus)
                    Keys.priority:Keys.BeforeItem
                    Keys.onPressed:function(event){if(event.key===Qt.Key_Tab||event.key===Qt.Key_Backtab){var next=this.nextItemInFocusChain(!(event.modifiers&Qt.ShiftModifier));if(next)next.forceActiveFocus();event.accepted=true}else if(event.key===Qt.Key_Escape){root.finishEditor(false);event.accepted=true}else if((event.key===Qt.Key_Return||event.key===Qt.Key_Enter)&&event.modifiers===Qt.ControlModifier){root.finishEditor(true);event.accepted=true}}
                    text:ideaCard.text;placeholderText:T3I18n.tr("Write an idea or a task list…");wrapMode:TextEdit.Wrap;selectByMouse:true
                    color:root.fg;font.pixelSize:Style.font.body;padding:Style.space(4);background:null
                    onTextChanged:if(activeFocus)root.updateIdea(ideaCard.key,text)
                    Component.onCompleted:if(visible)forceActiveFocus()
                    onVisibleChanged:if(visible)forceActiveFocus()
                    }
                  }
                  RowLayout {
                    Layout.fillWidth:true
                    T3PickerButton{objectName:"t3BoardConvertIdea_"+ideaCard.key;primary:false;outlined:true;implicitHeight:Style.space(28);pixelSize:Style.font.caption;text:T3I18n.tr("Create thread");enabled:!!ideaCard.text.trim();onClicked:root.ideaRequested(ideaCard.text)}
                    Item{Layout.fillWidth:true}
                    T3IconButton{name:ideaCard.editing?"check":"square-pen";tooltipText:T3I18n.tr(ideaCard.editing?"Done":"Edit");tint:root.dim;onClicked:root.editingIdea=ideaCard.editing?"":ideaCard.key}
                  }
                }
              }
              Column {
                visible:ideas.count===0;anchors{left:parent.left;right:parent.right;top:parent.top;topMargin:Style.space(30);margins:Style.space(12)}
spacing:Style.space(12)
                Text{width:parent.width;text:T3I18n.tr("Keep ideas here before choosing a project.");textFormat:Text.PlainText;wrapMode:Text.Wrap;color:root.dim;font.pixelSize:Style.font.body}
                T3PickerButton{primary:false;outlined:true;implicitHeight:Style.space(30);text:T3I18n.tr("Add idea");onClicked:root.addIdea("")}
              }
            }
          }
        }
        Repeater {
          id:sectionRepeater;model:root.columnSpecs
          Rectangle {
            id:column
            required property var modelData
            required property int index
            readonly property var rows:root.rows(modelData.key)
            readonly property var setting:root.columnSetting(modelData.key)
            readonly property color tint:T3Palette.stateColor(({input:"waiting",ready:"idle",pinned:"idle",settled:"done"})[modelData.key] || modelData.key)
            property alias view:list
            property alias menu:columnMenu
            property var header:setting.collapsed?foldedRail:columnHeader
            property var rowSync:({keys:[],prints:{}})
            function syncRows(){
              var previous=list.currentIndex
              ModelSync.sync(rowModel,rows,rowSync,function(row){return row.key},function(row){return {row:row}})
              if(root.navColumn===modelData.key){var i=rows.findIndex(function(t){return t.key===root.navKey});if(i<0&&rows.length)i=Math.min(Math.max(0,previous),rows.length-1);list.currentIndex=i;if(i>=0){root.navKey=rows[i].key;root.navIndex=i}}
            }
            onRowsChanged:syncRows()
            Component.onCompleted:syncRows()
            ListModel{id:rowModel;dynamicRoles:true}
            objectName:"t3BoardColumn_"+modelData.key
            border.width:0
            width:column.setting.collapsed?Style.space(44):columns.columnWidth;height:columns.height;radius:Style.space(12);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025)
            Controls.Menu {
              id:columnMenu;width:Style.space(280);
    palette.button:Qt.tint(Color.popups.background,Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06));palette.placeholderText:root.dim;palette.window:Color.popups.background;palette.windowText:root.fg;palette.text:root.fg;palette.buttonText:root.fg;palette.base:Color.popups.background;palette.highlight:Color.accent;palette.highlightedText:Color.popups.background
    background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:root.line}
              parent:column;x:0;y:Style.space(38)
              onOpened:root.columnMenuOpen=true
              onClosed:{root.columnMenuOpen=false;root.restoreFocus()}
              Controls.MenuItem{text:T3I18n.tr("Sort by");enabled:false}
              Repeater {
                model:root.sortOptions
                Controls.MenuItem{required property var modelData;objectName:"t3BoardSort_"+column.modelData.key+"_"+modelData.key;text:T3I18n.tr(modelData.label);checkable:true;checked:(column.setting.order || "sidebar")===modelData.key;onTriggered:root.configureColumn(column.modelData.key,{order:modelData.key})}
              }
              Controls.MenuItem{objectName:"t3BoardReverse_"+column.modelData.key;text:T3I18n.tr("Reverse order");checkable:true;checked:!!column.setting.reverse;onTriggered:root.configureColumn(column.modelData.key,{reverse:!column.setting.reverse})}
              Controls.MenuSeparator{}
              Controls.MenuItem{text:T3I18n.tr("Follow global live view");checkable:true;checked:typeof column.setting.live!=="boolean";onTriggered:root.configureColumn(column.modelData.key,{live:null})}
              Controls.MenuItem{text:T3I18n.tr("Expand live cards");checkable:true;checked:column.setting.live===true;onTriggered:root.configureColumn(column.modelData.key,{live:true})}
              Controls.MenuItem{text:T3I18n.tr("Compact cards");checkable:true;checked:column.setting.live===false;onTriggered:root.configureColumn(column.modelData.key,{live:false})}
              Controls.MenuSeparator{}
              Controls.MenuItem{text:T3I18n.tr(column.setting.collapsed?"Expand column":"Collapse column");onTriggered:root.configureColumn(column.modelData.key,{collapsed:!column.setting.collapsed})}
              Controls.MenuItem{visible:column.modelData.key==="settled"&&!!root.sidebarContext&&root.sidebarContext.olderSettledMissing>0;height:visible?implicitHeight:0;text:T3I18n.tr("Load older settled threads");onTriggered:root.loadOlder()}
            }

            Controls.AbstractButton {
              id:foldedRail;objectName:"t3BoardRail_"+column.modelData.key;anchors.fill:parent;visible:!!column.setting.collapsed
              activeFocusOnTab:activeFocus||root.navColumn===column.modelData.key&&!root.navKey
              Accessible.name:root.columnTitle(column.modelData)+" · "+T3I18n.tr("Expand column")
              Keys.onReturnPressed:function(event){event.accepted=true;foldedRail.clicked()}
              Keys.onEnterPressed:function(event){event.accepted=true;foldedRail.clicked()}
              Keys.onSpacePressed:function(event){event.accepted=true;foldedRail.clicked()}
              onClicked:{root.navColumn=column.modelData.key;root.configureColumn(column.modelData.key,{collapsed:false});Qt.callLater(root.restoreFocus)}
              background:Rectangle{radius:Style.space(10);color:foldedRail.hovered?root.line:"transparent";border.width:foldedRail.activeFocus?1:0;border.color:Color.accent}
              contentItem:Item {
                T3Icon{anchors{horizontalCenter:parent.horizontalCenter;top:parent.top;topMargin:Style.space(12)}
name:"chevron-right";tint:column.tint;implicitWidth:Style.space(14)}
                Text{objectName:"t3BoardRailTitle_"+column.modelData.key;anchors.centerIn:parent;width:parent.height-Style.space(80);rotation:90;text:root.columnTitle(column.modelData);color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}
              }
              TapHandler{acceptedButtons:Qt.RightButton;onTapped:{root.navColumn=column.modelData.key;columnMenu.open()}}
            }
            ColumnLayout {
              visible:!column.setting.collapsed
              anchors.fill:parent;anchors.margins:Style.space(10);spacing:Style.space(10)
              Item {
                id:columnHeader;objectName:"t3BoardColumnHeader_"+column.modelData.key
                activeFocusOnTab:activeFocus||root.navColumn===column.modelData.key&&!root.navKey
                Accessible.name:root.columnTitle(column.modelData)
                Layout.fillWidth:true;Layout.preferredHeight:Style.space(28)
                Rectangle{objectName:"t3BoardHeaderFocus_"+column.modelData.key;anchors.fill:parent;radius:Style.space(6);color:"transparent";border.width:columnHeader.activeFocus?1:0;border.color:Color.accent}
                RowLayout {
                anchors.fill:parent;anchors.leftMargin:Style.space(4);spacing:Style.space(7)
                Rectangle{width:Style.space(6);height:width;radius:width/2;color:column.tint}
                Text{Layout.fillWidth:true;text:root.columnTitle(column.modelData);color:root.fg;font.pixelSize:Style.font.caption;font.weight:Font.DemiBold;elide:Text.ElideRight;textFormat:Text.PlainText}
                Text{visible:column.modelData.label.indexOf("{count}")<0;text:String(root.columnCount(column.modelData.key));color:root.dim;font.pixelSize:Style.font.caption}
                T3IconButton{objectName:"t3BoardColumnMenu_"+column.modelData.key;name:column.setting.order&&column.setting.order!=="sidebar"||column.setting.reverse?"list-filter":"ellipsis-vertical";tint:column.setting.order&&column.setting.order!=="sidebar"||column.setting.reverse?Color.accent:root.dim;tooltipText:T3I18n.tr("Column options");onClicked:{root.navColumn=column.modelData.key;columnMenu.open()}}
                T3IconButton{objectName:"t3BoardColumnCollapse_"+column.modelData.key;name:column.setting.collapsed?"chevron-down":"chevron-up";tint:root.dim;tooltipText:T3I18n.tr(column.setting.collapsed?"Expand column":"Collapse column");onClicked:root.configureColumn(column.modelData.key,{collapsed:!column.setting.collapsed})}
                }
              }
              Item{visible:!!column.setting.collapsed;Layout.fillHeight:true;Layout.fillWidth:true}
              ListView {
                id:list;objectName:"t3BoardList_"+column.modelData.key;visible:!column.setting.collapsed;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;spacing:Style.space(8);model:rowModel;boundsBehavior:Flickable.StopAtBounds
                T3Wheel{flickable:list}
                Controls.ScrollBar.vertical:Controls.ScrollBar{}
                footer:T3PickerButton{objectName:"t3BoardOlderSettled";width:list.width;visible:column.modelData.key==="settled"&&!!root.sidebarContext&&root.sidebarContext.olderSettledMissing>0;height:visible?implicitHeight:0;primary:false;text:T3I18n.tr("Load older settled threads");onClicked:root.loadOlder()}
                Text{anchors{horizontalCenter:parent.horizontalCenter;top:parent.top;topMargin:Style.space(30)}
visible:!list.count;text:T3I18n.tr("No threads here");color:root.dim;font.pixelSize:Style.font.caption}
                delegate:Rectangle {
                  id:card
                  required property var row
                  required property int index
                  readonly property var modelData:row
                  objectName:"t3BoardThread_"+modelData.id
                  activeFocusOnTab:activeFocus||root.navColumn===column.modelData.key&&root.navKey===modelData.key
                  onActiveFocusChanged:if(activeFocus)root.rememberFocus(column.modelData.key,modelData.key,index)
                  Accessible.role:Accessible.Button
                  Accessible.name:[modelData.title,modelData.project,modelData.environmentName].filter(Boolean).join(" · ")
                  property alias sharedRow:sharedRow
                  signal clicked()
                  onClicked:root.threadRequested(modelData)
                  readonly property bool expanded:root.liveKeys.indexOf(modelData.key)>=0
                  readonly property bool watched:root.liveThreads.some(function(t){return t.key===card.modelData.key})
                  readonly property bool live:expanded||root.columnLive(column.modelData.key)
                  readonly property bool editing:root.editingKey===modelData.key
                  readonly property bool inViewport:!column.setting.collapsed&&card.y+card.height>list.contentY&&card.y<list.contentY+list.height&&column.x+column.width>viewport.contentX&&column.x<viewport.contentX+viewport.width
                  onInViewportChanged:root.watchVisible(modelData.key,inViewport)
                  Component.onCompleted:root.watchVisible(modelData.key,inViewport)
                  Component.onDestruction:root.watchVisible(modelData.key,false)
                  readonly property var snapshot:root.bridge.mosaicData[modelData.key] || {}
                  readonly property var messages:(snapshot.messages || []).filter(function(m){return m.role==="assistant"||m.role==="user"}).slice(-8)
                  width:ListView.view.width;height:cardBody.implicitHeight+Style.space(card.live?24:8)
                  color:card.live?Color.popups.background:card.activeFocus?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.035):"transparent";radius:Style.space(9);border.width:card.live||card.activeFocus?1:0;border.color:card.activeFocus?Color.accent:card.editing?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.25):root.line
                  ColumnLayout {
                    id:cardBody;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(4)}
                    spacing:Style.space(9)
                    Item {
                      Layout.fillWidth:true;Layout.preferredHeight:sharedRow.height
                      T3ThreadRow {
                        id:sharedRow;objectName:"t3BoardRow_"+card.modelData.id
                        thread:card.modelData;bridge:root.bridge;width:parent.width-Style.space(26)
                        keyboardCurrent:card.activeFocus;keyboardFrame:false
                        compactRows:root.sidebarContext?root.sidebarContext.compactRows:true
                        marquee:root.sidebarContext?root.sidebarContext.titleMarquee:false
                        projectWash:root.sidebarContext?root.sidebarContext.projectWash:true
                        sober:root.sidebarContext?root.sidebarContext.sober:true
                        projectTint:root.sidebarContext?(root.sidebarContext.groupTints[thread.projectKey] || "transparent"):"transparent"
                        hostStyles:root.bridge.viewState.hostStyles || ({});manyHosts:(root.bridge.state.environments || []).length>1
                        panelVisible:root.visible&&card.inViewport
                        stateColor:T3Palette.detailColor(Sidebar.detail(thread))
                        stateLabel:root.sidebarContext?root.sidebarContext.status(thread):T3I18n.tr(column.modelData.label)
                        timeLabel:root.sidebarContext?root.sidebarContext.threadTime(thread):""
                        providerGlyph:root.sidebarContext?root.sidebarContext.providerIcon(thread.provider):"acp"
                        sky:root.sky
                        effects:root.sidebarContext?root.sidebarContext.threadEffects:({working:"comet",input:"glints",change:"star",intensity:1})
                        stateChangedAt:root.sidebarContext?(root.sidebarContext.stateChangedAt[thread.key] || 0):0
                        onThreadActivated:Qt.callLater(function(){if(root)root.closed()})
                        onInteractionChanged:function(active){root.holdRow(thread.key,active)}
                        projectScoped:root.sidebarContext?!!root.sidebarContext.projectFilter&&(Array.isArray(root.sidebarContext.projectScope)?root.sidebarContext.projectScope.indexOf(thread.projectKey)>=0:root.sidebarContext.projectScope===thread.projectKey):false
                        onPullRequestsRequested:function(source,reference,multiple){root.pullRequestsRequested(source,reference,multiple)}
                        onFilterProjectRequested:function(key){root.filterProjectRequested(key)}
                        onProjectSettingsRequested:function(key){root.projectSettingsRequested(key)}
                        onNewThreadOnBranch:function(source){root.newThreadOnBranch(source)}
                        onProjectContextRequested:function(item,px,py){root.projectContextRequested(card.modelData,item,px,py)}
                        onProjectContextDismissed:root.projectContextDismissed()
                        onSubthreadsToggled:root.subthreadsToggled(card.modelData)
                      }
                      T3IconButton{objectName:"t3BoardExpand_"+card.modelData.id;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;name:card.expanded?"chevron-up":"maximize";tooltipText:T3I18n.tr("Live view");tint:root.dim;implicitWidth:Style.space(24);onClicked:root.toggleLive(card.modelData)}
                    }
                    Rectangle{visible:card.live;Layout.fillWidth:true;implicitHeight:1;color:root.line}
                    ListView {
                      id:transcript;objectName:"t3BoardMessages_"+card.modelData.id
                      visible:card.live;Layout.fillWidth:true;Layout.preferredHeight:Style.space(card.expanded?260:160);clip:true;spacing:Style.space(10);model:card.messages
                      property bool follow:true
                      onMovementEnded:follow=atYEnd
                      onContentHeightChanged:if(follow)Qt.callLater(function(){transcript.positionViewAtEnd()})
                      onCountChanged:if(follow)Qt.callLater(function(){transcript.positionViewAtEnd()})
                      Controls.ScrollBar.vertical:Controls.ScrollBar{}
                      T3Wheel{flickable:transcript;onUserScrolled:transcript.follow=false}
                      Text {anchors.fill:parent;visible:!transcript.count;text:card.snapshot.error?T3I18n.error(card.snapshot.error):T3I18n.tr(card.snapshot.loaded?"No messages yet":"Loading conversation…");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
                      delegate:Column {
                        required property var modelData
                        width:transcript.width;spacing:Style.space(4)
                        Text{visible:parent.modelData.role==="user";text:T3I18n.tr("You");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
                        TextEdit{width:parent.width;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;color:root.fg;selectionColor:Color.accent;font.pixelSize:Style.font.body;textFormat:parent.modelData.rendered&&root.renderer?TextEdit.RichText:TextEdit.PlainText;text:parent.modelData.rendered&&root.renderer?root.renderer(parent.modelData,width):String(parent.modelData.text || "");onLinkActivated:function(link){root.bridge.openReference(link)}}
                      }
                    }
                    RowLayout {
                      visible:card.live&&!card.editing;Layout.fillWidth:true
                      Text{Layout.fillWidth:true;text:root.columnTitle(column.modelData);color:column.tint;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
                      T3IconButton{objectName:"t3BoardWrite_"+card.modelData.id;name:"message-square";tooltipText:T3I18n.tr("Write message");tint:root.dim;onClicked:root.writeTo(card.modelData)}
                    }
                    Rectangle {
                      objectName:"t3BoardReplyFrame_"+card.modelData.id
                      visible:card.editing;Layout.fillWidth:true;implicitHeight:Style.space(104);radius:Style.space(7);border.width:1;border.color:reply.activeFocus?Color.accent:root.line;color:"transparent"
                      Controls.ScrollView {
                        id:replyScroll;contentWidth:availableWidth;clip:true
                        anchors{fill:parent;bottomMargin:Style.space(30);margins:Style.space(6)}
                        Controls.TextArea {
                        id:reply;objectName:"t3BoardReply_"+card.modelData.id;implicitWidth:0;implicitHeight:0;width:Math.max(1,replyScroll.availableWidth);height:Math.max(replyScroll.availableHeight,contentHeight+topPadding+bottomPadding)
                        enabled:card.editing&&root.bridge.selectedKey===card.modelData.key&&!root.bridge.draft.loading&&!root.bridge.draft.busy&&!root.bridge.draft.attempt&&!root.bridge.draft.loadError
                            onActiveFocusChanged:{root.editorFocus(this,activeFocus);root.holdRow("editor/"+card.modelData.key,activeFocus)}
                        Component.onDestruction:root.holdRow("editor/"+card.modelData.key,false)
                        Keys.priority:Keys.BeforeItem
                        Keys.onPressed:function(event){if(event.key===Qt.Key_Tab||event.key===Qt.Key_Backtab){var next=this.nextItemInFocusChain(!(event.modifiers&Qt.ShiftModifier));if(next)next.forceActiveFocus();event.accepted=true}else if(event.key===Qt.Key_Escape){root.finishEditor(false);event.accepted=true}else if((event.key===Qt.Key_Return||event.key===Qt.Key_Enter)&&event.modifiers===Qt.ControlModifier){root.finishEditor(true);event.accepted=true}}
                        text:root.bridge.selectedKey===card.modelData.key?root.bridge.draft.text:"";placeholderText:T3I18n.tr("Write message");color:root.fg;font.pixelSize:Style.font.body;wrapMode:TextEdit.Wrap;selectByMouse:true;background:null
                        onTextChanged:if(activeFocus&&enabled&&text!==root.bridge.draft.text)root.bridge.edit(text)
                        onEnabledChanged:if(enabled&&visible)forceActiveFocus()
                        onVisibleChanged:if(visible&&enabled)forceActiveFocus()
                        }
                      }
                      Row {
                        anchors{right:parent.right;bottom:parent.bottom;rightMargin:Style.space(3);bottomMargin:Style.space(3)}
                        T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");tint:root.dim;onClicked:root.editingKey=""}
                        T3IconButton{objectName:"t3BoardSend_"+card.modelData.id;name:"arrow-up";tooltipText:T3I18n.tr("Send");tint:Color.accent;enabled:reply.enabled&&!!reply.text.trim();onClicked:root.bridge.submit()}
                      }
                    }
                    Text{visible:card.editing&&root.bridge.selectedKey===card.modelData.key&&!!root.bridge.draft.error;Layout.fillWidth:true;text:visible?T3I18n.error(root.bridge.draft.error):"";color:T3Palette.failed;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
