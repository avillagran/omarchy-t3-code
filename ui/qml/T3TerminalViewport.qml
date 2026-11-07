import QtQuick 6.11
import qs.Commons
import O3.Terminal 1.0 as VT
import "../js/NativeTerminalLinks.js" as Links

VT.TerminalSurface {
  id: root
  Accessible.role: Accessible.EditableText
  Accessible.name: T3I18n.tr("Terminal input")
  Accessible.focusable: true
  property var nativeLinks: Links.create(function(text,pattern,flags){return root.matchTerminalPattern(text,pattern,flags)})
  linkResolver: function(row,column){return root.nativeLinks.terminalLinkAtPositionWithRange(root.linkRows(row),row,column)}
  linkAdmission: function(uri){return root.nativeLinks.isTerminalUrl(uri)}
  foreground: (Color.popups ? Color.popups.text : Color.foreground)
  background: (Color.popups ? Color.popups.background : Color.background)
  fontFamily: "monospace"
  fontPixelSize: Style.font.body
  T3TerminalScrollbar {
    objectName: "t3TerminalScrollback"
    anchors.right: parent.right; anchors.rightMargin: 1
    anchors.top: parent.top; anchors.topMargin: 4
    anchors.bottom: parent.bottom; anchors.bottomMargin: 4
    width: Style.space(8)
    surface: root
  }
}
