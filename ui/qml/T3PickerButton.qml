import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons
Controls.Button {
 id:root
 property bool primary:true
 hoverEnabled:true
 implicitHeight:Style.space(36)
 leftPadding:Style.space(15);rightPadding:leftPadding
 opacity:enabled?1:0.4
 contentItem:Text{text:root.text;color:root.primary?Color.popups.background:Color.popups.text;font.pixelSize:Style.font.body;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
 background:Rectangle{radius:Style.space(7);color:root.primary?(root.hovered?Qt.lighter(Color.accent,1.12):Color.accent):(root.hovered?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.08):"transparent");border.width:root.activeFocus?1:0;border.color:Color.popups.text}
 HoverHandler{cursorShape:root.enabled?Qt.PointingHandCursor:Qt.ArrowCursor}
}
