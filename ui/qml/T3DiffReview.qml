import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// A read-only checkpoint review. The host supplies parsed, bounded native RPC data on click.
Item {
 id: root
 objectName: "t3DiffReview"
 required property var view // {loading,error,data}; data is diff_review::present's result
 required property var summary
 property var rendererBridge:null
 readonly property bool managedLayout:!!rendererBridge&&typeof rendererBridge.setDiffLayout==="function"
 readonly property bool preferenceBusy:managedLayout&&!!rendererBridge.clientSettings.busy
 readonly property string preferenceError:managedLayout?(rendererBridge.diffPreferenceUpdate.error || ""):""
 property string richSessionId:""
 property string richSnapshotId:""
 property string richPath:""
 property string sentOptions:""
 property string refreshedSnapshot:""
 readonly property bool nativeRenderer:!!rendererBridge&&!!file&&!!file.reviewSnapshotId
 readonly property var nativeDocument:rendererBridge&&richSessionId?(rendererBridge.inlineDocuments[richSessionId] || {}):({})
 function closeNative(){if(richSessionId&&rendererBridge)rendererBridge.closeInlineDocument(richSessionId);richSessionId="";richSnapshotId="";richPath="";sentOptions=""}
 function openNative(){
  if(!visible||!nativeRenderer){closeNative();return}
  if(richSnapshotId===file.reviewSnapshotId&&richPath===file.path)return
  closeNative();richSnapshotId=file.reviewSnapshotId;richPath=file.path;richSessionId=rendererBridge.openReviewDocument(richSnapshotId,split?"split":"stacked",wordWrap,richPath)
 }
 function updateNativeOptions(){
  if(!richSessionId||!nativeDocument.frameUrl||nativeDocument.busy||!nativeDocument.rendererReady)return
  var signature=richSessionId+":"+diffLayout+":"+wordWrap
  if(signature===sentOptions)return
  sentOptions=signature;rendererBridge.send({op:"document-input",requestId:richSessionId,event:{type:"review-options",layout:split?"split":"stacked",wrap:wordWrap}})
 }
 Timer{id:openNativeDelay;interval:0;onTriggered:root.openNative()}
 onNativeDocumentChanged:{
  if(nativeDocument.rendererReady)updateNativeOptions()
  if(nativeDocument.error==="review_snapshot_expired"&&file&&file.reviewSnapshotId!==refreshedSnapshot){
   var path=file.path,snapshot=file.reviewSnapshotId;refreshedSnapshot=snapshot
   Qt.callLater(function(){if(root.file&&root.file.reviewSnapshotId===snapshot)root.fileRequested(path)})
  }
 }
 onFileChanged:openNativeDelay.restart()
 onRendererBridgeChanged:openNativeDelay.restart()
 onVisibleChanged:if(visible)openNativeDelay.restart();else closeNative()
 onDiffLayoutChanged:updateNativeOptions()
 onWordWrapChanged:updateNativeOptions()
 onSplitChanged:{sentOptions="";updateNativeOptions()}
 Component.onDestruction:closeNative()
 property string initialPath: ""
 property string selectedPath: initialPath
 property string diffLayout: "stacked" // native settings: stacked | split
 property bool wordWrap: true
 property bool ignoreWhitespace: true // parent refetches the native RPC when toggled
 property bool showContext: true
 property bool treeOpen: false
 property var foldedHunks: ({})
 signal fileRequested(string path)
 signal closed()
 signal retryRequested()
 signal whitespaceChanged(bool ignoreWhitespace)
 readonly property var files: view && view.data && view.data.files || []
 readonly property int selectedIndex: {var exact=files.findIndex(function(f){return f.path===selectedPath});return exact>=0?exact:files.findIndex(function(f){return f.oldPath===selectedPath})}
 readonly property var file: selectedIndex>=0 ? files[selectedIndex] : null
 readonly property bool narrow: width < Style.space(650)
 readonly property bool split: diffLayout==="split" && !narrow
 readonly property color fg: Color.popups.text
 readonly property color dim: Qt.rgba(fg.r,fg.g,fg.b,0.58)
 readonly property color border: Qt.rgba(fg.r,fg.g,fg.b,0.12)
 readonly property var visibleRows: {
  var input=file ? (split ? file.splitRows || [] : file.rows || []) : []
  var output=[], hunk=-1
  for(var i=0;i<input.length;i++) {
   var line=input[i],kind=split?(line.left || line.right || {}).kind:line.kind
   if(kind==="hunk")hunk=i
   if(i!==hunk && foldedHunks[hunk])continue
   if(!showContext && kind==="context")continue
   output.push({row:line,index:i,kind:kind})
  }
  return output
 }
 FontMetrics{id:codeMetrics;font.family:"monospace";font.pixelSize:Style.font.caption}
 readonly property real noWrapWidth:{
  if(wordWrap)return width
  var longest=0
  for(var i=0;i<visibleRows.length;i++){
   var row=visibleRows[i].row
   var texts=split?[row.left&&row.left.text || "",row.right&&row.right.text || ""]:[row.text || ""]
   for(var j=0;j<texts.length;j++)longest=Math.max(longest,codeMetrics.advanceWidth(texts[j]))
  }
  return (longest+Style.space(62))*(split?2:1)
 }
 Timer{id:requestFile;interval:0;onTriggered:if(root.file&&root.file.pending&&!root.view.loading&&!root.view.error)root.fileRequested(root.file.path)}
 onViewChanged:requestFile.restart()
 function selectFile(path){
  var next=files.find(function(f){return f.path===path}) || files.find(function(f){return f.oldPath===path})
  if(!next)return false
  selectedPath=next.path;requestFile.restart();foldedHunks=({});lines.positionViewAtBeginning();return true
 }
 function moveFile(step){if(!files.length)return;var i=selectedIndex<0?0:Math.max(0,Math.min(files.length-1,selectedIndex+step));selectFile(files[i].path)}
 function foldHunk(index){var next=Object.assign({},foldedHunks);next[index]=!next[index];foldedHunks=next}
 function hue(kind){return kind==="addition"?T3Palette.done:kind==="deletion"?Color.urgent:kind==="hunk"?Color.accent:root.dim}
 function shade(kind){var c=hue(kind);return kind==="addition"||kind==="deletion"||kind==="hunk"?Qt.rgba(c.r,c.g,c.b,0.08):"transparent"}
 onInitialPathChanged: { if(initialPath)selectFile(initialPath) }
 onFilesChanged: {
  requestFile.restart()
  if(!files.length){selectedPath="";foldedHunks=({});return}
  if(!files.some(function(f){return f.path===selectedPath || f.oldPath===selectedPath}))selectFile(initialPath || files[0].path)
 }
 activeFocusOnTab:true
 Keys.onPressed:function(event){
  if(event.key===Qt.Key_Escape){closed();event.accepted=true}
  else if(event.key===Qt.Key_Left && event.modifiers===Qt.ControlModifier){moveFile(-1);event.accepted=true}
  else if(event.key===Qt.Key_Right && event.modifiers===Qt.ControlModifier){moveFile(1);event.accepted=true}
  else if(event.key===Qt.Key_PageDown){lines.flick(0,-Style.space(500));event.accepted=true}
  else if(event.key===Qt.Key_PageUp){lines.flick(0,Style.space(500));event.accepted=true}
 }
 ColumnLayout {
  anchors.fill:parent;spacing:Style.space(8)
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(6)
   T3Icon{name:"file";tint:Color.accent;implicitWidth:Style.space(16)}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Changed files");color:root.fg;font.pixelSize:Style.font.body;font.bold:true}
   Text{objectName:"t3DiffTotals";visible:root.files.length>0;text:root.files.length+" · +"+((root.view.data || {}).additions || 0)+" −"+((root.view.data || {}).deletions || 0);color:root.dim;font.pixelSize:Style.font.caption}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.closed()}
  }
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(5)
   T3IconButton{name:"chevron-left";tooltipText:T3I18n.tr("Previous file");enabled:root.selectedIndex>0;onClicked:root.moveFile(-1)}
   Text{Layout.fillWidth:true;text:root.file?root.file.path:T3I18n.tr("No patch available for this selection.");elide:Text.ElideMiddle;color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   T3IconButton{name:"chevron-right";tooltipText:T3I18n.tr("Next file");enabled:root.selectedIndex>=0&&root.selectedIndex<root.files.length-1;onClicked:root.moveFile(1)}
   T3IconButton{name:"folder";tooltipText:T3I18n.tr(root.treeOpen?"Hide file tree":"Show file tree");visible:!root.narrow;onClicked:root.treeOpen=!root.treeOpen}
  }
  Flow {
   Layout.fillWidth:true;spacing:Style.space(5)
   T3PickerButton{primary:false;text:T3I18n.tr(root.diffLayout==="split"?"Unified":"Split");objectName:"t3DiffLayout";onClicked:{var value=root.diffLayout==="split"?"stacked":"split";if(root.managedLayout)root.rendererBridge.setDiffLayout(value);else root.diffLayout=value}enabled:!root.narrow&&!root.preferenceBusy}
   T3PickerButton{objectName:"t3DiffWrap";primary:false;text:T3I18n.tr(root.wordWrap?"No wrap":"Wrap");onClicked:root.wordWrap=!root.wordWrap}
   T3PickerButton{primary:false;visible:!root.nativeRenderer;text:T3I18n.tr(root.showContext?"Hide context":"Show context");onClicked:root.showContext=!root.showContext}
   T3PickerButton{objectName:"t3DiffWhitespace";primary:false;text:T3I18n.tr(root.ignoreWhitespace?"Show whitespace changes":"Hide whitespace changes");onClicked:root.whitespaceChanged(!root.ignoreWhitespace)}
  }
  Text{Layout.fillWidth:true;visible:!!root.preferenceError;text:T3I18n.error(root.preferenceError);color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  T3PickerButton{objectName:"t3DiffPreferenceRetry";primary:false;visible:!!root.preferenceError;text:T3I18n.tr("Retry");enabled:!root.preferenceBusy;onClicked:root.rendererBridge.setDiffLayout(root.diffLayout)}
  Text{Layout.fillWidth:true;visible:!!root.view.error;text:T3I18n.error(root.view.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  T3PickerButton{primary:false;visible:!!root.view.error;text:T3I18n.tr("Retry");onClicked:root.retryRequested()}
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.loading;active:visible}
  RowLayout {
   Layout.fillWidth:true;Layout.fillHeight:true;spacing:Style.space(8)
   Rectangle {
    visible:root.treeOpen&&!root.narrow
    Layout.preferredWidth:Math.min(Style.space(245),root.width*0.36);Layout.fillHeight:true
    color:Color.popups.background;radius:Style.space(6);border.width:1;border.color:root.border
    ColumnLayout{
     anchors.fill:parent;anchors.margins:Style.space(5);spacing:Style.space(4)
     Text{text:T3I18n.tr("Files")+" · "+root.files.length;color:root.dim;font.pixelSize:Style.font.caption}
     ListView{
      id:tree;objectName:"t3DiffFiles";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;reuseItems:true;cacheBuffer:Style.space(70);model:root.files
      T3Wheel{flickable:tree}
      Controls.ScrollBar.vertical:Controls.ScrollBar{}
      delegate:Rectangle{
       id:fileRow;required property var modelData;required property int index
       width:tree.width;height:Style.space(36);radius:Style.space(5)
       color:root.selectedIndex===index?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.14):hover.containsMouse?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent"
       activeFocusOnTab:true;Accessible.role:Accessible.Button;Accessible.name:modelData.path
       Keys.onReturnPressed:root.selectFile(modelData.path)
       RowLayout{anchors.fill:parent;anchors.leftMargin:Style.space(5);anchors.rightMargin:Style.space(5);spacing:Style.space(5)
        Text{text:({added:"A",deleted:"D",renamed:"R",modified:"M"})[fileRow.modelData.kind] || "M";color:fileRow.modelData.kind==="deleted"?Color.urgent:T3Palette.done;font.pixelSize:Style.font.caption}
        Text{Layout.fillWidth:true;text:fileRow.modelData.path;color:root.fg;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
        Text{text:"+"+(fileRow.modelData.summaryAdditions ?? fileRow.modelData.additions ?? 0);color:T3Palette.done;font.pixelSize:Style.font.caption}
        Text{text:"−"+(fileRow.modelData.summaryDeletions ?? fileRow.modelData.deletions ?? 0);color:Color.urgent;font.pixelSize:Style.font.caption}
       }
       MouseArea{id:hover;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:{fileRow.forceActiveFocus();root.selectFile(fileRow.modelData.path)}}
      }
     }
    }
   }
   ColumnLayout{
    Layout.fillWidth:true;Layout.fillHeight:true;spacing:Style.space(4)
    Controls.ComboBox{visible:root.narrow&&root.files.length>0;Layout.fillWidth:true;model:root.files.map(function(f){return f.path});currentIndex:Math.max(0,root.selectedIndex);onActivated:function(i){root.selectFile(root.files[i].path)}}
    Text{Layout.fillWidth:true;visible:!!root.file;text:(root.file?root.file.oldPath!==root.file.path?root.file.oldPath+" → "+root.file.path:root.file.path:"")+"   +"+(root.file?root.file.additions || 0:0)+" −"+(root.file?root.file.deletions || 0:0);color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;visible:!!root.file&&!!root.file.missing;text:T3I18n.tr("Patch unavailable for this file.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    Text{Layout.fillWidth:true;visible:!!root.file&&!!root.file.binary;text:T3I18n.tr("Binary file — metadata only");color:root.dim;font.pixelSize:Style.font.caption}
    T3PickerButton{primary:false;visible:root.nativeRenderer&&!!root.nativeDocument.error;text:T3I18n.tr("Retry");onClicked:root.retryRequested()}
    Text{Layout.fillWidth:true;visible:root.nativeRenderer&&!!((root.nativeDocument.data || {}).contextError);text:T3I18n.error((root.nativeDocument.data || {}).contextError || "");color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
    Loader {
     Layout.fillWidth:true;Layout.fillHeight:true;active:root.nativeRenderer&&!!root.richSessionId;visible:active
     sourceComponent:T3DocumentView {
      bridge:root.rendererBridge;sessionId:root.richSessionId;inlineMode:true;chromeVisible:false;ownsWheel:true
      keyFilter:function(e){if(e.modifiers===Qt.ControlModifier&&(e.key===Qt.Key_Left||e.key===Qt.Key_Right)){root.moveFile(e.key===Qt.Key_Left?-1:1);e.accepted=true;return true}return false}
      onFrameReady:root.updateNativeOptions()
      onClosed:root.closed()
     }
    }
    ListView{
     visible:!root.nativeRenderer
     id:lines;objectName:"t3DiffLines";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;reuseItems:true;cacheBuffer:Style.space(120);model:root.visibleRows
     contentWidth:Math.max(width,root.noWrapWidth);flickableDirection:root.wordWrap?Flickable.VerticalFlick:Flickable.AutoFlickDirection
     Controls.ScrollBar.horizontal:Controls.ScrollBar{policy:root.wordWrap?Controls.ScrollBar.AlwaysOff:Controls.ScrollBar.AsNeeded}
     T3Wheel{flickable:lines}
     Controls.ScrollBar.vertical:Controls.ScrollBar{}
     delegate:Rectangle{
      id:line;required property var modelData
      width:lines.contentWidth;implicitHeight:Math.max(Style.space(21),codeRow.implicitHeight+Style.space(3));height:implicitHeight
      color:root.split?"transparent":root.shade(modelData.kind)
      RowLayout{id:codeRow;width:parent.width;spacing:Style.space(2)
       Repeater{model:root.split?2:1
        delegate:RowLayout{
         required property int index
         Layout.fillWidth:true;Layout.preferredWidth:root.split?line.width/2:line.width;spacing:Style.space(3)
         readonly property var item:root.split?(index===0?line.modelData.row.left:line.modelData.row.right):line.modelData.row
         Text{Layout.preferredWidth:Style.space(53);horizontalAlignment:Text.AlignRight;text:parent.item?(root.split?(index===0?parent.item.oldLine ?? "":parent.item.newLine ?? ""):(parent.item.oldLine ?? "")+" / "+(parent.item.newLine ?? "")):"";color:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption}
         TextEdit{Layout.fillWidth:true;text:parent.item?parent.item.text || "":"";readOnly:true;selectByMouse:true;textFormat:TextEdit.PlainText;wrapMode:root.wordWrap?TextEdit.WrapAnywhere:TextEdit.NoWrap;clip:true;color:root.hue(parent.item?parent.item.kind:"");font.family:"monospace";font.pixelSize:Style.font.caption}
        }
       }
      }
      MouseArea{anchors.fill:parent;enabled:line.modelData.kind==="hunk";cursorShape:Qt.PointingHandCursor;onClicked:root.foldHunk(line.modelData.index)}
     }
    }
   }
  }
 }
}
