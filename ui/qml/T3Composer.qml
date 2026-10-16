import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import QtQuick.Dialogs
import qs.Commons
import qs.Ui

Rectangle {
  id: root
  property alias inputField: editor
  required property var bridge
  property bool showOptions: false
  property bool expanded: false
  signal toggleExpanded()
  property string lastDraftText:""
  property string pendingCursorKey:bridge.selectedKey
  function positionDraftAtEnd(){if(pendingCursorKey!==bridge.selectedKey||draft.loading)return;var key=pendingCursorKey;Qt.callLater(function(){if(key!==root.bridge.selectedKey||root.draft.loading)return;editor.cursorPosition=editor.text.length;root.bridge.rememberDraftCursor(editor.cursorPosition,editor.cursorPosition);root.pendingCursorKey=""})}
  Component.onCompleted:positionDraftAtEnd()
  Connections{target:root.bridge;function onSelectedKeyChanged(){root.pendingCursorKey=root.bridge.selectedKey;root.positionDraftAtEnd()}function onDraftRestored(key,start,end){if(key!==root.bridge.selectedKey)return;Qt.callLater(function(){if(key!==root.bridge.selectedKey)return;editor.select(start,end);editor.forceActiveFocus()})}}
  onDraftChanged:{positionDraftAtEnd();if(!draft.loading && draft.text!==lastDraftText && editor.activeFocus)saveQuietPeriod.restart();lastDraftText=draft.text}
  Timer{id:saveQuietPeriod;interval:1000}

  Connections{target:root.bridge;function onClipboardInserted(key,cursor){if(key===root.bridge.selectedKey)Qt.callLater(function(){if(key!==root.bridge.selectedKey)return;editor.cursorPosition=cursor;editor.forceActiveFocus()})}}
  property int historyIndex: -1
  property string historyScratch: ""
  readonly property var promptHistory: bridge.transcript.filter(function(m){return m.role === "user"}).map(function(m){return String(m.text || "").replace(/\n*<t3_context\b[\s\S]*<\/t3_context>\s*(?:\[Attached[\s\S]*)?$/,"").trim()}).filter(function(t){return !!t}).reverse()
  Connections { target: root.bridge; function onThreadWillChange(){root.historyIndex=-1;root.historyScratch="";saveQuietPeriod.stop()} }
  property var completion: null
  property int completionIndex: 0
  property string dismissedCompletion: ""
  readonly property bool completionVisible: !!completion && editor.activeFocus && !locked
  function closeCompletion() { dismissedCompletion=editor.text+"/"+editor.cursorPosition;completion=null;bridge.dismissSuggestions() }
  function updateCompletion() {
    if (!editor.activeFocus || locked || dismissedCompletion===editor.text+"/"+editor.cursorPosition) return
    var prefix=editor.text.slice(0,editor.cursorPosition)
    var match=/(?:^|\s)([@$])([^\s]*)$/.exec(prefix)
    var slash=/(?:^|\n)\/([^\s]*)$/.exec(prefix)
    var next=match ? {kind:match[1]==="@"?"path":"skill",query:match[2],start:prefix.length-match[2].length-1,end:editor.cursorPosition} : slash ? {kind:"command",query:slash[1],start:prefix.length-slash[1].length-1,end:editor.cursorPosition} : null
    if(!next){completion=null;bridge.dismissSuggestions();return}
    if(completion && completion.kind===next.kind && completion.query===next.query && completion.start===next.start)return
    completion=next;completionIndex=0
    bridge.requestSuggestions(next.kind,next.query,selection.instanceId)
  }
  function chooseCompletion(index) {
    if(!completion || !bridge.suggestionItems[index])return false
    var item=bridge.suggestionItems[index];var before=editor.text.slice(0,completion.start);var after=editor.text.slice(completion.end)
    var insert=item.insert || "";bridge.edit(before+insert+after);var cursor=before.length+insert.length
    closeCompletion();Qt.callLater(function(){editor.cursorPosition=cursor;dismissedCompletion=editor.text+"/"+cursor;editor.forceActiveFocus();if(item.kind==="model"){root.showOptions=true;modelPicker.forceActiveFocus();modelPicker.popup.open()}});return true
  }
  function completionKey(event,step) {
    if(!completionVisible || event.modifiers!==Qt.NoModifier)return false
    var count=bridge.suggestionItems.length
    if(count)completionIndex=(completionIndex+step+count)%count
    return true
  }
  Timer { id: completionDelay; interval:140; onTriggered:root.updateCompletion() }
  Connections { target:root.bridge; function onThreadWillChange(){root.completion=null;root.dismissedCompletion=""} }
  Controls.Popup {
    id:completionPopup
    objectName:"t3CompletionPopup"
    visible:root.completionVisible
    parent:root
    x:Style.space(12);y:root.expanded ? Math.max(0,Math.min(root.height-height-Style.space(48),editor.mapToItem(root,0,0).y+editor.cursorRectangle.y+editor.cursorRectangle.height+Style.space(6))) : -height-Style.space(6)
    width:Math.max(100,root.width-Style.space(24));height:Math.min(Style.space(230),Math.max(Style.space(52),completionList.contentHeight+Style.space(12)))
    padding:Style.space(6);focus:false;closePolicy:Controls.Popup.NoAutoClose
    background:Rectangle{radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16)}
    contentItem:ListView{
      T3Wheel {flickable:completionList}
      id:completionList;clip:true;model:root.bridge.suggestionItems;currentIndex:root.completionIndex
      onCurrentIndexChanged:positionViewAtIndex(currentIndex,ListView.Contain)
      delegate:Rectangle{
        required property var modelData;required property int index
        width:completionList.width;height:Style.space(46);radius:Style.space(6)
        color:index===root.completionIndex?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.16):"transparent"
        RowLayout{anchors.fill:parent;anchors.margins:Style.space(6);spacing:Style.space(8)
          T3Icon{name:modelData.icon;tint:Color.accent}
          ColumnLayout{Layout.fillWidth:true;spacing:0
            Text{Layout.fillWidth:true;text:modelData.label;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideMiddle;textFormat:Text.PlainText}
            Text{Layout.fillWidth:true;text:modelData.description || "";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
          }
        }
        MouseArea{anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onPositionChanged:root.completionIndex=index;onClicked:root.chooseCompletion(index)}
      }
      Text{anchors.fill:parent;verticalAlignment:Text.AlignVCenter;visible:!completionList.count;text:root.bridge.suggestionsLoading?T3I18n.tr("Searching…"):T3I18n.error(root.bridge.suggestionsError) || T3I18n.tr("No results");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    }
  }
  function navigateHistory(direction) {
    if (locked || draft.queueEdit || editor.selectionStart !== editor.selectionEnd) return false
    var edge=editor.positionToRectangle(direction < 0 ? 0 : editor.text.length)
    if (Math.abs(editor.cursorRectangle.y-edge.y)>1) return false
    if (direction < 0) {
      if (historyIndex+1 >= promptHistory.length) return false
      if (historyIndex < 0) historyScratch=editor.text
      historyIndex++;bridge.edit(promptHistory[historyIndex])
    } else {
      if (historyIndex < 0) return false
      historyIndex--;bridge.edit(historyIndex < 0 ? historyScratch : promptHistory[historyIndex])
    }
    Qt.callLater(function(){editor.cursorPosition=direction<0?0:editor.text.length});return true
  }
  function continueList() {
    if(locked || editor.selectionStart!==editor.selectionEnd)return false
    var cursor=editor.cursorPosition;var before=editor.text.slice(0,cursor);var start=before.lastIndexOf("\n")+1
    var line=before.slice(start);var match=/^([ \t]*)([-+*•◦‣–]|\d+[.)])\s+(\[[ xX]\]\s+)?(.*)$/.exec(line)
    if(!match)return false
    var suffix=editor.text.slice(cursor);var insert="";var next=cursor
    if(!match[4].trim() && !suffix.split("\n")[0].trim()) {before=before.slice(0,start);next=start}
    else {var marker=match[2];if(/^\d/.test(marker))marker=String(Number(marker.slice(0,-1))+1)+marker.slice(-1);insert="\n"+match[1]+marker+" "+(match[3]?"[ ] ":"");next=cursor+insert.length}
    bridge.edit(before+insert+suffix);Qt.callLater(function(){editor.cursorPosition=next});return true
  }
  Text { id:compactMeasure;visible:false;width:Math.max(1,root.width-Style.space(36));text:editor.text;font:editor.font;wrapMode:Text.Wrap;textFormat:Text.PlainText }
  readonly property real compactEditorHeight: Math.min(Style.space(130),Math.max(Style.space(56),compactMeasure.implicitHeight+Style.space(12)))
  function attachFiles() { if(locked)return; files.environment=bridge.selectedEnvironment;files.thread=bridge.selectedThread;files.open() }
  readonly property var draft: bridge.draft
  readonly property var selection: draft.modelSelection || bridge.selectedInfo.modelSelection || ({})
  readonly property var models: bridge.catalog.models || []
  readonly property int modelIndex: models.findIndex(function(m) { return m.instanceId === root.selection.instanceId && m.model === root.selection.model })
  readonly property var descriptors: modelIndex >= 0 ? (models[modelIndex].descriptors || []) : []
  readonly property bool locked: !!draft.busy || !!draft.attempt || !!draft.loading || !!draft.loadError
  readonly property bool running: !!bridge.selectedInfo.activeRunId
  readonly property bool hasContent: !!draft.text.trim() || draft.attachments.length > 0
  readonly property bool stopMode: running && !hasContent && !draft.queueEdit
  readonly property bool uploading: draft.attachments.some(function(f) { return f.uploading })
  readonly property bool invalidFiles: draft.attachments.some(function(f) { return !!f.error })
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.55)
  function optionValue(d) {
    var o = (selection.options || []).find(function(o) { return o.id === d.id })
    return o ? o.value : d.currentValue
  }
  implicitHeight: content.implicitHeight + Style.space(24)
  color: Qt.rgba(fg.r, fg.g, fg.b, 0.035); radius: Style.space(17)
  border.width: 1; border.color: editor.activeFocus ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.45) : Qt.rgba(fg.r, fg.g, fg.b, 0.09)
  Behavior on border.color { ColorAnimation { duration: 130 } }
  ColumnLayout {
    id: content
    anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(12) }
    height:root.expanded ? Math.max(0,root.height-Style.space(24)) : implicitHeight
    spacing: Style.space(8)
    RowLayout {
      Layout.fillWidth:true;visible:!!root.draft.queueEdit
      T3Icon{name:"square-pen";tint:Color.accent}
      Text{Layout.fillWidth:true;text:T3I18n.tr("Editing queued message");color:Color.accent;font.pixelSize:Style.font.caption}
      Controls.Button{objectName:"t3CancelQueueEdit";text:T3I18n.tr("Cancel edit");enabled:!root.draft.busy&&!root.draft.attempt;onClicked:root.bridge.cancelQueueEdit()}
    }
    Flow {
      Layout.fillWidth: true
      visible: root.draft.attachments.length > 0
      spacing: Style.space(5)
      Repeater {
        model: root.draft.attachments
        Rectangle {
          required property var modelData
          required property int index
          width: Math.min(fileRow.implicitWidth + Style.space(14), Style.space(270)); height: Style.space(30)
          radius: Style.space(7); color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.06)
          RowLayout {
            id: fileRow
            anchors.fill: parent; anchors.leftMargin: Style.space(7); spacing: Style.space(6)
            WorkIndicator { visible: modelData.uploading; active: visible && root.visible; implicitWidth: Style.space(13) }
            T3Icon { visible: !modelData.uploading; name: modelData.error ? "circle-alert" : "file"; tint: modelData.error ? Color.urgent : Color.accent; implicitWidth: Style.space(14) }
            Text { Layout.fillWidth: true; text: modelData.name; color: root.fg; font.pixelSize: Style.font.caption; elide: Text.ElideMiddle; textFormat: Text.PlainText }
            T3IconButton { name: "x"; enabled: !root.locked && !modelData.uploading; tooltipText: T3I18n.error(modelData.error) || T3I18n.tr("Remove attachment"); onClicked: root.bridge.removeAttachment(index) }
          }
        }
      }
    }
    Flickable {
      T3Wheel {flickable:editorViewport}
      id:editorViewport
      objectName:"t3EditorViewport"
      Layout.fillWidth: true
      Layout.fillHeight: root.expanded
      implicitHeight:root.compactEditorHeight
      Layout.minimumHeight:root.expanded?0:root.compactEditorHeight
      Layout.maximumHeight:root.expanded?Infinity:root.compactEditorHeight
      Layout.preferredHeight:root.compactEditorHeight
      contentWidth:width
      contentHeight:editor.height
      boundsBehavior:Flickable.StopAtBounds
      function revealCursor(){var r=editor.cursorRectangle;if(r.y<contentY)contentY=r.y;else if(r.y+r.height>contentY+height)contentY=r.y+r.height-height;contentY=Math.max(0,Math.min(contentY,Math.max(0,contentHeight-height)))}
      onHeightChanged:Qt.callLater(revealCursor)
      Controls.ScrollBar.vertical:Controls.ScrollBar{}
      clip: true
      Controls.TextArea {
        id: editor
        width:editorViewport.width
        height:Math.max(implicitHeight,editorViewport.height)
        textFormat:TextEdit.PlainText
        LayoutMirroring.enabled:false
        objectName: "t3ComposerEditor"
        text: root.draft.text
        placeholderText: T3I18n.tr("Ask anything, @files/folders, $skills or /commands…")
        color: root.fg; placeholderTextColor: root.dim
        selectionColor: Color.accent; selectedTextColor: Color.popups.background
        font.pixelSize: Style.font.body
        wrapMode: TextEdit.Wrap; selectByMouse: true
        readOnly: root.locked
        background: Item {}
        onTextChanged: { if (text !== root.draft.text && activeFocus && !root.locked) {root.historyIndex=-1;root.bridge.edit(text,true)};completionDelay.restart() }
        onSelectionStartChanged:if(!root.draft.loading)root.bridge.rememberDraftCursor(selectionStart,selectionEnd)
        onSelectionEndChanged:if(!root.draft.loading)root.bridge.rememberDraftCursor(selectionStart,selectionEnd)
        onCursorPositionChanged: {if(!root.draft.loading)root.bridge.rememberDraftCursor(selectionStart,selectionEnd);completionDelay.restart();Qt.callLater(editorViewport.revealCursor)}
        onCursorRectangleChanged:Qt.callLater(editorViewport.revealCursor)
        Keys.onPressed:function(event){if(event.modifiers===Qt.MetaModifier && event.key===Qt.Key_A){editor.selectAll();event.accepted=true;return}if(event.matches(StandardKey.Undo)||((event.modifiers===Qt.MetaModifier||event.modifiers===Qt.ControlModifier) && event.key===Qt.Key_Z)){root.bridge.undoDraft(false);event.accepted=true;return}if(event.matches(StandardKey.Redo)||((event.modifiers===Qt.MetaModifier||event.modifiers===Qt.ControlModifier) && event.key===Qt.Key_Y)||((event.modifiers===(Qt.MetaModifier|Qt.ShiftModifier)||event.modifiers===(Qt.ControlModifier|Qt.ShiftModifier))&&event.key===Qt.Key_Z)){root.bridge.undoDraft(true);event.accepted=true;return}if(event.matches(StandardKey.Paste)){event.accepted=true;if(!root.locked)root.bridge.pasteClipboard(editor.selectionStart,editor.selectionEnd)}}
        onActiveFocusChanged: { if (!activeFocus) root.bridge.flushDrafts(); else completionDelay.restart() }
        Keys.onUpPressed: function(event) { event.accepted = root.completionKey(event,-1) || (event.modifiers === Qt.NoModifier && root.navigateHistory(-1)) }
        Keys.onDownPressed: function(event) { event.accepted = root.completionKey(event,1) || (event.modifiers === Qt.NoModifier && root.navigateHistory(1)) }
        Keys.onTabPressed: function(event) {event.accepted=root.completionVisible;if(event.accepted)root.chooseCompletion(root.completionIndex)}
        Keys.onEscapePressed: function(event) {event.accepted=root.completionVisible;if(event.accepted)root.closeCompletion()}
        KeyNavigation.priority: KeyNavigation.BeforeItem
        KeyNavigation.tab: expandEditorButton
        Keys.onReturnPressed: function(event) {
          if(root.completionVisible && event.modifiers===Qt.NoModifier){event.accepted=true;root.chooseCompletion(root.completionIndex);return}
          if(!(event.modifiers & (Qt.ControlModifier|Qt.AltModifier|Qt.MetaModifier)) && root.continueList()){event.accepted=true;return}
          if(event.modifiers!==Qt.MetaModifier){event.accepted=false;return}
          event.accepted = true; root.bridge.submit()
        }
        Keys.onEnterPressed: function(event) {
          if(root.completionVisible && event.modifiers===Qt.NoModifier){event.accepted=true;root.chooseCompletion(root.completionIndex);return}
          if(!(event.modifiers & (Qt.ControlModifier|Qt.AltModifier|Qt.MetaModifier)) && root.continueList()){event.accepted=true;return}
          if(event.modifiers!==Qt.MetaModifier){event.accepted=false;return}
          event.accepted = true; root.bridge.submit()
        }
      }
    }
    ColumnLayout {
      Layout.fillWidth: true
      visible: root.showOptions && !root.draft.queueEdit
      spacing: Style.space(6)
      Repeater {
        model: root.descriptors
        RowLayout {
          required property var modelData
          Layout.fillWidth: true
          Text { Layout.preferredWidth: Style.space(125); text: T3I18n.tr(modelData.label); font.pixelSize: Style.font.caption; color: root.dim; textFormat: Text.PlainText }
          T3Select {
            objectName: "t3Option_" + modelData.id
            Layout.fillWidth: true; visible: modelData.type === "select"
            enabled: !root.locked
            model: modelData.options || []; textRole: "label";translateLabels:true
            currentIndex: model.findIndex(function(o) { return o.id === root.optionValue(modelData) })
            onActivated: function(index) { root.bridge.chooseOption(modelData.id, model[index].id) }
          }
          Controls.Switch {
            visible: modelData.type === "boolean"; enabled: !root.locked
            checked: !!root.optionValue(modelData)
            onToggled: root.bridge.chooseOption(modelData.id, checked)
            palette.highlight: Color.accent
          }
        }
      }
      RowLayout {
        Text { text: T3I18n.tr("Thread permissions"); color: root.dim; font.pixelSize: Style.font.caption }
        Item { Layout.fillWidth: true }
        Text { text: root.bridge.selectedInfo.runtimeMode==="full-access"?T3I18n.tr("Full access"):root.bridge.selectedInfo.runtimeMode==="approval-required"?T3I18n.tr("Approval required"):root.bridge.selectedInfo.runtimeMode || "—"; color: root.dim; font.pixelSize: Style.font.caption }
      }
    }
    Text {
      Layout.fillWidth: true
      visible: !!text
      text: root.draft.error ? (root.draft.error === "rpc_outcome_unknown" ? T3I18n.tr("No confirmation from T3. Retry the same submission; it will not duplicate.") : T3I18n.error(root.draft.error))
        : root.invalidFiles ? T3I18n.tr("An upload failed. Remove it and attach it again.")
        : T3I18n.error(root.bridge.catalog.error || root.bridge.messageError)
      color: Color.urgent; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText
    }
    RowLayout {
      Layout.fillWidth: true; spacing: Style.space(5)
      T3IconButton {
        name: "paperclip"; tooltipText: T3I18n.tr("Attach files · or drag them here")
        enabled: !root.locked
        onClicked: root.attachFiles()
      }
      T3ModelPicker {
        id:modelPicker;objectName:"t3ComposerModel"
        Layout.fillWidth:true;Layout.maximumWidth:Style.space(300)
        bridge:root.bridge;selection:root.selection
        enabled:!root.locked && !root.draft.queueEdit
        onPicked:editor.forceActiveFocus()
      }
      T3IconButton { enabled:!root.draft.queueEdit;name: "sliders-horizontal"; tooltipText: T3I18n.tr("Model options"); onClicked: root.showOptions = !root.showOptions }
      Item { Layout.fillWidth: true }
      Item {
        id:draftStatus
        objectName:"t3DraftStatus"
        implicitWidth:Style.space(22);implicitHeight:Style.space(28)
        readonly property bool saving:root.draft.loading || root.draft.saveState==="saving" || saveQuietPeriod.running
        readonly property bool failed:!!root.draft.loadError || !!root.draft.saveError
        readonly property string statusText:failed?T3I18n.tr("Draft could not be saved/restored"):root.draft.loading?T3I18n.tr("Restoring draft…"):saving?T3I18n.tr("Saving…"):T3I18n.tr("Draft saved")
        Accessible.role:Accessible.StaticText
        Accessible.name:statusText
        T3Icon{anchors.centerIn:parent;visible:!draftStatus.saving || draftStatus.failed;name:draftStatus.failed?"circle-alert":"check";tint:draftStatus.failed?Color.urgent:root.dim;implicitWidth:Style.space(14)}
        WorkIndicator{anchors.centerIn:parent;visible:draftStatus.saving&&!draftStatus.failed;active:visible&&root.visible;implicitWidth:Style.space(14);tint:root.dim}
        HoverHandler{id:draftStatusHover}
        Controls.ToolTip.visible:draftStatusHover.hovered
        Controls.ToolTip.text:statusText
        Controls.ToolTip.delay:500
      }
      T3IconButton {
        id:expandEditorButton
        objectName:"t3ExpandEditor"
        KeyNavigation.backtab:editor
        name:root.expanded?"chevron-down":"square-pen"
        tooltipText:(root.expanded?T3I18n.tr("Back to chat"):T3I18n.tr("Edit"))+" · "+(root.expanded?"Esc":"Super+E")
        onClicked:root.toggleExpanded()
      }
      T3Select {
        objectName:"t3DeliverySelect"
        Layout.preferredWidth: Style.space(115)
        visible:!root.draft.queueEdit
        model: [T3I18n.tr("Automatic"), T3I18n.tr("Queue")]
        currentIndex: root.draft.delivery === "queue" ? 1 : 0
        enabled: !root.locked
        onActivated: function(index) { root.bridge.patchDraft(root.bridge.selectedKey, {delivery: index === 1 ? "queue" : "auto"}) }
      }
      Rectangle {
        id: primaryAction
        objectName: "t3PrimaryAction"
        property string actionMode: root.stopMode ? "stop" : root.draft.queueEdit ? "edit" : root.running ? "queue" : "send"
        property string iconName: root.stopMode ? "stop-filled" : root.draft.attempt ? "refresh-cw" : root.draft.queueEdit ? "check" : root.running ? "list-plus" : "arrow-up"
        activeFocusOnTab: sendMouse.enabled
        Accessible.role: Accessible.Button
        Accessible.name: root.stopMode ? T3I18n.tr("Stop execution and hold queue") : root.draft.queueEdit ? T3I18n.tr("Save queued message") : root.running ? T3I18n.tr("Queue message") : T3I18n.tr("Send message")
        function activate() { if(sendMouse.enabled) { if(root.stopMode) root.bridge.stop(); else root.bridge.submit() } }
        Keys.onReturnPressed: activate()
        Keys.onSpacePressed: activate()
        border.width: activeFocus ? 1 : 0; border.color: root.fg
        width: Style.space(32); height: width; radius: width / 2
        color: root.stopMode ? Color.urgent : Color.accent
        opacity: sendMouse.enabled ? 1 : 0.35
        Behavior on color { ColorAnimation { duration: 130 } }
        WorkIndicator { anchors.centerIn: parent; visible: root.stopMode ? !!root.draft.stopping : root.draft.busy || root.uploading; active: visible && root.visible; tint: root.stopMode ? "white" : Color.popups.background }
        T3Icon { anchors.centerIn: parent; name: primaryAction.iconName; tint: root.stopMode ? "white" : Color.popups.background; visible: root.stopMode ? !root.draft.stopping : !root.draft.busy && !root.uploading }
        MouseArea {
          id: sendMouse; anchors.fill: parent; cursorShape: Qt.PointingHandCursor
          enabled: root.stopMode ? !root.draft.stopping : !root.draft.loading && !root.draft.loadError && !root.draft.busy && !root.uploading && !root.invalidFiles && root.hasContent
          onClicked: primaryAction.activate()
        }
        Controls.ToolTip.visible: sendHover.hovered
        Controls.ToolTip.text: primaryAction.Accessible.name + (root.stopMode ? "" : " · Super+Enter")
        HoverHandler { id: sendHover }
      }

    }
  }
  DropArea {
    anchors.fill: parent
    onEntered: function(drag) { drag.accepted = drag.hasUrls && !root.locked }
    onDropped: function(drop) {
      if (root.locked) return
      for (var i = 0; i < drop.urls.length; i++) root.bridge.attach(drop.urls[i], root.bridge.selectedEnvironment, root.bridge.selectedThread)
      drop.acceptProposedAction()
    }
    Rectangle { anchors.fill: parent; radius: root.radius; color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.10); border.width: 1; border.color: Color.accent; visible: parent.containsDrag }
  }
  FileDialog {
    id: files
    property string environment: ""
    property string thread: ""
    title: T3I18n.tr("Attach to T3")
    fileMode: FileDialog.OpenFiles
    onAccepted: { for (var i = 0; i < selectedFiles.length; i++) root.bridge.attach(selectedFiles[i], environment, thread) }
  }
}
