import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Checkout sources are native review.getDiffPreview results; reuse the checkpoint viewer.
Item {
 id:root;objectName:"t3WorkingReview"
 property var rendererBridge:null
 required property var view
 property var refs:({query:"",local:{refs:[]},remote:{refs:[]}})
 signal refsRequested(string query,string kind,var cursor)
 signal refsDismissed()
 signal closed()
 signal requested(string kind,string base,bool whitespace,string path)
 function refresh(path){requested(view.sourceKind,view.baseRef,view.ignoreWhitespace,path || "")}
 ColumnLayout {
  anchors.fill:parent;spacing:Style.space(7)
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(6)
   T3Select {objectName:"t3ReviewSource";Layout.preferredWidth:Style.space(170);model:[T3I18n.tr("Uncommitted"),T3I18n.tr("Changes")];currentIndex:root.view.sourceKind==="branch-range"?1:0;popupAbove:false;onActivated:function(i){root.requested(i===0?"working-tree":"branch-range",root.view.baseRef,root.view.ignoreWhitespace,"")}}
   Text {visible:root.view.sourceKind==="branch-range"&&root.width>Style.space(650);text:((root.view.data || {}).source || {}).headRef || "HEAD";color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption;Layout.maximumWidth:Style.space(180);elide:Text.ElideMiddle;textFormat:Text.PlainText}
   Text {visible:root.view.sourceKind==="branch-range"&&root.width>Style.space(650);text:"→";color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}
   T3BaseRefPicker {
    id:base;Layout.fillWidth:true;Layout.preferredWidth:Style.space(190);Layout.maximumWidth:Style.space(200);visible:root.view.sourceKind==="branch-range";view:root.refs
    selectedRef:root.view.baseRef;resolvedRef:((root.view.data || {}).source || {}).baseRef || "";headRef:((root.view.data || {}).source || {}).headRef || ""
    onRequested:function(query,kind,cursor){root.refsRequested(query,kind,cursor)}
    onDismissed:root.refsDismissed()
    onChosen:function(value){root.requested(root.view.sourceKind,value,root.view.ignoreWhitespace,"")}
   }
   Item {Layout.fillWidth:true}
   T3IconButton {name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:root.requested(root.view.sourceKind,root.view.baseRef,root.view.ignoreWhitespace,review.selectedPath)}
  }
  Text {Layout.fillWidth:true;visible:!!((root.view.data || {}).source || {}).truncated || !!(review.file || {}).truncated;text:T3I18n.tr("Patch truncated. Select a file to load its changes.");color:Color.popups.text;opacity:0.6;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
  Text {Layout.fillWidth:true;visible:!root.view.loading&&!root.view.error&&root.view.data&&!(root.view.data.files || []).length;text:T3I18n.tr("No changes");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}
  T3DiffReview {
   id:review;rendererBridge:root.rendererBridge;diffLayout:root.rendererBridge&&root.rendererBridge.diffPreferences?root.rendererBridge.diffPreferences.layout:"stacked";wordWrap:root.rendererBridge&&root.rendererBridge.diffPreferences?root.rendererBridge.diffPreferences.wordWrap:true;Layout.fillWidth:true;Layout.fillHeight:true;view:root.view;summary:({});treeOpen:true
   ignoreWhitespace:root.view.ignoreWhitespace
   initialPath:root.view.filePath || ""
   onClosed:root.closed()
   onRetryRequested:root.refresh(selectedPath || root.view.filePath)
   onWhitespaceChanged:function(value){root.requested(root.view.sourceKind,root.view.baseRef,value,selectedPath)}
   onFileRequested:function(path){root.refresh(path)}
  }
 }
}
