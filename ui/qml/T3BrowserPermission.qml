import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui

Item {
 id:root
 required property var bridge
 required property Item popupHost
 readonly property bool opened:dialog.visible
 readonly property Item defaultFocusItem:block
 property string requestId:""
 property string origin:""
 function synchronize(){
  var prompt=bridge.browserPermission
  if(!prompt){requestId="";dialog.close();return}
  if(requestId===prompt.requestId)return
  requestId=prompt.requestId;origin=String(prompt.origin||"")
  dialog.open()
 }
 function choose(allow){if(requestId){requestId="";bridge.answerBrowserPermission(allow===true)}dialog.close()}
 Connections{target:root.bridge;function onBrowserPermissionChanged(){root.synchronize()}}
 Component.onCompleted:synchronize()
 Controls.Popup {
  id:dialog
  objectName:"t3BrowserClipboardPermission"
  parent:root.popupHost
  anchors.centerIn:parent
  width:Math.min(Style.space(480),parent.width-Style.space(32));padding:Style.space(20)
  modal:true;focus:true
  closePolicy:Controls.Popup.CloseOnEscape
  onOpened:block.forceActiveFocus()
  onClosed:{if(root.requestId)root.choose(false)}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.accent}
  contentItem:ColumnLayout {
   spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("{origin} wants to see text and images copied to the clipboard",{origin:root.origin.slice(0,300)+(root.origin.length>300?"…":"")});textFormat:Text.PlainText;color:Color.popups.text;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   RowLayout{
    Layout.alignment:Qt.AlignRight
    Button{objectName:"browserClipboardAllow";focusable:true;text:T3I18n.tr("Allow");onClicked:root.choose(true)}
    Button{id:block;objectName:"browserClipboardBlock";focusable:true;text:T3I18n.tr("Block");onClicked:root.choose(false)}
   }
  }
 }
}
