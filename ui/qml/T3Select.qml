import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons

Controls.ComboBox {
  id: root
  property bool translateLabels:false
  property bool popupAbove: true
  implicitHeight: Style.space(30)
  font.pixelSize: Style.font.caption
  palette.window: Color.popups.background
  palette.base: Color.popups.background
  palette.text: Color.popups.text
  palette.windowText: Color.popups.text
  palette.button: Color.popups.background
  palette.buttonText: Color.popups.text
  palette.highlight: Color.accent
  palette.highlightedText: Color.popups.background
  contentItem: Text {
    leftPadding: Style.space(9); rightPadding: Style.space(25)
    text: root.translateLabels?T3I18n.tr(root.displayText):root.displayText; textFormat: Text.PlainText; font: root.font; color: Color.popups.text
    verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight
    opacity: root.enabled ? 1 : 0.45
  }
  indicator: T3Icon {
    x: root.width - width - Style.space(7); anchors.verticalCenter: parent.verticalCenter
    name: "chevron-down"; implicitWidth: Style.space(12); tint: Color.popups.text
  }
  background: Rectangle {
    radius: Style.space(7)
    color: Qt.rgba(Color.popups.text.r, Color.popups.text.g, Color.popups.text.b, root.hovered ? 0.09 : 0.035)
    border.width: root.activeFocus ? 1 : 0; border.color: Color.accent
  }
  popup: Controls.Popup {
    y: root.popupAbove ? -height - 2 : root.height + 2; width: Math.max(root.width, Style.space(240))
    height: Math.min(contentItem.implicitHeight + padding * 2, Style.space(280))
    padding: Style.space(5)
    background: Rectangle { color: Color.popups.background; radius: Style.space(8); border.width: 1; border.color: Color.accent }
    contentItem: ListView {
    id:comboList
    T3Wheel {flickable:comboList}
      clip: true; implicitHeight: contentHeight
      model: root.popup.visible ? root.delegateModel : null
      currentIndex: root.highlightedIndex
      Controls.ScrollBar.vertical: Controls.ScrollBar {}
    }
  }
  delegate: Controls.ItemDelegate {
    required property int index
    required property var modelData
    width: root.popup.width - root.popup.padding * 2
    highlighted: root.highlightedIndex === index
    contentItem: Text {
      text: root.translateLabels?T3I18n.tr(root.textRole?modelData[root.textRole]:modelData):(root.textRole?modelData[root.textRole]:modelData)
      textFormat: Text.PlainText
      font: root.font; color: Color.popups.text; elide: Text.ElideRight
    }
    background: Rectangle {
      radius: Style.space(5)
      color: parent.highlighted ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.20) : "transparent"
    }
  }
}
