import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
// Low free space on this computer's T3 disk (the client checks it every 5 minutes).
// Clean up now runs T3's storage cleanup on the local server; Settings opens the
// storage page. Worktrees lists each worktree's target/ size, merged state and
// local changes, measured only when opened. Closing hides it until space recovers.
Rectangle {
 id:root
 objectName:"t3DiskSpaceNotice"
 required property var bridge
 signal settingsRequested()
 readonly property var disk:bridge.diskSpace || ({})
 readonly property var cleanup:bridge.storageCleanup || ({})
 readonly property var diskReport:bridge.diskReport || ({})
 readonly property var worktrees:(diskReport.result || {}).worktrees || []
 property bool listOpen:false
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.65)
 function size(bytes){bytes=Number(bytes||0);if(bytes<1024)return bytes+" B";var u=["KB","MB","GB","TB"],i=-1;do{bytes/=1024;i++}while(bytes>=1024&&i<u.length-1);return bytes.toFixed(bytes<10?1:0)+" "+u[i]}
 function name(w){var parts=String(w.path||"").split("/");return parts.slice(-2).join("/")}
 function flags(w){var out=[];if(w.merged)out.push(T3I18n.tr("Merged"));if(w.hasChanges)out.push(T3I18n.tr("Local changes"));return out.join(" · ")}
 visible:!!disk.low&&!bridge.diskNoticeDismissed
 implicitHeight:column.implicitHeight+Style.space(24)
 radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.14)
 ColumnLayout {
  id:column;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(12)}spacing:Style.space(6)
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(10)
   T3Icon{name:"hard-drive";implicitWidth:Style.space(20);tint:T3Palette.warning;Layout.alignment:Qt.AlignTop}
   ColumnLayout {
    Layout.fillWidth:true;spacing:Style.space(2)
    Text{objectName:"t3DiskSpaceTitle";Layout.fillWidth:true;text:T3I18n.tr("Low disk space: {free} free",{free:root.size(root.disk.freeBytes)});color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Below the {threshold} warning on this computer. Remote environments are not checked.",{threshold:root.size(root.disk.thresholdBytes)});color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   }
   T3IconButton{objectName:"t3DiskSpaceClose";name:"x";implicitWidth:Style.space(22);implicitHeight:implicitWidth;Layout.alignment:Qt.AlignTop;tooltipText:T3I18n.tr("Dismiss");onClicked:root.bridge.diskNoticeDismissed=true}
  }
  Text{visible:root.cleanup.running||!!root.cleanup.error||!!root.cleanup.report&&root.bridge.diskCleanupRan;Layout.fillWidth:true;Layout.leftMargin:Style.space(30);wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption
   color:root.cleanup.error?Color.urgent:root.dim
   text:root.cleanup.running?T3I18n.tr("Cleaning up…"):root.cleanup.error?T3I18n.error(root.cleanup.error):root.cleanup.report?T3I18n.tr("Last cleanup: {removed} removed, {kept} kept, {failed} failed, {freed} freed.",{removed:root.cleanup.report.counts.removed,kept:root.cleanup.report.counts.kept,failed:root.cleanup.report.counts.failed,freed:root.size(root.cleanup.report.bytesFreed)}):""}
  RowLayout {
   Layout.fillWidth:true;Layout.leftMargin:Style.space(30);spacing:Style.space(8)
   T3PickerButton{objectName:"t3DiskSpaceCleanup";primary:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;enabled:!root.cleanup.busy;text:T3I18n.tr(root.cleanup.running?"Cleaning up…":"Clean up now");onClicked:root.bridge.cleanLocalStorage()}
   T3PickerButton{objectName:"t3DiskSpaceSettings";primary:false;outlined:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;text:T3I18n.tr("Settings");onClicked:root.settingsRequested()}
   T3PickerButton{objectName:"t3DiskSpaceWorktrees";primary:false;outlined:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;text:T3I18n.tr("Worktrees");onClicked:{root.listOpen=!root.listOpen;if(root.listOpen)root.bridge.requestDiskReport()}}
   Item{Layout.fillWidth:true}
  }
  Text{visible:root.listOpen&&(!!root.diskReport.busy||!!root.diskReport.error||!root.diskReport.busy&&!root.worktrees.length);Layout.fillWidth:true;Layout.leftMargin:Style.space(30);text:root.diskReport.busy?T3I18n.tr("Loading…"):root.diskReport.error?T3I18n.error(root.diskReport.error):T3I18n.tr("No worktrees found.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Repeater {
   model:root.listOpen&&!root.diskReport.busy?root.worktrees.slice(0,10):[]
   RowLayout {
    required property var modelData
    Layout.fillWidth:true;Layout.leftMargin:Style.space(30);spacing:Style.space(8)
    Text{Layout.fillWidth:true;text:modelData.title?modelData.title+" · "+root.name(modelData):root.name(modelData);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
    Text{visible:!!root.flags(modelData);text:root.flags(modelData);color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    Text{text:T3I18n.tr("{size} in target/",{size:root.size(modelData.targetBytes)});color:modelData.targetBytes>0?root.fg:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   }
  }
 }
}
