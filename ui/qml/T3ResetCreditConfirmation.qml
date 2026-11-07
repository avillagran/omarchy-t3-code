import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui

Item {
  id:root
  required property var bridge
  required property Item popupHost
  Connections{target:root.bridge;function onResetCreditConfirmationRequested(){dialog.open()}}
  Controls.Popup {
    id:dialog
    objectName:"t3ResetCreditConfirmation"
    parent:root.popupHost
    anchors.centerIn:parent
    width:Math.min(Style.space(480),parent.width-Style.space(32));padding:Style.space(20)
    modal:true;focus:true;closePolicy:Controls.Popup.CloseOnEscape
    onOpened:cancel.forceActiveFocus()
    onClosed:root.bridge.resetCreditConfirmation=null
    background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.accent}
    contentItem:ColumnLayout {
      spacing:Style.space(12)
      Text{Layout.fillWidth:true;text:T3I18n.tr("Use a reset credit?");color:Color.popups.text;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:root.bridge.resetCreditConfirmation?root.bridge.resetCreditConfirmation.label:"";color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:T3I18n.tr("This redeems one credit on your account and clears the current rate-limit windows. It cannot be undone.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      RowLayout {
        Button{id:cancel;objectName:"resetCreditCancel";text:T3I18n.tr("Cancel");onClicked:dialog.close()}
        Button{objectName:"resetCreditConfirm";text:T3I18n.tr("Use credit");enabled:!!root.bridge.resetCreditConfirmation && !!root.bridge.resetCreditAccess[root.bridge.resetCreditConfirmation.redeem.environmentId] && !root.bridge.resetCreditBusy(root.bridge.resetCreditConfirmation);onClicked:{root.bridge.confirmResetCredit();dialog.close()}}
      }
    }
  }
}
