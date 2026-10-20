import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's session import: folders where Claude or Codex ran, with how many sessions
// each has; importing one brings that history into a T3 project.
Controls.Popup {
 id:root;objectName:"t3AgentSessions"
 required property var bridge
 readonly property var view:bridge.agentSessions || ({})
 readonly property var candidates:view.candidates || []
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 function show(){open();bridge.agentSessionsAction({action:"scan"})}
 modal:true;focus:true;padding:Style.space(18)
 x:Math.round((parent.width-width)/2);y:Math.round((parent.height-height)/2)
 width:Math.min(Style.space(640),parent.width-Style.space(40));height:Math.min(Style.space(560),parent.height-Style.space(40))
 background:Rectangle{radius:Style.space(12);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
 ColumnLayout{
  anchors.fill:parent;spacing:Style.space(10)
  RowLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Import agent sessions");color:root.fg;font.pixelSize:Style.font.body+4;font.weight:Font.DemiBold}
   T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Scan again");enabled:!root.view.busy;onClicked:root.bridge.agentSessionsAction({action:"scan"})}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
  }
  Text{Layout.fillWidth:true;wrapMode:Text.Wrap;color:root.muted;font.pixelSize:Style.font.caption;text:T3I18n.tr("Folders where Claude or Codex ran on this environment. Importing brings their sessions into the folder's project, creating it if needed.")}
  Text{visible:!!root.view.error;Layout.fillWidth:true;wrapMode:Text.Wrap;text:T3I18n.error(root.view.error||"");color:Color.urgent;font.pixelSize:Style.font.caption}
  Text{visible:!!root.view.last;Layout.fillWidth:true;wrapMode:Text.Wrap;color:root.fg;font.pixelSize:Style.font.caption;text:root.view.last?T3I18n.tr("Imported {imported} sessions, skipped {skipped}.",{imported:root.view.last.importedCount,skipped:root.view.last.skippedCount}):""}
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.busy;active:visible}
  ListView{
   id:list;objectName:"agentSessionList";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:root.candidates;spacing:Style.space(4)
   T3Wheel{flickable:list}
   Text{anchors.centerIn:parent;visible:!list.count&&!root.view.busy;text:T3I18n.tr("No agent sessions found.");color:root.muted;font.pixelSize:Style.font.caption}
   delegate:RowLayout{
    required property var modelData;width:list.width;height:Style.space(44);spacing:Style.space(8)
    ColumnLayout{Layout.fillWidth:true;spacing:Style.space(1)
     Text{Layout.fillWidth:true;text:modelData.title;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
     Text{Layout.fillWidth:true;text:modelData.path+" · "+(modelData.sources||[]).map(function(s){return s==="claudeAgent"?"Claude":"Codex"}).join(", ")+" · "+T3I18n.tr("{count} sessions",{count:modelData.threadCount});color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
    }
    T3PickerButton{objectName:"agentSessionImport";implicitHeight:Style.space(28);primary:!modelData.alreadyImported;enabled:!root.view.busy;text:T3I18n.tr(modelData.alreadyImported?"Import new":"Import");onClicked:root.bridge.agentSessionsAction({action:"import",path:modelData.path,projectId:modelData.projectId||null})}
   }
  }
 }
}
