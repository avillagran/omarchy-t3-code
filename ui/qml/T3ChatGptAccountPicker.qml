import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeProviderAuth.js" as NativeProviderAuth

Controls.Popup {
    id: root
    objectName: "chatGptAccountPicker"
    parent: Controls.Overlay.overlay
    anchors.centerIn: parent
    modal: true
    focus: true
    padding: Style.space(20)
    width: Math.min(Style.space(520), parent ? parent.width - Style.space(32) : Style.space(520))
    closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutside
    property string environment: ""
    property string instanceId: ""
    property var authState: ({})
    property bool busy: false
    property bool canManage: false
    property string error: ""
    property string selection: ""
    readonly property var profiles: NativeProviderAuth.profileMethods(authState)
    readonly property string selectedMethodId: NativeProviderAuth.selectedId(selection, profiles)
    readonly property bool canRemoveSelected: NativeProviderAuth.removable(selectedMethodId, profiles)
    readonly property color fg: Color.popups.text
    signal chosen(string methodId)
    signal removeRequested(string methodId)
    onEnvironmentChanged: { selection = ""; close() }
    onInstanceIdChanged: { selection = ""; close() }
    background: Rectangle {
        color: Color.popups.background
        radius: Style.space(12)
        border.width: 1
        border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.18)
    }
    contentItem: ColumnLayout {
        spacing: Style.space(12)
        Text {
            Layout.fillWidth: true
            text: T3I18n.tr("Reconnect ChatGPT")
            textFormat: Text.PlainText
            color: root.fg
            font.pixelSize: Style.font.body
            font.bold: true
        }
        Text {
            Layout.fillWidth: true
            text: T3I18n.tr("On OpenAI, sign in with the account you choose here.")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: root.fg
        }
        Controls.ButtonGroup { id: accounts }
        Repeater {
            model: root.profiles
            Controls.RadioButton {
                id: profileChoice
                required property var modelData
                objectName: "chatGptProfile_" + modelData.id
                Layout.fillWidth: true
                Controls.ButtonGroup.group: accounts
                checked: root.selectedMethodId === modelData.id
                enabled: !root.busy && root.canManage
                onClicked: root.selection = modelData.id
                contentItem: Text {
                    text: profileChoice.modelData.name
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    leftPadding: profileChoice.indicator ? profileChoice.indicator.width + Style.space(10) : 0
                    color: root.fg
                }
            }
        }
        Controls.RadioButton {
            objectName: "chatGptDifferentAccount"
            Layout.fillWidth: true
            Controls.ButtonGroup.group: accounts
            text: T3I18n.tr("Use a different account")
            checked: root.selectedMethodId === NativeProviderAuth.changeAccount
            enabled: !root.busy && root.canManage
            onClicked: root.selection = NativeProviderAuth.changeAccount
        }
        Text {
            objectName: "chatGptAccountError"
            Layout.fillWidth: true
            visible: root.error.length > 0
            text: root.error
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            color: Color.urgent
        }
        RowLayout {
            Layout.fillWidth: true
            T3PickerButton {
                objectName: "chatGptRemoveConnection"
                visible: root.canRemoveSelected
                enabled: !root.busy && root.canManage
                primary: false
                text: T3I18n.tr("Remove connection")
                onClicked: if (enabled && root.canRemoveSelected) root.removeRequested(root.selectedMethodId)
            }
            Item { Layout.fillWidth: true }
            T3PickerButton {
                objectName: "chatGptReconnect"
                enabled: !root.busy && root.canManage
                text: T3I18n.tr("Reconnect")
                onClicked: if (enabled) root.chosen(root.selectedMethodId)
            }
        }
    }
}
