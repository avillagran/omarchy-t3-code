import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Window 6.11
import QtQuick.Controls as Controls
import QtQuick.Dialogs
import qs.Commons
import qs.Ui
import "../js/QuickReplies.js" as QuickReplies

Rectangle {
  id: root
  property alias inputField: editor
  readonly property bool modelPickerAvailable:modelPicker.enabled
  readonly property bool permissionPickerAvailable:permissionPicker.enabled
  readonly property bool modelPickerOpen:modelPicker.popup.visible
  readonly property bool permissionPickerOpen:permissionPicker.popup.visible
  function nativeModelControl(command){
    if(command==="modelPicker.toggle"){if(modelPicker.enabled)modelPicker.popup.visible?modelPicker.popup.close():modelPicker.popup.open();return}
    if(command==="composer.mode"){if(permissionPicker.enabled)permissionPicker.popup.open();return}
    if(command==="composer.branch"){branchPicker.open();return}
    if(command==="composer.increaseEffort"||command==="composer.decreaseEffort"){modelOptions.stepEffort(command==="composer.increaseEffort"?1:-1);return}
    if(command==="composer.effort"){showOptions=true;Qt.callLater(function(){modelOptions.focusEffort()});return}
    if(command==="modelPicker.previousProvider"||command==="modelPicker.nextProvider"){modelPicker.moveProvider(command==="modelPicker.nextProvider"?1:-1);return}
    var jump=/^modelPicker\.jump\.([1-9])$/.exec(command);if(jump)modelPicker.selectResult(Number(jump[1])-1)
  }
  required property var bridge
  property bool showOptions: false
  property bool expanded: false
  signal toggleExpanded()
  signal previewAttachment(var attachment)
  property bool draftsOpen:false
  readonly property bool projectDraftsVisible:draftsOpen&&!!bridge.selectedInfo.projectId
  function toggleProjectDrafts(){draftsOpen=!draftsOpen;if(draftsOpen){bridge.requestProjectDrafts("list");Qt.callLater(function(){projectDraftDrawer.focusList()})}}
  onExpandedChanged:{draftsOpen=expanded;if(expanded&&bridge.selectedInfo.projectId)bridge.requestProjectDrafts("list")}
  readonly property bool draftReady:!draft.loading&&!draft.loadError&&!!bridge.selectedInfo.projectId
  onDraftReadyChanged:if(draftReady&&projectDraftsVisible)bridge.requestProjectDrafts("list")
  Connections{target:root.bridge;ignoreUnknownSignals:true
    function onSelectedKeyChanged(){if(root.projectDraftsVisible)Qt.callLater(function(){root.bridge.requestProjectDrafts("list")})}
    function onProjectDraftRestored(key){if(key===root.bridge.selectedKey){if(!root.expanded)root.draftsOpen=false;Qt.callLater(function(){editor.cursorPosition=editor.length;editor.forceActiveFocus()})}}
  }
  // Attachment references (`![name](t3-context://v1/image/…)`) show as chips,
  // as in T3's composer. The draft keeps its text; the editor shows each
  // reference as one inline image (one character, U+FFFC) and every edit is
  // turned back into the draft's text, so positions are converted at the edges.
  property var viewRefs:[]
  property string viewText:""
  property bool syncingEditor:false
  readonly property string referenceSource:"!?\\[[^\\]\\n]*\\]\\(t3-context://v1/[a-z]+/[^)\\s]+\\)"
  function splitReferences(source){var refs=[];var text=String(source || "").replace(new RegExp(referenceSource,"g"),function(m){refs.push(m);return "\ufffc"});return {text:text,refs:refs}}
  function joinReferences(view,refs){var i=0;return String(view).replace(/\ufffc/g,function(){return i<refs.length?refs[i++]:""})}
  function sourceText(){return joinReferences(viewText,viewRefs)}
  // Editor position -> draft position, and back.
  function sourcePosition(position){var extra=0,n=0;for(var k=0;k<position&&k<viewText.length;k++)if(viewText.charCodeAt(k)===0xfffc){extra+=(viewRefs[n] || "x").length-1;n++}return position+extra}
  function viewPosition(position){var s=0,n=0;for(var k=0;k<viewText.length;k++){var len=viewText.charCodeAt(k)===0xfffc?(viewRefs[n++] || "x").length:1;if(s+len>position)return k+(position>s?1:0);s+=len}return viewText.length}
  function commitView(view,typed){bridge.edit(joinReferences(view,viewRefs),typed)}
  function editorPlain(){return editor.getText(0,editor.length).replace(/[\u2028\u2029]/g,"\n")}
  function escapeHtml(t){return t.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}
  function sizeText(bytes){return !(bytes>0)?"":bytes<1024?bytes+" B":bytes<1048576?Math.round(bytes/1024)+" KB":(bytes/1048576).toFixed(1)+" MB"}
  // Qt SVG (Tiny 1.2) has no rgba(): colours are hex, opacity a separate attribute.
  function svgColor(col){var hex=function(v){var h=Math.round(v*255).toString(16);return h.length<2?"0"+h:h};return "#"+hex(col.r)+hex(col.g)+hex(col.b)}
  function chipImage(ref){
    var m=/^(!?)\[([^\]]*)\]\(t3-context:\/\/v1\/([a-z]+)\/([^)]+)\)$/.exec(ref) || ["","","",""," "]
    var attachment=(draft.attachments || []).map(function(a){return a.attachment || {}}).find(function(a){return a.id===m[4]}) || {}
    var name=m[2] || attachment.name || m[3],size=sizeText(attachment.sizeBytes)
    chipName.text=name;chipSize.text=size
    var h=Math.round(Style.space(22)),pad=Math.round(Style.space(7)),icon=Math.round(Style.space(13)),gap=Math.round(Style.space(5))
    var nameW=Math.ceil(Math.min(chipName.advanceWidth,Style.space(200))),sizeW=size?Math.ceil(chipSize.advanceWidth)+gap:0
    var w=pad+icon+gap+nameW+sizeW+pad,f=chipName.font.pixelSize,ink=svgColor(fg),accent=svgColor(Color.accent),family=escapeHtml(chipName.font.family),baseline=(h/2+f*0.35).toFixed(1)
    var glyph=m[1]==="!"?'<rect x="1.5" y="1.5" width="11" height="11" rx="2" fill="none" stroke="'+accent+'" stroke-width="1.3"/><circle cx="5" cy="5" r="1.3" fill="'+accent+'"/><path d="M2 11l3.5-3.5 2.5 2.5 1.5-1.5 3 3" fill="none" stroke="'+accent+'" stroke-width="1.2"/>'
      :'<path d="M3 1.5h5l3.5 3.5v7.5h-8.5z" fill="none" stroke="'+accent+'" stroke-width="1.3"/>'
    // Drawn at the screen's scale (at least 2x) and shown at its logical size, so it stays sharp.
    var scale=Math.max(2,Math.ceil(editor.Screen.devicePixelRatio || 1))
    var svg='<svg xmlns="http://www.w3.org/2000/svg" width="'+(w*scale)+'" height="'+(h*scale)+'" viewBox="0 0 '+w+' '+h+'">'
      +'<rect x="0.5" y="0.5" width="'+(w-1)+'" height="'+(h-1)+'" rx="'+Math.round(h/3.2)+'" fill="'+ink+'" fill-opacity="0.07" stroke="'+ink+'" stroke-opacity="0.16"/>'
      +'<g transform="translate('+pad+','+((h-icon)/2)+') scale('+(icon/14)+')">'+glyph+'</g>'
      +'<text x="'+(pad+icon+gap)+'" y="'+baseline+'" font-family="'+family+'" font-size="'+f+'" fill="'+ink+'">'+escapeHtml(chipName.elidedText)+'</text>'
      +(size?'<text x="'+(pad+icon+gap+nameW+gap)+'" y="'+baseline+'" font-family="'+family+'" font-size="'+f+'" fill="'+ink+'" fill-opacity="0.55">'+escapeHtml(size)+'</text>':"")+'</svg>'
    return '<img src="data:image/svg+xml,'+encodeURIComponent(svg)+'" width="'+w+'" height="'+h+'" style="vertical-align:middle">'
  }
  TextMetrics{id:chipName;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;elideWidth:Style.space(200)}
  TextMetrics{id:chipSize;font.pixelSize:Style.font.caption}
  function htmlOf(split){var n=0,self=root;return '<span style="white-space:pre-wrap">'+split.text.split("").map(function(ch){return ch==="\ufffc"?self.chipImage(split.refs[n++]):ch==="\n"?"<br>":self.escapeHtml(ch)}).join("")+"</span>"}
  // Draft -> editor, when the draft changed from outside the editor (thread switch, completion, paste, undo).
  function renderEditor(){
    var split=splitReferences(draft.text)
    if(split.text===viewText&&split.refs.join("\n")===viewRefs.join("\n")&&editorPlain()===viewText)return
    var cursor=editor.cursorPosition;syncingEditor=true
    viewRefs=split.refs;viewText=split.text;editor.text=htmlOf(split)
    editor.cursorPosition=Math.min(cursor,editor.length);syncingEditor=false
  }
  // Editor -> draft: keep the references whose chips survived the edit, in order.
  function editorEdited(){
    var next=editorPlain(),old=viewText,p=0,q=0
    while(p<old.length&&p<next.length&&old[p]===next[p])p++
    while(q<old.length-p&&q<next.length-p&&old[old.length-1-q]===next[next.length-1-q])q++
    var count=function(t){return (t.match(/\ufffc/g) || []).length}
    var before=count(old.slice(0,p)),after=count(old.slice(old.length-q)),middle=next.slice(p,next.length-q),stray=count(middle)>0
    var refs=viewRefs.slice(0,before).concat(after?viewRefs.slice(viewRefs.length-after):[])
    if(stray)next=next.slice(0,p)+middle.replace(/\ufffc/g,"")+next.slice(next.length-q)
    viewRefs=refs;viewText=next
    if(stray)Qt.callLater(renderEditor)
    return joinReferences(next,refs)
  }
  property string lastDraftText:""
  property string pendingCursorKey:bridge.selectedKey
  function positionDraftAtEnd(){if(pendingCursorKey!==bridge.selectedKey||draft.loading)return;var key=pendingCursorKey;Qt.callLater(function(){if(key!==root.bridge.selectedKey||root.draft.loading)return;editor.cursorPosition=editor.length;root.bridge.rememberDraftCursor(root.sourcePosition(editor.cursorPosition),root.sourcePosition(editor.cursorPosition));root.pendingCursorKey=""})}
  Component.onCompleted:positionDraftAtEnd()
  Connections{target:root.bridge;function onSelectedKeyChanged(){root.pendingCursorKey=root.bridge.selectedKey;root.positionDraftAtEnd()}function onDraftRestored(key,start,end){if(key!==root.bridge.selectedKey)return;Qt.callLater(function(){if(key!==root.bridge.selectedKey)return;root.renderEditor();editor.select(root.viewPosition(start),root.viewPosition(end));editor.forceActiveFocus()})}}
  onDraftChanged:{renderEditor();positionDraftAtEnd();if(!draft.loading && draft.text!==lastDraftText && editor.activeFocus)saveQuietPeriod.restart();lastDraftText=draft.text}
  Timer{id:saveQuietPeriod;interval:1000}

  Connections{target:root.bridge;function onDictationInserted(key,cursor){if(key===root.bridge.selectedKey)Qt.callLater(function(){if(key!==root.bridge.selectedKey)return;root.renderEditor();editor.cursorPosition=root.viewPosition(cursor);editor.forceActiveFocus()})}}
  // Ctrl+Space: held past a moment it is push-to-talk, a quick tap starts and a second tap stops.
  property double dictationPressedAt: 0
  function dictationKey(event,pressed){
    if(event.key!==Qt.Key_Space||!bridge.dictationEnabled)return false
    if(pressed){
      if(event.modifiers!==Qt.ControlModifier)return false
      if(event.isAutoRepeat)return true
      var state=bridge.dictation.state
      if(state==="idle"){dictationPressedAt=Date.now();bridge.startDictation()}else if(state==="starting"||state==="recording"){dictationPressedAt=0;bridge.stopDictation()}
      return true
    }
    if(event.isAutoRepeat||!dictationPressedAt)return false
    var held=Date.now()-dictationPressedAt>450;dictationPressedAt=0
    if(held)bridge.stopDictation()
    return held
  }
  Connections{target:root.bridge;function onClipboardInserted(key,cursor){if(key===root.bridge.selectedKey)Qt.callLater(function(){if(key!==root.bridge.selectedKey)return;root.renderEditor();editor.cursorPosition=root.viewPosition(cursor);editor.forceActiveFocus()})}}
  property int historyIndex: -1
  property string historyScratch: ""
  readonly property var promptHistory: bridge.transcript.filter(function(m){return m.role === "user"&&!m.attribution}).map(function(m){return String(m.recallableText!=null?m.recallableText:m.text || "").replace(/\n*<t3_context\b[\s\S]*<\/t3_context>\s*(?:\[Attached[\s\S]*)?$/,"").trim()}).filter(function(t){return !!t}).reverse()
  Connections { target: root.bridge; function onThreadWillChange(){root.historyIndex=-1;root.historyScratch="";saveQuietPeriod.stop()} }
  property var completion: null
  property int completionIndex: 0
  property string dismissedCompletion: ""
  readonly property bool completionVisible: !!completion && editor.activeFocus && !locked
  function closeCompletion() { dismissedCompletion=root.viewText+"/"+editor.cursorPosition;completion=null;bridge.dismissSuggestions() }
  function updateCompletion() {
    if (!editor.activeFocus || locked || dismissedCompletion===root.viewText+"/"+editor.cursorPosition) return
    var prefix=root.viewText.slice(0,editor.cursorPosition)
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
    var item=bridge.suggestionItems[index];var before=root.viewText.slice(0,completion.start);var after=root.viewText.slice(completion.end)
    var insert=item.insert || "";root.commitView(before+insert+after);var cursor=before.length+insert.length
    closeCompletion();Qt.callLater(function(){editor.cursorPosition=cursor;dismissedCompletion=root.viewText+"/"+cursor;editor.forceActiveFocus();if(item.kind==="model"){root.showOptions=true;modelPicker.forceActiveFocus();modelPicker.popup.open()}});return true
  }
  function completionKey(event,step) {
    if(!completionVisible || event.modifiers!==Qt.NoModifier)return false
    var count=bridge.suggestionItems.length
    if(count)completionIndex=(completionIndex+step+count)%count
    return true
  }
  Timer { id: completionDelay; interval:140; onTriggered:root.updateCompletion() }
  // A draft briefly locked (loading or saving) skips completion; look again once it is free.
  onLockedChanged:if(!locked&&editor.activeFocus)completionDelay.restart()
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
    var edge=editor.positionToRectangle(direction < 0 ? 0 : editor.length)
    if (Math.abs(editor.cursorRectangle.y-edge.y)>1) return false
    if (direction < 0) {
      if (historyIndex+1 >= promptHistory.length) return false
      if (historyIndex < 0) historyScratch=root.draft.text
      historyIndex++;bridge.edit(promptHistory[historyIndex])
    } else {
      if (historyIndex < 0) return false
      historyIndex--;bridge.edit(historyIndex < 0 ? historyScratch : promptHistory[historyIndex])
    }
    Qt.callLater(function(){editor.cursorPosition=direction<0?0:editor.length});return true
  }
  function continueList() {
    if(locked || editor.selectionStart!==editor.selectionEnd)return false
    var cursor=editor.cursorPosition;var before=root.viewText.slice(0,cursor);var start=before.lastIndexOf("\n")+1
    var line=before.slice(start);var match=/^([ \t]*)([-+*•◦‣–]|\d+[.)])\s+(\[[ xX]\]\s+)?(.*)$/.exec(line)
    if(!match)return false
    var suffix=root.viewText.slice(cursor);var insert="";var next=cursor
    if(!match[4].trim() && !suffix.split("\n")[0].trim()) {before=before.slice(0,start);next=start}
    else {var marker=match[2];if(/^\d/.test(marker))marker=String(Number(marker.slice(0,-1))+1)+marker.slice(-1);insert="\n"+match[1]+marker+" "+(match[3]?"[ ] ":"");next=cursor+insert.length}
    root.commitView(before+insert+suffix);Qt.callLater(function(){editor.cursorPosition=next});return true
  }
  function handleEnter(event) {
    // Let the input method finish a composed character before interpreting send.
    if(editor.inputMethodComposing){event.accepted=false;return}
    var modifiers=event.modifiers
    if(root.completionVisible && modifiers===Qt.NoModifier){event.accepted=true;root.chooseCompletion(root.completionIndex);return}
    if(modifiers===(Qt.ControlModifier|Qt.ShiftModifier)){event.accepted=true;if(!event.isAutoRepeat)root.bridge.submit(true);return}
    if(modifiers===Qt.ControlModifier || (modifiers===Qt.NoModifier && !root.expanded)){
      event.accepted=true;if(!event.isAutoRepeat)root.bridge.submit(false,undefined,modifiers===Qt.ControlModifier&&!root.expanded);return
    }
    if(modifiers===Qt.ShiftModifier || (modifiers===Qt.NoModifier && root.expanded)){
      event.accepted=true
      if(!root.locked && !root.continueList()){
        var start=editor.selectionStart,end=editor.selectionEnd
        root.commitView(root.viewText.slice(0,start)+"\n"+root.viewText.slice(end),true)
        Qt.callLater(function(){editor.cursorPosition=start+1})
      }
      return
    }
    event.accepted=false
  }
  Text { id:compactMeasure;visible:false;width:Math.max(1,root.width-Style.space(36));text:root.viewText.replace(/\ufffc/g,"\u2003\u2003\u2003\u2003\u2003");font:editor.font;wrapMode:Text.Wrap;textFormat:Text.PlainText }
  readonly property real compactEditorHeight: Math.min(Style.space(130),Math.max(Style.space(56),compactMeasure.implicitHeight+Style.space(12)))
  function attachFiles() { if(locked)return; files.environment=bridge.selectedEnvironment;files.thread=bridge.selectedThread;files.open() }
  readonly property var draft: bridge.draft
  readonly property var selection: draft.modelSelection || bridge.selectedInfo.modelSelection || ({})
  readonly property var models: bridge.catalog.models || []
  readonly property int modelIndex: models.findIndex(function(m) { return m.instanceId === root.selection.instanceId && m.model === root.selection.model })
  readonly property var descriptors: modelIndex >= 0 ? (models[modelIndex].descriptors || []) : []
  readonly property bool locked: !!(bridge.planOperation || {}).busy || !!draft.busy || !!draft.attempt || !!draft.loading || !!draft.loadError
  readonly property bool running: !!bridge.selectedInfo.activeRunId
  readonly property bool hasContent: !!draft.text.trim() || draft.attachments.length > 0
  readonly property bool stopMode: running && !hasContent && !draft.queueEdit
  // During a run a message follows T3's follow-up preference (queue or steer);
  // Ctrl, held while typing or over the send button, switches to the other one.
  property bool editorCtrlHeld:false
  readonly property bool alternateHeld:editorCtrlHeld||(sendHover.hovered&&(sendHover.point.modifiers&Qt.ControlModifier)!==0)
  readonly property string followUp:bridge.dispatchMode(alternateHeld)
  readonly property bool uploading: draft.attachments.some(function(f) { return f.uploading })
  readonly property bool invalidFiles: draft.attachments.some(function(f) { return !!f.error })
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.55)
  function optionValue(d) {
    var o = (selection.options || []).find(function(o) { return o.id === d.id })
    return o ? o.value : d.currentValue
  }
  implicitHeight: content.implicitHeight + Style.space(24)
  color: Color.popups.background; radius: Style.space(17)
  border.width: 1; border.color: editor.activeFocus ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.45) : Qt.rgba(fg.r, fg.g, fg.b, 0.09)
  Behavior on border.color { ColorAnimation { duration: 130 } }
  ColumnLayout {
    id: content
    anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(12) }
    height:root.expanded ? Math.max(0,root.height-Style.space(24)) : implicitHeight
    spacing: Style.space(8)
    T3Reveal {
      id:draftsReveal;Layout.fillWidth:true;shown:root.projectDraftsVisible
    T3ProjectDrafts {
      id:projectDraftDrawer;objectName:"t3ProjectDrafts";anchors.left:parent.left;anchors.right:parent.right;visible:root.projectDraftsVisible||draftsReveal.moving
      bridge:root.bridge
      onCloseRequested:{root.draftsOpen=false;editor.forceActiveFocus()}
    }
    }
    RowLayout {
      Layout.fillWidth:true;visible:!!root.draft.queueEdit
      T3Icon{name:"square-pen";tint:Color.accent}
      Text{Layout.fillWidth:true;text:T3I18n.tr("Editing queued message");color:Color.accent;font.pixelSize:Style.font.caption}
      Controls.Button{objectName:"t3CancelQueueEdit";text:T3I18n.tr("Cancel edit");enabled:!root.draft.busy&&!root.draft.attempt;onClicked:root.bridge.cancelQueueEdit()}
    }
    // Worktree setup progress (T3's worktree setup card).
    ColumnLayout {
      objectName:"t3WorktreeSetup";Layout.fillWidth:true;spacing:Style.space(4)
      readonly property var setup:root.bridge.selectedWorktreeSetup
      visible:!!setup&&(setup.phase==="running"||setup.phase==="failed")
      RowLayout{Layout.fillWidth:true;spacing:Style.space(6)
        WorkIndicator{active:!!parent.parent.setup&&parent.parent.setup.phase==="running";visible:active;implicitWidth:Style.space(12);implicitHeight:implicitWidth}
        Text{Layout.fillWidth:true;text:parent.parent.setup?(parent.parent.setup.phase==="failed"?T3I18n.tr("Worktree setup failed"):T3I18n.tr("Setting up worktree {branch}",{branch:parent.parent.setup.branch||""})):"";color:parent.parent.setup&&parent.parent.setup.phase==="failed"?Color.urgent:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
        T3PickerButton{visible:!!parent.parent.setup&&parent.parent.setup.phase==="running";primary:false;implicitHeight:Style.space(24);text:T3I18n.tr("Cancel");onClicked:root.bridge.cancelWorktreeSetup()}
      }
      Repeater{model:parent.setup?parent.setup.stages.filter(function(s){return s.status!=="pending"&&s.status!=="skipped"}):[]
        Text{required property var modelData;Layout.fillWidth:true;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight;color:modelData.status==="failed"?Color.urgent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.7)
          text:(modelData.status==="done"?"✓ ":modelData.status==="failed"?"✕ ":modelData.status==="warning"?"! ":"… ")+T3I18n.tr(({fetch:"Fetching",checkout:"Checking out",submodules:"Submodules","setup-script":"Setup script",agent:"Starting agent"})[modelData.id]||modelData.id)+(modelData.percent!==null&&modelData.percent!==undefined?" "+modelData.percent+"%":"")+(modelData.detail?" · "+modelData.detail:"")}
      }
      Text{visible:!!parent.setup&&!!parent.setup.error;Layout.fillWidth:true;wrapMode:Text.Wrap;color:Color.urgent;font.pixelSize:Style.font.caption;text:parent.setup&&parent.setup.error?parent.setup.error:""}
    }
    // Subagents still working for this thread (T3's subagent bar).
    Flow {
      objectName:"t3SubagentBar";Layout.fillWidth:true;spacing:Style.space(6);visible:(root.bridge.activeSubagents||[]).some(function(s){return ["pending","running","waiting"].indexOf(s.status)>=0||!!s.scriptPath})
      Repeater{model:(root.bridge.activeSubagents||[]).filter(function(s){return ["pending","running","waiting"].indexOf(s.status)>=0||!!s.scriptPath})
        Rectangle{required property var modelData;height:Style.space(26);width:subRow.implicitWidth+Style.space(14);radius:height/2;color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.1)
          Row{id:subRow;anchors.centerIn:parent;spacing:Style.space(6)
            WorkIndicator{active:modelData.status==="running";visible:active;implicitWidth:Style.space(10);implicitHeight:implicitWidth;anchors.verticalCenter:parent.verticalCenter}
            Text{text:modelData.title;color:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideRight;width:Math.min(implicitWidth,Style.space(220));textFormat:Text.PlainText;anchors.verticalCenter:parent.verticalCenter}
            T3IconButton{objectName:"t3WorkflowScript";name:"code";implicitWidth:Style.space(18);implicitHeight:implicitWidth;visible:!!modelData.scriptPath;tooltipText:T3I18n.tr("Show workflow script");onClicked:root.bridge.openWorkflowScript(modelData);anchors.verticalCenter:parent.verticalCenter}
            T3IconButton{name:"square";implicitWidth:Style.space(18);implicitHeight:implicitWidth;visible:["pending","running","waiting"].indexOf(modelData.status)>=0&&(modelData.workflow||!!modelData.runId);tooltipText:T3I18n.tr(modelData.workflow?"Stop workflow":"Stop subagent");enabled:!root.bridge.managementBusy;onClicked:modelData.workflow?root.bridge.stopWorkflow(modelData):root.bridge.stopSubagent(modelData);anchors.verticalCenter:parent.verticalCenter}
            T3IconButton{name:"arrow-right";implicitWidth:Style.space(18);implicitHeight:implicitWidth;visible:!!modelData.childThreadId;tooltipText:T3I18n.tr("Open subagent thread");onClicked:root.bridge.selectThread(root.bridge.selectedEnvironment,modelData.childThreadId);anchors.verticalCenter:parent.verticalCenter}
          }
        }
      }
    }
    // The checkout moved off the thread's branch (T3's branch change notice).
    RowLayout {
      objectName:"t3BranchMismatch"
      readonly property var mismatch:root.bridge.branchMismatch.key===root.bridge.selectedKey&&!root.bridge.branchMismatch.dismissed?root.bridge.branchMismatch.value:null
      Layout.fillWidth:true;visible:!!mismatch;spacing:Style.space(6)
      T3Icon{name:"git-branch";tint:Color.accent;implicitWidth:Style.space(14)}
      Text{Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:Color.popups.text;font.pixelSize:Style.font.caption
        text:parent.mismatch?T3I18n.tr("This thread last ran on {thread}. Sending will continue on {current}.",{thread:parent.mismatch.threadBranch,current:parent.mismatch.currentBranch}):""}
      T3PickerButton{objectName:"t3BranchRestore";visible:!!parent.mismatch&&parent.mismatch.canRestore;primary:false;implicitHeight:Style.space(26);text:T3I18n.tr(root.bridge.branchMismatch.busy?"Restoring...":"Restore branch");enabled:!root.bridge.branchMismatch.busy&&!root.bridge.selectedInfo.working;onClicked:root.bridge.resolveBranchMismatch(true)}
      T3PickerButton{objectName:"t3BranchAdopt";visible:!!parent.mismatch;primary:false;implicitHeight:Style.space(26);text:T3I18n.tr("Use {branch}",{branch:parent.mismatch?parent.mismatch.currentBranch:""});enabled:!root.bridge.branchMismatch.busy;onClicked:root.bridge.resolveBranchMismatch(false)}
      T3IconButton{name:"x";tooltipText:T3I18n.tr("Dismiss branch change notice");onClicked:root.bridge.branchMismatch=Object.assign({},root.bridge.branchMismatch,{dismissed:true})}
    }
    Flow {
      Layout.fillWidth: true
      visible: root.draft.attachments.length > 0
      spacing: Style.space(5)
      Repeater {
        model: root.draft.attachments
        // Uploaded images show as thumbnails, as in T3's composer; other files as chips.
        Item {
          id: attachmentTile
          required property var modelData
          required property int index
          readonly property bool image: !!modelData.attachment && modelData.attachment.type === "image" && !modelData.error && !modelData.uploading
          implicitWidth: image ? thumbnail.width : chip.width; implicitHeight: image ? thumbnail.height : chip.height
          width: implicitWidth; height: implicitHeight
          T3MessageImage {
            id: thumbnail
            objectName: "t3PreviewDraftAttachment_" + attachmentTile.index
            visible: attachmentTile.image; enabled: attachmentTile.image
            width: Style.space(56); height: Style.space(56)
            bridge: root.bridge; attachment: attachmentTile.modelData.attachment || ({})
            active: attachmentTile.image && root.visible
            onEnlarge: function(a) { root.previewAttachment(a) }
            T3IconButton {
              objectName: "t3RemoveDraftImage_" + attachmentTile.index
              anchors.top: parent.top; anchors.right: parent.right; anchors.margins: Style.space(3)
              implicitWidth: Style.space(20); implicitHeight: implicitWidth; name: "x"
              radius: width / 2; color: Qt.rgba(0, 0, 0, 0.55); tint: "white"
              enabled: !root.locked; tooltipText: T3I18n.tr("Remove attachment") + ": " + attachmentTile.modelData.name
              onClicked: root.bridge.removeAttachment(attachmentTile.index)
            }
          }
        Rectangle {
          id: chip
          visible: !attachmentTile.image
          readonly property var modelData: attachmentTile.modelData
          readonly property int index: attachmentTile.index
          width: Math.min(fileRow.implicitWidth + Style.space(14), Style.space(270)); height: Style.space(30)
          radius: Style.space(7); color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.06)
          RowLayout {
            id: fileRow
            anchors.fill: parent; anchors.leftMargin: Style.space(7); spacing: Style.space(6)
            WorkIndicator { visible: modelData.uploading; active: visible && root.visible; implicitWidth: Style.space(13) }
            T3Icon { visible: !modelData.uploading; name: modelData.error ? "circle-alert" : "file"; tint: modelData.error ? Color.urgent : Color.accent; implicitWidth: Style.space(14) }
            Text { Layout.fillWidth: true; text: modelData.name; color: root.fg; font.pixelSize: Style.font.caption; elide: Text.ElideMiddle; textFormat: Text.PlainText }
            T3IconButton { objectName:"t3PreviewDraftFile_"+index;name:"eye";glyph:"\uf06e";glyphScale:0.85;enabled:!modelData.uploading&&!modelData.error&&!!modelData.attachment;tooltipText:T3I18n.tr("Preview")+": "+modelData.name;onClicked:root.previewAttachment(modelData.attachment) }
            T3IconButton { name: "x"; enabled: !root.locked && !modelData.uploading; tooltipText: T3I18n.error(modelData.error) || T3I18n.tr("Remove attachment"); onClicked: root.bridge.removeAttachment(index) }
          }
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
        textFormat:TextEdit.RichText
        LayoutMirroring.enabled:false
        objectName: "t3ComposerEditor"
        // The draft's text as plain text (chips as references), for keyboard handling and tests.
        readonly property string plainText: root.viewText
        Component.onCompleted: root.renderEditor()
        placeholderText: T3I18n.tr("Ask anything, @files/folders, $skills or /commands…")
        color: root.fg; placeholderTextColor: root.dim
        selectionColor: Color.accent; selectedTextColor: Color.popups.background
        font.pixelSize: Style.font.body
        wrapMode: TextEdit.Wrap; selectByMouse: true
        readOnly: root.locked
        background: Item {}
        onTextChanged: { if (root.syncingEditor) return; var source=root.editorEdited(); if (source !== root.draft.text && activeFocus && !root.locked) {root.historyIndex=-1;root.bridge.edit(source,true)} else if (source !== root.draft.text) Qt.callLater(root.renderEditor);completionDelay.restart() }
        onSelectionStartChanged:if(!root.draft.loading)root.bridge.rememberDraftCursor(root.sourcePosition(selectionStart),root.sourcePosition(selectionEnd))
        onSelectionEndChanged:if(!root.draft.loading)root.bridge.rememberDraftCursor(root.sourcePosition(selectionStart),root.sourcePosition(selectionEnd))
        onCursorPositionChanged: {if(!root.draft.loading)root.bridge.rememberDraftCursor(root.sourcePosition(selectionStart),root.sourcePosition(selectionEnd));completionDelay.restart();Qt.callLater(editorViewport.revealCursor)}
        onCursorRectangleChanged:Qt.callLater(editorViewport.revealCursor)
        Keys.onReleased:function(event){if(event.key===Qt.Key_Control)root.editorCtrlHeld=false;if(root.dictationKey(event,false))event.accepted=true}
        Keys.onPressed:function(event){if(root.bridge.releaseThreadUndoShortcut&&event.key!==Qt.Key_Z&&[Qt.Key_Control,Qt.Key_Meta,Qt.Key_Shift].indexOf(event.key)<0)root.bridge.releaseThreadUndoShortcut();if(event.key===Qt.Key_Control)root.editorCtrlHeld=true;if(root.dictationKey(event,true)){event.accepted=true;return}if(event.modifiers===Qt.ShiftModifier&&event.key===Qt.Key_Backtab&&root.bridge.interactionModeEnabled===true&&!root.locked){root.bridge.chooseInteractionMode(root.bridge.interactionMode==="plan"?"default":"plan");event.accepted=true;return}if(event.modifiers===Qt.MetaModifier && event.key===Qt.Key_A){editor.selectAll();event.accepted=true;return}if(event.matches(StandardKey.Undo)||((event.modifiers===Qt.MetaModifier||event.modifiers===Qt.ControlModifier) && event.key===Qt.Key_Z)){root.bridge.undoDraft(false);event.accepted=true;return}if(event.matches(StandardKey.Redo)||((event.modifiers===Qt.MetaModifier||event.modifiers===Qt.ControlModifier) && event.key===Qt.Key_Y)||((event.modifiers===(Qt.MetaModifier|Qt.ShiftModifier)||event.modifiers===(Qt.ControlModifier|Qt.ShiftModifier))&&event.key===Qt.Key_Z)){root.bridge.undoDraft(true);event.accepted=true;return}if(event.matches(StandardKey.Paste)){event.accepted=true;if(!root.locked)root.bridge.pasteClipboard(root.sourcePosition(editor.selectionStart),root.sourcePosition(editor.selectionEnd))}}
        onActiveFocusChanged: { if (!activeFocus) {root.editorCtrlHeld=false;root.bridge.flushDrafts()} else completionDelay.restart() }
        Keys.onUpPressed: function(event) { event.accepted = root.completionKey(event,-1) || (event.modifiers === Qt.NoModifier && root.navigateHistory(-1)) }
        Keys.onDownPressed: function(event) { event.accepted = root.completionKey(event,1) || (event.modifiers === Qt.NoModifier && root.navigateHistory(1)) }
        Keys.onTabPressed: function(event) {event.accepted=root.completionVisible;if(event.accepted)root.chooseCompletion(root.completionIndex)}
        Keys.onEscapePressed: function(event) {if(["starting","recording","transcribing"].indexOf(root.bridge.dictation.state)>=0){root.bridge.cancelDictation();event.accepted=true;return}event.accepted=root.completionVisible;if(event.accepted)root.closeCompletion()}
        KeyNavigation.priority: KeyNavigation.BeforeItem
        KeyNavigation.tab: expandEditorButton
        Keys.onReturnPressed: function(event) {root.handleEnter(event)}
        Keys.onEnterPressed: function(event) {root.handleEnter(event)}
      }
    }
    T3ModelOptions {id:modelOptions;Layout.fillWidth:true;visible:root.showOptions&&!root.draft.queueEdit;bridge:root.bridge;descriptors:root.descriptors;selection:root.selection;locked:root.locked}
    Text {
      Layout.fillWidth: true
      visible: !!text
      text: root.draft.error ? (root.draft.error === "rpc_outcome_unknown" ? T3I18n.tr("No confirmation from T3. Retry the same submission; it will not duplicate.") : T3I18n.error(root.draft.error))
        : root.invalidFiles ? T3I18n.tr("An upload failed. Remove it and attach it again.")
        : root.bridge.screenshot.error ? T3I18n.tr(root.bridge.screenshot.error)
        : root.bridge.dictation.error ? T3I18n.tr(root.bridge.dictation.error)
        : root.bridge.dictationDownload.error ? T3I18n.tr(root.bridge.dictationDownload.error)
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
      T3IconButton {
        objectName: "t3ComposerScreenshot"
        visible: root.bridge.screenshotEnabled
        name: "camera"; tint: root.bridge.screenshot.busy ? Color.accent : root.fg
        tooltipText: T3I18n.tr(root.bridge.screenshot.busy ? "Cancel screenshot" : "Take a screenshot")
        enabled: !root.locked && !!root.bridge.selectedThread
        onClicked: root.bridge.captureScreenshot()
      }
      T3IconButton {
        id:quickButton;objectName:"t3QuickRepliesButton"
        readonly property var menu:quickPopup
        name:"sparkles";tooltipText:T3I18n.tr("Quick replies")
        visible:root.bridge.viewState.quickRepliesButton!==false
        enabled:!root.locked&&!!root.bridge.selectedThread
        readonly property string language:QuickReplies.language(root.bridge.draft.text,root.bridge.transcript,T3I18n.language,root.bridge.viewState.quickReplyLanguage)
        readonly property var replies:QuickReplies.replies(language,root.bridge.viewState.quickReplies)
        tint:hovered||quickPopup.visible?Color.accent:root.fg
        scale:hovered?1.12:1
        Behavior on scale {NumberAnimation{duration:180;easing.type:Easing.OutBack}}
        Rectangle {
          anchors.fill:parent;anchors.margins:-Style.space(2);radius:Style.space(9);color:"transparent"
          border.width:1;border.color:Color.accent;visible:quickButton.hovered||quickPopup.visible
          SequentialAnimation on opacity {running:quickButton.visible&&(quickButton.hovered||quickPopup.visible);loops:Animation.Infinite;NumberAnimation{from:0.15;to:0.65;duration:650} NumberAnimation{from:0.65;to:0.15;duration:650}}
        }
        Timer{interval:350;running:quickButton.hovered&&!quickPopup.visible;onTriggered:quickPopup.open()}
        Timer{interval:500;running:quickPopup.visible&&!quickPopup.activeFocus&&!quickButton.hovered&&!quickHover.hovered;onTriggered:quickPopup.close()}
        onClicked:{if(quickPopup.visible)quickPopup.close();else{quickPopup.open();quickPopup.forceActiveFocus()}}
        Controls.Popup {
          id:quickPopup;objectName:"t3QuickRepliesPopup";parent:quickButton;y:-height-Style.space(6)
          width:Style.space(260);padding:Style.space(8);height:Math.min(implicitHeight,Style.space(350))
          closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
          background:Rectangle{radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Color.accent}
          enter:Transition{NumberAnimation{property:"opacity";from:0;to:1;duration:160}}
          contentItem:ColumnLayout {
            HoverHandler{id:quickHover}
            Text{text:T3I18n.tr("Quick replies")+" · "+quickButton.language.toUpperCase();color:Color.popups.text;font.pixelSize:Style.font.caption}
            ListView {
              Layout.fillWidth:true;Layout.preferredHeight:Math.min(contentHeight,Style.space(290));clip:true
              model:quickButton.replies
              Controls.ScrollBar.vertical:Controls.ScrollBar{}
              delegate:Controls.ItemDelegate {
                required property string modelData
                required property int index
                objectName:"t3QuickReply_"+index
                width:ListView.view.width
                contentItem:Text{text:modelData;textFormat:Text.PlainText;wrapMode:Text.Wrap;color:Color.popups.text;font.pixelSize:Style.font.body}
                onClicked:{root.bridge.insertDictation(root.bridge.selectedKey,modelData);quickPopup.close();editor.forceActiveFocus()}
              }
            }
          }
        }
      }
      T3IconButton {
        id: dictateButton
        objectName: "t3ComposerDictate"
        visible: root.bridge.dictationEnabled
        readonly property string mode: root.bridge.dictationDownload.busy ? "downloading" : root.bridge.dictation.state
        readonly property bool listening: mode === "starting" || mode === "recording"
        readonly property var model: root.bridge.dictationModelInfo || ({})
        name: "mic"; tint: listening ? Color.urgent : root.fg
        tooltipText: mode === "downloading" ? T3I18n.tr("Downloading the dictation model… {percent}% · click to cancel",{percent:Math.floor(100*root.bridge.dictationDownload.bytes/Math.max(1,root.bridge.dictationDownload.total))})
          : listening ? T3I18n.tr("Stop and insert the text") + " · Ctrl+Space · " + T3I18n.tr("Esc cancels")
          : mode === "transcribing" ? T3I18n.tr("Transcribing…")
          : !model.installed ? T3I18n.tr("Download the dictation model ({size}), then dictate",{size:root.sizeText(model.sizeBytes)})
          : T3I18n.tr("Dictate") + " · " + T3I18n.tr("Ctrl+Space: hold to talk, tap to start and stop")
        enabled: mode === "downloading" || listening || (mode === "idle" && (!model.installed || (!root.locked && !!root.bridge.selectedThread)))
        onClicked: mode === "downloading" ? root.bridge.cancelDictationDownload() : root.bridge.toggleDictation()
        // Resting the mouse on the microphone lists the recordings kept for this
        // thread, so the audio behind a lost transcription is a click away.
        readonly property bool keepsRecordings: root.bridge.dictationRecordingsPolicy !== "off"
        readonly property var recordings: recordingsPopup
        Timer { interval: 900; running: dictateButton.hovered && dictateButton.mode === "idle" && dictateButton.keepsRecordings && !recordingsPopup.visible; onTriggered: { root.bridge.refreshDictationRecordings(); recordingsPopup.open() } }
        Timer { interval: 450; running: recordingsPopup.visible && !dictateButton.hovered && !recordingsHover.hovered; onTriggered: recordingsPopup.close() }
        Controls.Popup {
          id: recordingsPopup
          objectName: "t3ComposerRecordings"
          readonly property var items: root.bridge.dictationRecordings.key === root.bridge.selectedKey ? (root.bridge.dictationRecordings.items || []).slice(0, 8) : []
          readonly property color faint: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.6)
          function when(ms) { var d = new Date(Number(ms)), now = new Date(); return d.toDateString() === now.toDateString() ? Qt.formatTime(d, "HH:mm") : Qt.formatDateTime(d, "d MMM HH:mm") }
          function length(s) { var n = Math.round(Number(s) || 0); return Math.floor(n / 60) + ":" + ("0" + n % 60).slice(-2) }
          parent: dictateButton; x: 0; y: -height - Style.space(6)
          width: Style.space(320); padding: Style.space(8); modal: false
          closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside
          background: Rectangle { color: Color.popups.background; radius: Style.space(9); border.width: 1; border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.15) }
          contentItem: Column {
            spacing: Style.space(2)
            HoverHandler { id: recordingsHover }
            Text { width: parent.width; leftPadding: Style.space(6); text: T3I18n.tr("Recordings of this thread"); color: recordingsPopup.faint; font.pixelSize: Style.font.caption; textFormat: Text.PlainText }
            Text { width: parent.width; padding: Style.space(6); visible: recordingsPopup.items.length === 0; text: T3I18n.tr("No recordings kept for this thread yet."); color: root.fg; font.pixelSize: Style.font.body; wrapMode: Text.WordWrap; textFormat: Text.PlainText }
            Repeater {
              model: recordingsPopup.items
              Rectangle {
                id: recordingRow
                required property var modelData
                required property int index
                objectName: "t3ComposerRecording_" + index
                width: parent.width; height: Style.space(30); radius: Style.space(6)
                color: rowMouse.containsMouse ? Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.07) : "transparent"
                MouseArea { id: rowMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.bridge.openDictationRecording(recordingRow.modelData.file) }
                Controls.ToolTip.visible: rowMouse.containsMouse; Controls.ToolTip.delay: 700
                Controls.ToolTip.text: T3I18n.tr("Play the recording · the text went into the draft")
                Row {
                  anchors.fill: parent; anchors.leftMargin: Style.space(6); spacing: Style.space(8)
                  T3Icon { name: "play"; tint: Color.accent; anchors.verticalCenter: parent.verticalCenter; implicitWidth: Style.space(13) }
                  Text { text: recordingsPopup.when(recordingRow.modelData.time); color: recordingsPopup.faint; font.pixelSize: Style.font.caption; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText }
                  Text { text: recordingsPopup.length(recordingRow.modelData.seconds); color: recordingsPopup.faint; font.pixelSize: Style.font.caption; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText }
                  Text { width: recordingRow.width - Style.space(150); text: String(recordingRow.modelData.text || "").trim() || "…"; color: root.fg; font.pixelSize: Style.font.body; elide: Text.ElideRight; maximumLineCount: 1; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText }
                }
                T3IconButton { name: "trash-2"; implicitWidth: Style.space(22); implicitHeight: implicitWidth; anchors.right: parent.right; anchors.rightMargin: Style.space(4); anchors.verticalCenter: parent.verticalCenter; tint: root.fg; visible: rowMouse.containsMouse || hovered; tooltipText: T3I18n.tr("Delete recording"); onClicked: root.bridge.deleteDictationRecording(recordingRow.modelData.id) }
              }
            }
            Rectangle { width: parent.width; height: Style.space(8); color: "transparent"; visible: recordingsPopup.items.length > 0 }
            Rectangle {
              width: parent.width; height: Style.space(26); radius: Style.space(6); visible: recordingsPopup.items.length > 0
              color: folderMouse.containsMouse ? Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.07) : "transparent"
              Row { anchors.fill: parent; anchors.leftMargin: Style.space(6); spacing: Style.space(8)
                T3Icon { name: "folder"; tint: recordingsPopup.faint; anchors.verticalCenter: parent.verticalCenter; implicitWidth: Style.space(13) }
                Text { text: T3I18n.tr("Open folder"); color: recordingsPopup.faint; font.pixelSize: Style.font.caption; anchors.verticalCenter: parent.verticalCenter; textFormat: Text.PlainText }
              }
              MouseArea { id: folderMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.bridge.openDictationRecordings() }
            }
          }
        }
        // The ring follows the microphone level while recording.
        Rectangle {
          anchors.centerIn: parent; z: -1
          width: parent.width * (0.75 + 0.45 * root.bridge.dictation.level); height: width; radius: width / 2
          color: Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.18)
          visible: dictateButton.listening
          Behavior on width { NumberAnimation { duration: 70 } }
        }
        WorkIndicator { anchors.centerIn: parent; implicitWidth: Style.space(26); visible: dictateButton.mode === "transcribing" || dictateButton.mode === "downloading"; active: visible && root.visible; tint: Color.accent }
        // While it records, a bubble grows out of the microphone with the last two
        // seconds of sound, newest on the right, measured from the room's noise:
        // flat while nothing is heard, tall bars for a voice. Two drops rise first,
        // then the bubble stretches out of a circle and settles with a small wobble.
        Item {
          id: dictationWave
          objectName: "t3ComposerDictationWave"
          readonly property bool shown: dictateButton.visible && dictateButton.listening
          readonly property var levels: root.bridge.dictation.levels || []
          readonly property int count: root.bridge.dictationWaveLength
          readonly property real floor: Math.min(0.5, levels.reduce(function(low, v) { return Math.min(low, v) }, 1))
          function bar(v) { return Math.max(0, Math.min(1, (v - floor) / Math.max(0.2, 0.85 - floor))) }
          function clamp(v) { return Math.max(0, Math.min(1, v)) }
          // Pauses can arrive as exact zeros (a mic's noise gate), so only a muted
          // source replaces the wave.
          readonly property bool muted: root.bridge.dictation.muted === true
          readonly property string notice: muted ? T3I18n.tr("Microphone muted · click to turn it on") : ""
          property real startedAt: 0
          property int seconds: 0
          onShownChanged: if (shown) { startedAt = Date.now(); seconds = 0 }
          Timer { interval: 250; repeat: true; running: dictationWave.shown; onTriggered: dictationWave.seconds = Math.floor((Date.now() - dictationWave.startedAt) / 1000) }
          // 0 → 1 drives the drops, then the bubble; it overshoots a little on the way in.
          property real grow: shown ? 1 : 0
          Behavior on grow { NumberAnimation { duration: dictationWave.shown ? 520 : 220; easing.type: dictationWave.shown ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.6 } }
          readonly property real morph: Math.max(0, (grow - 0.3) / 0.7)
          readonly property real tone: 0.35 + 0.65 * root.bridge.dictation.level
          readonly property color red: Color.urgent
          readonly property real full: notice ? noticeText.implicitWidth + Style.space(32) : waveContent.implicitWidth + Style.space(28)
          readonly property real tall: Style.space(34)
          x: 0; width: full; height: tall + Style.space(18); z: 10
          anchors.bottom: parent.top; anchors.bottomMargin: -Style.space(2)
          visible: grow > 0.01
          // The drops between the microphone and the bubble.
          Repeater {
            model: 2
            Rectangle {
              readonly property real size: Style.space(index ? 8 : 5)
              readonly property real pop: dictationWave.clamp(dictationWave.grow * 4 - index * 0.7)
              x: dictateButton.width / 2 - width / 2 + index * Style.space(2)
              y: dictationWave.height - Style.space(index ? 13 : 6) - height / 2
              width: size * pop; height: width; radius: width / 2
              color: Qt.rgba(dictationWave.red.r, dictationWave.red.g, dictationWave.red.b, 0.55)
              opacity: pop
            }
          }
          // A soft glow that breathes with the voice.
          Rectangle {
            anchors.fill: waveBubble; anchors.margins: -Style.space(3)
            radius: height / 2; opacity: waveBubble.opacity
            color: Qt.rgba(dictationWave.red.r, dictationWave.red.g, dictationWave.red.b, 0.04 + 0.14 * root.bridge.dictation.level)
            Behavior on color { ColorAnimation { duration: 120 } }
          }
          Rectangle {
            id: waveBubble
            readonly property real m: dictationWave.morph
            // Out of a circle over the microphone into the full width, squashing as it stretches.
            width: dictationWave.tall + (dictationWave.full - dictationWave.tall) * m
            height: dictationWave.tall * (1 - 0.12 * Math.max(0, m - 1) / 0.15)
            x: (dictateButton.width / 2 - dictationWave.tall / 2) * Math.max(0, 1 - m)
            y: (dictationWave.tall - height) / 2
            radius: height / 2
            opacity: dictationWave.clamp(m * 2.5)
            color: Color.popups.background
            border.width: 1; border.color: Qt.rgba(dictationWave.red.r, dictationWave.red.g, dictationWave.red.b, 0.3 + 0.4 * root.bridge.dictation.level)
            Behavior on border.color { ColorAnimation { duration: 120 } }
            clip: true
            Text {
              id: noticeText
              objectName: "t3ComposerDictationNotice"
              anchors.centerIn: parent; visible: !!dictationWave.notice
              opacity: dictationWave.clamp((waveBubble.m - 0.6) / 0.4)
              text: dictationWave.notice; color: dictationWave.red; font.pixelSize: Style.font.caption; textFormat: Text.PlainText
            }
            MouseArea {
              anchors.fill: parent; enabled: dictationWave.muted; cursorShape: Qt.PointingHandCursor
              onClicked: root.bridge.unmuteMicrophone()
            }
            Row {
              id: waveContent
              visible: !dictationWave.notice
              anchors.centerIn: parent; spacing: Style.space(10)
              opacity: dictationWave.clamp((waveBubble.m - 0.6) / 0.4)
              Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Style.space(7); height: width; radius: width / 2; color: dictationWave.red
                SequentialAnimation on opacity {
                  running: dictationWave.shown; loops: Animation.Infinite
                  NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
                  NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                }
              }
              Row {
                anchors.verticalCenter: parent.verticalCenter
                height: dictationWave.tall - Style.space(14); spacing: Style.space(2)
                Repeater {
                  model: dictationWave.count
                  Rectangle {
                    readonly property int at: index - (dictationWave.count - dictationWave.levels.length)
                    readonly property real value: at >= 0 ? dictationWave.bar(dictationWave.levels[at]) : 0
                    anchors.verticalCenter: parent.verticalCenter
                    width: Style.space(3); radius: width / 2
                    height: Math.max(width, parent.height * value)
                    color: Qt.rgba(dictationWave.red.r, dictationWave.red.g, dictationWave.red.b, 0.25 + 0.75 * value)
                    Behavior on height { NumberAnimation { duration: 120; easing.type: Easing.OutQuad } }
                  }
                }
              }
              Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Math.floor(dictationWave.seconds / 60) + ":" + ("0" + dictationWave.seconds % 60).slice(-2)
                color: root.dim; font.pixelSize: Style.font.caption; font.features: { "tnum": 1 }; textFormat: Text.PlainText
              }
            }
          }
        }
      }
      T3ModelPicker {
        id:modelPicker;objectName:"t3ComposerModel"
        Layout.fillWidth:true;Layout.maximumWidth:Style.space(300)
        bridge:root.bridge;selection:root.selection
        enabled:!root.locked && !root.draft.queueEdit
        onPicked:editor.forceActiveFocus()
      }
      T3PermissionPicker {
        id:permissionPicker
        bridge:root.bridge
        enabled:!root.locked&&!root.draft.queueEdit&&root.bridge.catalog.canWrite!==false
        onPicked:editor.forceActiveFocus()
      }
      T3PickerButton{objectName:"t3InteractionMode";primary:false;implicitHeight:Style.space(28);leftPadding:Style.space(8);rightPadding:leftPadding;visible:root.bridge.interactionModeEnabled===true;text:T3I18n.tr(root.bridge.interactionMode==="plan"?"Plan":"Build");enabled:!root.locked&&!root.draft.queueEdit&&root.bridge.catalog.canWrite!==false;onClicked:root.bridge.chooseInteractionMode(root.bridge.interactionMode==="plan"?"default":"plan")}
      T3BranchPicker { id:branchPicker;bridge:root.bridge;Layout.maximumWidth:Style.space(190) }
      T3ContextMeter { objectName:"t3ComposerContext";usage:root.bridge.contextWindow }
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
        // Saving and saved cross-fade: the check pops in as the spinner fades.
        T3Icon{
          id:draftSavedIcon;anchors.centerIn:parent;name:draftStatus.failed?"circle-alert":"check";tint:draftStatus.failed?Color.urgent:root.dim;implicitWidth:Style.space(14)
          readonly property bool shown:!draftStatus.saving || draftStatus.failed
          opacity:shown?1:0;scale:shown?1:0.35;visible:opacity>0
          Behavior on opacity{NumberAnimation{duration:draftSavedIcon.shown?240:140;easing.type:Easing.OutCubic}}
          Behavior on scale{NumberAnimation{duration:draftSavedIcon.shown?420:140;easing.type:draftSavedIcon.shown?Easing.OutBack:Easing.InCubic;easing.overshoot:2.2}}
        }
        WorkIndicator{
          id:draftSavingIcon;anchors.centerIn:parent;implicitWidth:Style.space(14);tint:root.dim
          readonly property bool shown:draftStatus.saving&&!draftStatus.failed
          opacity:shown?1:0;visible:opacity>0;active:visible&&root.visible
          Behavior on opacity{NumberAnimation{duration:draftSavingIcon.shown?160:260;easing.type:Easing.InOutSine}}
        }
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
        tooltipText:(root.expanded?T3I18n.tr("Back to chat"):T3I18n.tr("Edit"))+" · "+(root.expanded?"Esc":"Super+E")+" · "+T3I18n.tr("Right-click: project drafts")
        onClicked:root.toggleExpanded()
        MouseArea{anchors.fill:parent;acceptedButtons:Qt.RightButton;onClicked:root.toggleProjectDrafts()}
      }
      T3IconButton {
        objectName:"t3SendAndNewThread"
        name:"git-branch"
        tooltipText:T3I18n.tr("Send and start new thread")+" · Ctrl+Shift+Enter"
        enabled:!root.bridge.rewindPending&&!root.locked&&!root.draft.queueEdit&&!root.bridge.activeQuestion&&!root.bridge.managementBusy&&!root.bridge.managementRequest&&!!root.bridge.selectedInfo.projectId&&(!!root.draft.text.trim()||root.draft.attachments.length>0)
        onClicked:root.bridge.submit(true)
      }
      Rectangle {
        id: primaryAction
        objectName: "t3PrimaryAction"
        property string actionMode: root.stopMode ? "stop" : root.draft.queueEdit ? "edit" : root.running ? root.followUp : "send"
        property string iconName: root.stopMode ? "stop-filled" : root.draft.attempt ? "refresh-cw" : root.draft.queueEdit ? "check" : root.running ? (root.followUp === "steer" ? "corner-up-right" : "list-plus") : "arrow-up"
        activeFocusOnTab: sendMouse.enabled
        Accessible.role: Accessible.Button
        Accessible.name: root.stopMode ? T3I18n.tr("Stop execution and hold queue") : root.draft.queueEdit ? T3I18n.tr("Save queued message") : root.running ? T3I18n.tr(root.followUp === "steer" ? "Steer the active run" : "Queue message") : T3I18n.tr("Send message")
        function activate(alternate) { if(sendMouse.enabled) { if(root.stopMode) root.bridge.stop(); else root.bridge.submit(false, undefined, alternate === undefined ? root.alternateHeld : alternate) } }
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
          enabled: root.stopMode ? !root.draft.stopping : !root.bridge.rewindPending && !root.draft.loading && !root.draft.loadError && !root.draft.busy && !root.uploading && !root.invalidFiles && root.hasContent
          onClicked: function(mouse) { primaryAction.activate((mouse.modifiers & Qt.ControlModifier) !== 0) }
        }
        Controls.ToolTip.visible: sendHover.hovered
        Controls.ToolTip.text: primaryAction.Accessible.name + (root.stopMode ? "" : root.expanded ? " · Ctrl+Enter" : " · Enter") + (root.running && !root.stopMode && !root.draft.queueEdit ? "\n" + T3I18n.tr(root.followUp === "steer" ? "Ctrl-click or Ctrl+Enter to queue instead" : "Ctrl-click or Ctrl+Enter to steer instead") : "")
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
