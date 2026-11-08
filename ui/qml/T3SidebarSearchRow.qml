import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

Controls.ItemDelegate {
  id: root
  required property var thread
  required property var result
  property bool routeActive: false
  property bool manyHosts: false
  signal highlightRequested()
  signal threadChosen()
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r,fg.g,fg.b,0.6)
  function rich(parts,fallback){
    function escaped(value){return String(value).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}
    return parts ? parts.map(function(part){var text=escaped(part.text);return part.highlighted?"<b>"+text+"</b>":text}).join("") : escaped(fallback)
  }
  objectName: "sidebarSearchResult"
  implicitHeight: content.implicitHeight + Style.space(14)
  padding: Style.space(7)
  Accessible.role: Accessible.ListItem
  Accessible.name: thread.title || T3I18n.tr("Untitled thread")
  Accessible.description: [thread.project || "",thread.branch || ""].filter(Boolean).join(" · ")
  onClicked: threadChosen()
  onHoveredChanged: if(hovered)highlightRequested()
  background: Rectangle {
    radius: Style.space(5)
    color: root.highlighted || root.hovered ? Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08) : "transparent"
    border.width: root.activeFocus ? 1 : 0
    border.color: Color.accent
  }
  contentItem: ColumnLayout {
    id: content
    spacing: Style.space(3)
    RowLayout {
      Layout.fillWidth: true
      spacing: Style.space(6)
      T3Icon { name:root.thread.settled?"archive":root.thread.working?"loader-circle":"message-square";implicitWidth:Style.space(14);tint:root.dim }
      Text {
        objectName: "sidebarSearchTitle"
        Layout.fillWidth:true
        text:root.rich(root.result.titleParts,root.thread.title || T3I18n.tr("Untitled thread"))
        textFormat:Text.RichText;color:root.fg;font.pixelSize:Style.font.body
        elide:Text.ElideRight;maximumLineCount:1
      }
      T3Icon { visible:root.routeActive;name:"circle-dot";implicitWidth:Style.space(10);tint:Color.accent }
    }
    Text {
      objectName: "sidebarSearchMetadata"
      Layout.fillWidth:true
      text:[root.thread.project || "",root.thread.branch || "",root.manyHosts?root.thread.environmentName || root.thread.environmentId:""].filter(Boolean).join(" · ")
      textFormat:Text.PlainText;color:root.dim;font.pixelSize:Style.font.caption
      elide:Text.ElideRight;maximumLineCount:1
    }
    Text {
      objectName: "sidebarSearchSnippet"
      Layout.fillWidth:true
      visible:root.result.snippetParts!==null && root.result.snippetParts!==undefined
      text:root.rich(root.result.snippetParts,"")
      textFormat:Text.RichText;color:root.dim;font.pixelSize:Style.font.caption
      wrapMode:Text.Wrap;maximumLineCount:2;elide:Text.ElideRight
    }
  }
}
