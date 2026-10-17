import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui

Item {
 id:root
 required property var bridge
 required property Item popupHost
 property var request:null
 function choose(isolated){if(request)bridge.confirmNewThread(request,isolated)}
 Connections{target:root.bridge;function onNewThreadWorkspaceRequested(value){root.request=value;dialog.open()}function onWorkspaceChoiceAccepted(){dialog.close()}}
 Controls.Popup {
  id:dialog
  objectName:"t3WorkspaceConfirmation"
  parent:root.popupHost
  anchors.centerIn:parent
  width:Math.min(Style.space(480),parent.width-Style.space(32));padding:Style.space(20)
  modal:true;focus:true
  closePolicy:root.bridge.workspaceOperation.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
  onClosed:root.request=null
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.accent}
  contentItem:ColumnLayout {
   spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Enable worktrees by default?");color:Color.popups.text;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("T3 Code can create a separate branch and folder so threads can edit in parallel. Changes are integrated through T3 Code's Git and pull request actions.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("This choice is saved in T3 Code for this environment. Future threads use it without asking again. Project overrides still apply.");color:Color.popups.text;opacity:0.65;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:!!root.bridge.workspaceOperation.error;text:T3I18n.error(root.bridge.workspaceOperation.error);color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   Flow{Layout.fillWidth:true;spacing:Style.space(8)
    Button{objectName:"workspaceConfirmIsolated";enabled:!root.bridge.workspaceOperation.busy&&!root.bridge.workspaceOperation.error;text:T3I18n.tr("Enable worktrees");onClicked:root.choose(true)}
    Button{objectName:"workspaceConfirmShared";enabled:!root.bridge.workspaceOperation.busy&&!root.bridge.workspaceOperation.error;text:T3I18n.tr("Use same folder");onClicked:root.choose(false)}
    Button{objectName:"workspacePreferenceRetry";visible:!!root.bridge.workspaceOperation.error;enabled:!root.bridge.workspaceOperation.busy;text:T3I18n.tr("Retry");onClicked:root.bridge.beginNewThread(root.request)}
    Button{objectName:"workspaceConfirmCancel";enabled:!root.bridge.workspaceOperation.busy;text:T3I18n.tr("Cancel");onClicked:dialog.close()}
   }
  }
 }
}
