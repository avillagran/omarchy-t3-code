import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons
Controls.TextField {
 id:root
 implicitHeight:Style.space(42)
 leftPadding:Style.space(12);rightPadding:leftPadding
 font.pixelSize:Style.font.body
 color:Color.popups.text
 placeholderTextColor:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.45)
 selectionColor:Color.accent;selectedTextColor:Color.popups.background
 selectByMouse:true
 background:Rectangle{radius:Style.space(7);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.035);border.width:1;border.color:root.activeFocus?Color.accent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.12)}
}
