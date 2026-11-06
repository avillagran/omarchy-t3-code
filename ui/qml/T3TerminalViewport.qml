import QtQuick 6.11
import qs.Commons
import O3.Terminal 1.0 as VT

VT.TerminalSurface {
  foreground: (Color.popups ? Color.popups.text : Color.foreground)
  background: (Color.popups ? Color.popups.background : Color.background)
  fontFamily: "monospace"
  fontPixelSize: Style.font.body
}
