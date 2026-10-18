import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/IconAssets.js" as Icons

Item {
  id: root
  required property var thread
  signal pullRequestsRequested(var source,var reference,bool multiple)
  signal threadActivated(var source)
  signal filterProjectRequested(string key)
  signal projectSettingsRequested(string key)
  signal newThreadOnBranch(var source)
  signal projectContextDismissed()
  signal projectContextRequested(var item,real px,real py)
  required property var bridge
  property bool panelVisible: true
  property bool pooled: false
  property bool keyboardCurrent: false
  property string stateLabel: ""
  property string timeLabel: ""
  property color stateColor: Color.accent
  // Panel sky (T3Starfield leader): shared clock and motion mode for the aura.
  property var sky: null
  // When the panel saw this thread change state; a recent change plays a
  // shooting star even if the row was just created in another section.
  property real stateChangedAt: 0
  readonly property bool needsInput: !!thread.pending && ["request","plan","waiting"].indexOf(thread.reason) >= 0
  readonly property bool skyMoving: !!sky && sky.animated
  // Styles chosen in Preferences; a still sky keeps every effect still.
  property var effects: ({working: "comet", input: "glints", change: "star", intensity: 1})
  readonly property int workingStyle: ["comet","pulse"].indexOf(effects.working)
  readonly property int inputStyle: !skyMoving && effects.input !== "off" ? 2 : ["glints","pulse","static"].indexOf(effects.input)
  readonly property int changeStyle: ["star","flash"].indexOf(effects.change)
  property string providerGlyph: "acp"
  property var catalog: bridge.catalogs[thread.environmentId] || ({})
  readonly property var projectInfo:(bridge.state.projects || []).find(function(p){return p.key===root.thread.projectKey}) || ({})
  readonly property bool scratchProject:!!thread.projectIsScratch || (!!catalog.scratchWorkspaceRoot&&String(projectInfo.workspaceRoot || "").replace(/[/\\]+$/,"")===String(catalog.scratchWorkspaceRoot).replace(/[/\\]+$/,""))
  readonly property bool showScratchMachine:!thread.branch&&scratchProject&&(bridge.state.environments || []).length>1
  readonly property var modelInfo: (catalog.models || []).find(function(m){return m.instanceId === root.thread.provider && m.model === root.thread.model}) || ({})
  readonly property var draft: bridge.drafts[thread.key] || ({})
  readonly property bool hasDraft: !draft.queueEdit && (!!String(draft.text || "").trim() || !!(draft.attachments && draft.attachments.length))
  readonly property bool showDraft:hasDraft&&bridge.selectedKey!==thread.key
  readonly property bool slim:!!thread.settled||!!thread.snoozed
  readonly property bool selected:bridge.selectedKey===thread.key
  readonly property bool receding:!selected&&!showActions&&((!!thread.working&&!(thread.pending&&["request","plan","waiting"].indexOf(thread.reason)>=0))||!!thread.settled)
  readonly property bool busy: !!bridge.managementBusy || !!bridge.managementRequest
  readonly property bool canOperate: catalog.canWrite === true
  readonly property var capabilities:catalog.capabilities || ({})
  readonly property bool canPin:canOperate&&capabilities.threadPinning===true
  readonly property bool canAutoSettle:canOperate&&capabilities.threadAutoSettleOptOut===true
  readonly property bool canRegenerate:canOperate&&capabilities.threadTitleRegeneration===true
  readonly property string workspacePath:thread.worktreePath || ((bridge.state.projects || []).find(function(p){return p.key===root.thread.projectKey}) || {}).workspaceRoot || ""
  property var contextThread:null
  readonly property var actionThread:contextThread || thread
  readonly property var actionCatalog:bridge.catalogs[actionThread.environmentId] || ({})
  readonly property var actionCaps:actionCatalog.capabilities || ({})
  readonly property bool actionCanOperate:actionCatalog.canWrite===true
  readonly property string actionPath:actionThread.worktreePath || ((bridge.state.projects || []).find(function(p){return p.key===root.actionThread.projectKey}) || {}).workspaceRoot || ""
  property bool snoozeContext:false
  property bool projectScoped:false
  property string editAction:"thread-rename"
  property string editRequest:""
  property string editError:""
  readonly property bool canSettle: canOperate && !!catalog.capabilities && catalog.capabilities.threadSettlement === true
  readonly property bool canSnooze: canOperate && !!catalog.capabilities && catalog.capabilities.threadSnooze === true && thread.canSnooze === true
  readonly property bool showActions: rowHover.hovered || keyboardCurrent || snoozeMenu.visible || rowMenu.visible || settleButton.activeFocus || snoozeButton.activeFocus || discardButton.activeFocus
  readonly property bool hovered: rowHover.hovered
  readonly property bool previewVisible: preview.visible
  readonly property var previewPopup: preview
  readonly property var snoozePopup: snoozeMenu
  readonly property var settleControl: settleButton
  readonly property var snoozeControl: snoozeButton
  readonly property var discardControl: discardButton
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r,fg.g,fg.b,0.57)
  height: Style.space(slim?36:82)
  Accessible.role: Accessible.Button
  Accessible.name: [thread.title,thread.project,stateLabel].filter(Boolean).join(" · ")
  function hidePreview(){openDelay.stop();closeDelay.stop();preview.close()}
  function dismiss(){if(threadPRBadge)threadPRBadge.closePreview();if(slimPRBadge)slimPRBadge.closePreview();projectContextDismissed();hidePreview();snoozeMenu.close();rowMenu.close();customSnooze.close()}
  function manage(action,extra){hidePreview();bridge.manage(Object.assign({action:action,environment:thread.environmentId,thread:thread.id},extra || {}))}
  function snoozeManage(action,extra){if(snoozeContext)contextManage(action,extra);else manage(action,extra)}
  function contextManage(action,extra){var t=actionThread;bridge.manage(Object.assign({action:action,environment:t.environmentId,thread:t.id},extra || {}))}
  function menuIcon(name){return "data:image/svg+xml,"+encodeURIComponent(Icons.svg(name).replace(/currentColor/g,String(root.fg)))}
  function copyValue(value){copyField.text=String(value || "");copyField.selectAll();copyField.copy();copyField.deselect()}
  function confirmAction(action){if(!contextThread)contextThread=Object.assign({},thread);editAction=action;editError="";editRequest="";threadTitle.text=actionThread.title || "";editPopup.open();if(action==="thread-rename")Qt.callLater(function(){threadTitle.forceActiveFocus();threadTitle.selectAll()})}
  function submitAction(){var extra=editAction==="thread-rename"?{title:threadTitle.text.trim()}:editAction==="delete-thread"?{confirmed:true}:{working:!!actionThread.working};contextManage(editAction,extra);editRequest=bridge.managementRequest?bridge.managementRequest.requestId:""}
  function presets(now) {
    var d=now || new Date(),h=3600000,result=[{label:"In 1 hour",date:new Date(d.getTime()+h)},{label:"In 3 hours",date:new Date(d.getTime()+3*h)}]
    var evening=new Date(d);evening.setHours(18,0,0,0);if(evening.getTime()-d.getTime()>h)result.push({label:"This evening",date:evening})
    var tomorrow=new Date(d);tomorrow.setDate(tomorrow.getDate()+1);tomorrow.setHours(9,0,0,0);result.push({label:"Tomorrow",date:tomorrow})
    var monday=new Date(d);monday.setDate(monday.getDate()+((1-d.getDay()+7)%7 || 7));monday.setHours(9,0,0,0);if(monday.getTime()!==tomorrow.getTime())result.push({label:"Next week",date:monday})
    return result
  }
  function openSnooze(context){snoozeContext=!!context;hidePreview();snoozeMenu.choices=presets();snoozeMenu.open()}
  onPanelVisibleChanged: if(!panelVisible)dismiss()
  onPooledChanged: if(pooled){dismiss();burstAnimation.stop();aura.burst=0}
  readonly property int burstMs: 1300
  function playBurst() {
    var age = Date.now() - stateChangedAt
    if (!stateChangedAt || !skyMoving || changeStyle < 0 || pooled || age < 0 || age > burstMs - 200) return
    burstAnimation.stop(); burstAnimation.from = age / burstMs; burstAnimation.duration = burstMs - age; burstAnimation.start()
  }
  onStateChangedAtChanged: playBurst()
  Component.onCompleted: playBurst()
  property string identity: thread.key || ""
  onIdentityChanged: dismiss()
  Component.onDestruction: dismiss()
  Connections {
    target: root.ListView.view
    function onContentYChanged(){root.dismiss()}
    function onMovingChanged(){if(root.ListView.view.moving)root.dismiss()}
  }
  HoverHandler {
    id: rowHover
    onHoveredChanged: {
      if(hovered && root.panelVisible && !root.pooled){closeDelay.stop();root.bridge.ensureCatalog(root.thread.environmentId);openDelay.restart()}
      else {openDelay.stop();closeDelay.restart()}
    }
  }
  Timer { id:openDelay;interval:500;onTriggered:if(root.hovered && !actionsHover.hovered && !snoozeMenu.visible && !rowMenu.visible && root.panelVisible && !root.pooled)preview.open() }
  Timer { id:closeDelay;interval:140;onTriggered:if(!root.hovered && !previewHover.hovered)preview.close() }
  Rectangle {
    anchors.fill:parent;anchors.topMargin:Style.space(root.slim?0:2);anchors.bottomMargin:Style.space(root.slim?0:2)
    radius:Style.space(6)
    color: root.selected ? Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.075) : root.showActions ? Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.045) : root.showDraft?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.06):"transparent"
    border.width:root.keyboardCurrent?1:0;border.color:Color.accent
    Behavior on color {ColorAnimation{duration:130}}
    ShaderEffect {
      id: aura
      objectName: "threadAura"
      anchors.fill: parent
      property real burst: 0
      readonly property bool bursting: burst > 0 && burst < 1
      property real working: !root.slim && !root.needsInput && !!root.thread.working && root.skyMoving && root.workingStyle >= 0 ? 1 : 0
      property real attention: !root.slim && root.needsInput && root.inputStyle >= 0 ? 1 : 0
      property real intensity: root.effects.intensity || 1
      property real workingStyle: Math.max(0, root.workingStyle)
      property real attentionStyle: Math.max(0, root.inputStyle)
      property real burstStyle: Math.max(0, root.changeStyle)
      Behavior on working { NumberAnimation { duration: 600 } }
      Behavior on attention { NumberAnimation { duration: 600 } }
      visible: root.panelVisible && !root.pooled && (working > 0 || attention > 0 || bursting)
      // Only visible auras read the clock, so idle rows add no bindings work.
      property real clock: visible && root.sky ? root.sky.clock : 1.2
      property real radius: parent.radius
      // Per-thread phase so neighbouring comets never orbit in lockstep.
      property real seed: { var h = 0; for (var i = 0; i < root.identity.length; i++) h = (h * 31 + root.identity.charCodeAt(i)) % 9973; return h / 9973 }
      property vector2d size: Qt.vector2d(width, height)
      property color tone: T3Palette.working
      property color alert: T3Palette.waiting
      property color flash: root.stateColor
      blending: true
      fragmentShader: Qt.resolvedUrl("../assets/shaders/threadaura.frag.qsb")
      NumberAnimation { id: burstAnimation; target: aura; property: "burst"; to: 1 }
    }
    MouseArea {
      anchors.fill:parent;cursorShape:Qt.PointingHandCursor;acceptedButtons:Qt.LeftButton|Qt.RightButton
      onClicked:function(mouse){root.hidePreview();if(mouse.button===Qt.RightButton){root.contextThread=Object.assign({},root.thread);root.bridge.ensureCatalog(root.thread.environmentId);rowMenu.popup();}else {root.threadActivated(root.thread);root.bridge.selectThread(root.thread.environmentId,root.thread.id)}}
    }
    ColumnLayout {
      anchors.fill:parent;anchors.leftMargin:Style.space(10);anchors.rightMargin:Style.space(10);anchors.topMargin:Style.space(root.slim?0:8);anchors.bottomMargin:Style.space(root.slim?0:8);spacing:Style.space(2)
      opacity:root.receding&&root.thread.working?0.7:1
      RowLayout {
        Layout.fillWidth:true;Layout.preferredHeight:Style.space(20);spacing:Style.space(6)
        T3Icon {name:"square-pen";implicitWidth:Style.space(13);tint:root.dim;visible:root.showDraft}
        T3ProjectIcon {
          project:root.projectInfo;projects:root.bridge.state.projects || [];monogram:root.thread.projectMonogram || "PR";projectKey:root.thread.projectKey || ""
          width:Style.space(16);height:width;radius:Style.space(4)
          opacity:root.slim&&!root.showActions?0.4:1
          MouseArea{objectName:"projectIconContext";anchors.fill:parent;acceptedButtons:Qt.RightButton;onClicked:function(mouse){root.hidePreview();root.projectContextRequested(parent,mouse.x,mouse.y)}}
        }
        Text{objectName:"threadProjectOrTitle";Layout.fillWidth:true;text:root.slim?root.thread.title || "":root.thread.project || root.thread.environmentName || "";color:root.slim?(root.receding?root.dim:root.fg):root.dim;font.pixelSize:root.slim?Style.font.body:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText
          activeFocusOnTab:!root.slim
          Keys.onPressed:function(event){if(event.key===Qt.Key_Menu || (event.key===Qt.Key_F10&&event.modifiers===Qt.ShiftModifier)){root.hidePreview();root.projectContextRequested(this,width/2,height/2);event.accepted=true}}
          MouseArea{objectName:"projectNameContext";anchors.fill:parent;enabled:!root.slim;acceptedButtons:Qt.RightButton;onClicked:function(mouse){root.hidePreview();root.projectContextRequested(parent,mouse.x,mouse.y)}}
        }
        T3IconButton{objectName:"threadPinIndicator";visible:!!root.thread.pinned;name:root.canPin&&root.hovered?"pin-off":"pin";implicitWidth:Style.space(18);implicitHeight:Style.space(18);tooltipText:T3I18n.tr("Unpin thread");opacity:root.canPin?1:0.65;enabled:root.canPin&&!root.busy;onClicked:root.manage("unpin")}
        T3PullRequestBadge{id:slimPRBadge;objectName:root.slim?"threadPullRequestBadge":"slimThreadPullRequestBadge";active:root.slim&&root.panelVisible&&!root.pooled;badge:root.thread.pullRequestBadge || null;visible:root.slim&&!!badge;onActivated:function(reference,multiple){root.hidePreview();root.pullRequestsRequested(root.thread,reference,multiple)};onPreviewOpened:root.hidePreview()}
        RowLayout {
          visible:!root.showActions || (!root.canSettle && !root.canSnooze && !root.showDraft);spacing:Style.space(4)
          T3Icon{name:root.thread.pending&&["request","plan","waiting"].indexOf(root.thread.reason)>=0?"message-circle-question":root.thread.working?"circle-dashed":root.stateLabel===T3I18n.tr("Done")?"circle-check":"circle-alert";visible:!root.slim&&!!root.stateLabel;implicitWidth:Style.space(16);tint:root.stateColor}
          Text{text:!root.slim&&root.stateLabel ? root.stateLabel+(root.stateLabel===T3I18n.tr("Working") && root.timeLabel ? " "+root.timeLabel : "") : root.timeLabel;color:!root.slim&&root.stateLabel ? root.stateColor : root.dim;font.pixelSize:Style.font.caption}
        }
        RowLayout {
          visible:root.showActions;spacing:0
          HoverHandler{id:actionsHover;onHoveredChanged:if(hovered)root.hidePreview();else if(root.hovered)openDelay.restart()}
          T3IconButton{id:discardButton;objectName:"threadDiscard";name:"x";tooltipText:T3I18n.tr("Discard draft");visible:root.showDraft;enabled:!root.draft.busy&&!root.draft.loading&&!root.draft.attempt&&!root.draft.loadError;implicitWidth:Style.space(22);implicitHeight:Style.space(20);onClicked:root.bridge.discardDraft(root.thread.key)}
          T3IconButton{id:snoozeButton;objectName:"threadSnooze";name:root.thread.snoozed?"alarm-clock-off":"clock";tooltipText:T3I18n.tr(root.thread.snoozed?"Wake thread now":"Snooze thread");visible:root.canSnooze||!!root.thread.snoozed&&root.canOperate;enabled:!root.busy;implicitWidth:Style.space(22);implicitHeight:Style.space(20);onClicked:if(root.thread.snoozed)root.manage("unsnooze");else root.openSnooze()}
          Controls.Button {
            id:settleButton;objectName:"threadSettle";visible:root.canSettle&&!root.thread.snoozed;enabled:!root.busy
            implicitHeight:Style.space(20);implicitWidth:root.thread.settled?Style.space(24):settleContent.implicitWidth+Style.space(8)
            padding:0;leftPadding:Style.space(4);rightPadding:Style.space(4)
            Accessible.name:T3I18n.tr(root.thread.settled?"Move to active":"Mark settled")
            background:Rectangle{radius:Style.space(4);color:settleButton.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07):"transparent";border.width:settleButton.activeFocus?1:0;border.color:Color.accent}
            contentItem:RowLayout{id:settleContent;spacing:Style.space(3);T3Icon{name:root.thread.settled?"undo-2":"check";implicitWidth:Style.space(13);tint:root.dim}Text{visible:!root.thread.settled;text:T3I18n.tr("Settle");color:root.dim;font.pixelSize:Style.font.caption}}
            Controls.ToolTip.visible:hovered;Controls.ToolTip.delay:500;Controls.ToolTip.text:Accessible.name
            onClicked:root.manage(root.thread.settled?"unsettle":"settle")
          }
        }
      }
      RowLayout{
        visible:!root.slim
        Layout.fillWidth:true;Layout.topMargin:Style.space(2);spacing:Style.space(6)
        Text{objectName:"threadCardTitle";Layout.fillWidth:true;text:root.thread.title || "";color:root.receding?root.dim:root.fg;font.weight:root.receding?Font.Normal:Font.Medium;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
        // Under "Done": how long ago the thread finished, muted.
        Text{objectName:"threadDoneAge";visible:root.stateLabel===T3I18n.tr("Done")&&!!root.timeLabel;text:root.timeLabel;color:root.dim;font.pixelSize:Style.font.caption}
      }
      RowLayout{
        visible:!root.slim
        Layout.fillWidth:true;spacing:Style.space(6)
        T3Icon{objectName:"threadWorkspaceIcon";name:root.thread.branch?"folder-git-2":"server";visible:root.thread.branch?!!root.thread.worktreePath:root.showScratchMachine;tint:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.4);implicitWidth:Style.space(12)}
        Text{objectName:"threadBranch";Layout.fillWidth:true;text:root.thread.branch||(root.showScratchMachine?root.thread.environmentName || "":"");color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.4);font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
        T3PullRequestBadge{id:threadPRBadge;active:!root.slim&&root.panelVisible&&!root.pooled;objectName:root.slim?"cardThreadPullRequestBadge":"threadPullRequestBadge";badge:root.thread.pullRequestBadge || null;visible:!!badge;onActivated:function(reference,multiple){root.hidePreview();root.pullRequestsRequested(root.thread,reference,multiple)};onPreviewOpened:root.hidePreview()}
        T3Icon{objectName:"threadRemoteIcon";name:"server";tint:root.dim;implicitWidth:Style.space(12);visible:root.thread.environmentId!=="local"&&!root.showScratchMachine}
        T3ProviderIcon{provider:root.modelInfo.instanceId?root.modelInfo:({driverKind:root.thread.provider,registryIcon:""});tint:root.dim;Layout.preferredWidth:Style.space(12);Layout.preferredHeight:Style.space(12)}
      }
    }
  }
  Controls.Popup {
    id:preview;objectName:"threadHoverCard";x:root.width+Style.space(4);y:Style.space(2);width:Style.space(320);padding:Style.space(12);focus:false;closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
    background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}
    contentItem:ColumnLayout{
      spacing:Style.space(6)
      HoverHandler{id:previewHover;onHoveredChanged:if(hovered)closeDelay.stop();else closeDelay.restart()}
      Text{Layout.fillWidth:true;Layout.bottomMargin:Style.space(2);text:root.thread.title||"";color:root.fg;font.pixelSize:Style.font.caption;font.weight:Font.Medium;elide:Text.ElideRight;textFormat:Text.PlainText}
      Repeater{
        model:[{icon:"folder",label:root.thread.project},{icon:"server",label:root.thread.environmentName},{icon:"git-branch",label:root.thread.branch},{icon:root.providerGlyph,label:root.modelInfo.displayName||root.thread.model},{icon:"arrow-right-left",label:(root.thread.providerHistory||[]).filter(function(p){return p!==root.thread.provider}).map(function(p){var provider=(root.catalog.providers||[]).find(function(v){return v.instanceId===p});return provider?provider.name:p}).join(", ")},{icon:"circle-alert",label:root.thread.hasRuntimeError?T3I18n.tr(root.thread.runtimeErrorClass==="usage_limit"?"Usage limit reached":"Error occurred"):""}].filter(function(m){return !!m.label})
        delegate:RowLayout{
          required property var modelData
          Layout.fillWidth:true;spacing:Style.space(8)
          T3Icon{name:modelData.icon;tint:modelData.icon==="circle-alert"?Color.urgent:root.dim;implicitWidth:Style.space(12)}
          Text{Layout.fillWidth:true;text:modelData.label;color:modelData.icon==="circle-alert"?Color.urgent:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
        }
      }
    }
  }
  Controls.Menu {
    width:Style.space(280)
    id:snoozeMenu;property var choices:[];objectName:"threadSnoozeMenu"
    x:Math.max(0,root.width-width);y:Style.space(24)
    palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
    background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:root.dim}
    Instantiator{
      model:snoozeMenu.choices
      delegate:Controls.MenuItem{required property var modelData;text:T3I18n.tr(modelData.label)+"    "+Qt.formatTime(modelData.date,"hh:mm");enabled:!root.busy&&(root.snoozeContext?root.actionCanOperate&&root.actionCaps.threadSnooze===true&&root.actionThread.canSnooze===true:root.canSnooze);onTriggered:root.snoozeManage("snooze",{snoozedUntil:modelData.date.toISOString()})}
      onObjectAdded:function(index,object){snoozeMenu.insertItem(index,object)}
      onObjectRemoved:function(index,object){snoozeMenu.removeItem(object)}
    }
    Controls.MenuSeparator{}
    Controls.MenuItem{text:T3I18n.tr("Custom…");onTriggered:{customDate.text=Qt.formatDateTime(new Date(Date.now()+3600000),"yyyy-MM-ddTHH:mm");customSnooze.open()}}
  }
  Controls.Popup {
    id:customSnooze;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Style.space(320);padding:Style.space(16);modal:true
    background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:Color.accent}
    contentItem:ColumnLayout{
      spacing:Style.space(12)
      Text{text:T3I18n.tr("Snooze thread");color:root.fg;font.pixelSize:Style.font.body}
      Controls.TextField{id:customDate;Layout.fillWidth:true;placeholderText:"YYYY-MM-DDTHH:mm";color:root.fg;selectByMouse:true;Accessible.name:T3I18n.tr("Wake date and time")}
      Controls.Button{text:T3I18n.tr("Snooze thread");Layout.alignment:Qt.AlignRight;enabled:(root.snoozeContext?root.actionCanOperate&&root.actionCaps.threadSnooze===true&&root.actionThread.canSnooze===true:root.canSnooze)&&!root.busy&&isFinite(Date.parse(customDate.text))&&Date.parse(customDate.text)>Date.now();onClicked:{root.snoozeManage("snooze",{snoozedUntil:new Date(customDate.text).toISOString()});customSnooze.close()}}
    }
  }
  Controls.Menu {
    width:Style.space(280)
    id:autoSettleMenu
      title:T3I18n.tr("Auto-settle behavior");enabled:(root.actionCanOperate&&root.actionCaps.threadAutoSettleOptOut===true)&&!root.busy
      palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
      background:Rectangle{color:Color.popups.background;border.color:root.dim;border.width:1;radius:Style.space(7)}
      Controls.MenuItem{text:T3I18n.tr("Enabled");checkable:true;checked:!root.actionThread.autoSettleDisabledAt;onTriggered:root.contextManage("auto-settle",{enabled:true})}
      Controls.MenuItem{text:T3I18n.tr("Disabled");checkable:true;checked:!!root.actionThread.autoSettleDisabledAt;onTriggered:root.contextManage("auto-settle",{enabled:false})}
    }
  TextEdit{id:copyField;visible:false;textFormat:TextEdit.PlainText}
  Controls.Menu {
    width:Style.space(280)
    id:rowMenu;property bool autoMenuInserted:false
    onAboutToShow:{if(root.actionCaps.threadAutoSettleOptOut===true&&!autoMenuInserted){insertMenu(count-5,autoSettleMenu);autoMenuInserted=true}else if(root.actionCaps.threadAutoSettleOptOut!==true&&autoMenuInserted){removeMenu(autoSettleMenu);autoMenuInserted=false}}
    objectName:"t3ThreadContextMenu"
    palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
    background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:root.dim}
    Controls.MenuItem{objectName:"threadNewOnBranch";icon.source:root.menuIcon("square-pen");icon.color:root.dim;text:T3I18n.tr("New thread on {branch}",{branch:root.actionThread.branch || ""});visible:!!root.actionThread.branch;height:visible?implicitHeight:0;enabled:root.actionCanOperate&&!root.busy;onTriggered:root.newThreadOnBranch(root.actionThread)}
    Controls.MenuItem{objectName:"threadPin";icon.source:root.menuIcon("pin");icon.color:root.dim;text:T3I18n.tr(root.actionThread.pinned?"Unpin thread":"Pin thread");visible:root.actionCaps.threadPinning===true;height:visible?implicitHeight:0;enabled:(root.actionCanOperate&&root.actionCaps.threadPinning===true)&&!root.busy;onTriggered:root.contextManage(root.actionThread.pinned?"unpin":"pin")}
    Controls.MenuItem{text:T3I18n.tr(root.actionThread.settled?"Move to active":"Mark settled");visible:root.actionCaps.threadSettlement===true;height:visible?implicitHeight:0;enabled:(root.actionCanOperate&&root.actionCaps.threadSettlement===true)&&!root.busy;onTriggered:root.contextManage(root.actionThread.settled?"unsettle":"settle")}
    Controls.MenuItem{text:T3I18n.tr(root.actionThread.snoozed?"Wake thread now":"Snooze thread");visible:root.actionCaps.threadSnooze===true;height:visible?implicitHeight:0;enabled:root.actionCanOperate&&!root.busy&&((root.actionCanOperate&&root.actionCaps.threadSnooze===true&&root.actionThread.canSnooze===true)||!!root.actionThread.snoozed);onTriggered:if(root.actionThread.snoozed)root.contextManage("unsnooze");else root.openSnooze(true)}
    Controls.MenuSeparator{visible:!!root.actionThread.branch||root.actionCaps.threadPinning===true||root.actionCaps.threadSettlement===true||root.actionCaps.threadSnooze===true;height:visible?implicitHeight:0}
    Controls.MenuItem{objectName:"threadRename";icon.source:root.menuIcon("square-pen");icon.color:root.dim;text:T3I18n.tr("Rename thread");enabled:root.actionCanOperate&&!root.busy;onTriggered:root.confirmAction("thread-rename")}
    Controls.MenuItem{objectName:"threadRegenerate";icon.source:root.menuIcon("refresh-cw");icon.color:root.dim;text:T3I18n.tr(root.actionThread.titleRegeneration?"Regenerating…":"Regenerate title");visible:root.actionCaps.threadTitleRegeneration===true;height:visible?implicitHeight:0;enabled:(root.actionCanOperate&&root.actionCaps.threadTitleRegeneration===true)&&!root.busy&&!root.actionThread.titleRegeneration;onTriggered:root.contextManage("regenerate-title")}
    Controls.MenuItem{objectName:"threadMarkUnread";icon.source:root.menuIcon("mail-open");icon.color:root.dim;text:T3I18n.tr("Mark unread");enabled:root.actionCanOperate&&!root.busy;onTriggered:root.contextManage("mark-unread")}
    Controls.MenuItem{objectName:"threadFilterProject";icon.source:root.menuIcon("folder");icon.color:root.dim;text:T3I18n.tr(root.projectScoped?"All projects":"Filter by {project}",{project:root.actionThread.project || ""});visible:!!root.actionThread.projectKey;height:visible?implicitHeight:0;onTriggered:root.filterProjectRequested(root.actionThread.projectKey)}
    Controls.MenuSeparator{}
    Controls.Menu {
    width:Style.space(280)
      title:T3I18n.tr("Copy")
      palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
      background:Rectangle{color:Color.popups.background;border.color:root.dim;border.width:1;radius:Style.space(7)}
      Controls.MenuItem{objectName:"threadCopyPath";icon.source:root.menuIcon("folder");icon.color:root.dim;text:T3I18n.tr("Path");enabled:!!root.actionPath;onTriggered:root.copyValue(root.actionPath)}
      Controls.MenuItem{text:T3I18n.tr("Branch");visible:!!root.actionThread.branch;height:visible?implicitHeight:0;onTriggered:root.copyValue(root.actionThread.branch)}
      Controls.MenuItem{text:T3I18n.tr("Thread ID");onTriggered:root.copyValue(root.actionThread.id)}
    }
    Controls.MenuItem{text:T3I18n.tr("Project settings");enabled:!!root.actionThread.projectKey;onTriggered:root.projectSettingsRequested(root.actionThread.projectKey)}
    Controls.MenuSeparator{}
    Controls.MenuItem{objectName:"threadArchive";icon.source:root.menuIcon("archive");icon.color:root.dim;text:T3I18n.tr("Archive thread");enabled:root.actionCanOperate&&!root.busy&&!root.actionThread.working&&!root.actionThread.activeRunId;onTriggered:root.confirmAction("archive")}
    Controls.MenuItem{objectName:"threadDelete";icon.source:root.menuIcon("trash-2");icon.color:Color.urgent;text:T3I18n.tr("Delete");enabled:root.actionCanOperate&&!root.busy;palette.text:Color.urgent;palette.windowText:Color.urgent;onTriggered:root.confirmAction("delete-thread")}
  }
  Controls.Popup {
    id:editPopup;objectName:"t3ThreadActionDialog";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Style.space(430);padding:Style.space(18);modal:true;focus:true
    background:Rectangle{color:Color.popups.background;radius:Style.space(10);border.width:1;border.color:root.editAction==="delete-thread"?Color.urgent:Color.accent}
    contentItem:ColumnLayout{
      spacing:Style.space(12)
      Text{Layout.fillWidth:true;text:T3I18n.tr(root.editAction==="thread-rename"?"Rename thread":root.editAction==="archive"?"Archive thread":"Delete");color:root.fg;font.pixelSize:Style.font.body;font.bold:true}
      Text{Layout.fillWidth:true;text:root.actionThread.title || "";color:root.dim;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      Controls.TextField{id:threadTitle;objectName:"threadRenameInput";Layout.fillWidth:true;visible:root.editAction==="thread-rename";color:root.fg;selectByMouse:true;enabled:!root.busy;background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06)} onAccepted:if(text.trim())root.submitAction()}
      Text{Layout.fillWidth:true;visible:root.editAction==="delete-thread";text:T3I18n.tr("This permanently clears conversation history for this thread.")+"\n"+T3I18n.tr("This action cannot be undone.");color:Color.urgent;wrapMode:Text.Wrap}
      Text{Layout.fillWidth:true;visible:!!root.editError;text:T3I18n.error(root.editError);color:Color.urgent;wrapMode:Text.Wrap}
      RowLayout{
        Item{Layout.fillWidth:true}
        Controls.Button{text:T3I18n.tr("Cancel");enabled:!root.bridge.managementBusy;onClicked:editPopup.close()}
        Controls.Button{objectName:"threadActionSubmit";text:T3I18n.tr(root.bridge.managementRequest?"Retry":root.editAction==="thread-rename"?"Save":root.editAction==="archive"?"Archive thread":"Delete");enabled:root.actionCanOperate&&!root.bridge.managementBusy&&(root.editAction!=="thread-rename"||!!threadTitle.text.trim());palette.buttonText:root.editAction==="delete-thread"?Color.urgent:root.fg;onClicked:if(root.bridge.managementRequest)root.bridge.manage({});else root.submitAction()}
      }
    }
  }
  Connections{target:root.bridge;function onManagementFinished(data){if(!root.editRequest||data.requestId!==root.editRequest)return;root.editError=data.error || "";if(!data.error)editPopup.close()}}
}
