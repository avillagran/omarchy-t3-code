import QtQuick
import QtQuick.Controls as Controls

Image {
    id: root
    smooth: false
    cache: false
    fillMode: Image.PreserveAspectFit
    activeFocusOnTab: visible
    Accessible.role: Accessible.Button
    Accessible.name: "QR"
    Accessible.onPressAction: enlarge()
    function enlarge() { if (visible && source.toString() !== "") viewer.open() }
    onSourceChanged: viewer.close()
    onVisibleChanged: if (!visible) viewer.close()
    Keys.onReturnPressed: enlarge()
    Keys.onSpacePressed: enlarge()
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.enlarge()
    }
    Controls.Popup {
        id: viewer
        objectName: "pairingQrViewer"
        parent: Controls.Overlay.overlay
        anchors.centerIn: parent
        width: Math.max(0, Math.min(680, parent ? parent.width - 32 : 680, parent ? parent.height - 32 : 680))
        height: width
        padding: Math.max(12, width * 0.04)
        modal: true
        focus: true
        closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside
        background: Rectangle { color: "white"; radius: 8 }
        contentItem: Image {
            source: viewer.visible ? root.source : ""
            fillMode: Image.PreserveAspectFit
            smooth: false
            cache: false
            MouseArea { anchors.fill: parent; onClicked: viewer.close() }
        }
    }
}
