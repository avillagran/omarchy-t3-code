import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Native CLI account authorization is separate from the Electron Clerk session.
// Parent supplies the bridge documented in docs/CONNECTION_CLOUD.md.
ColumnLayout {
    id: root
    required property var bridge
    required property string environment
    readonly property var store: bridge.connectionCloud || ({})
    readonly property bool local: environment === "local"
    readonly property bool scoped: local && store.environment === environment
    readonly property bool busy: scoped && !!store.busy
    readonly property var status: scoped && store.status ? store.status : ({})
    readonly property var device: scoped && store.device ? store.device : null
    readonly property color fg: Color.popups.text
    readonly property color muted: Qt.rgba(fg.r, fg.g, fg.b, 0.58)
    property string pendingAction: ""
    spacing: Style.space(12)

    function request(action, values) {
        if (bridge && local && !busy) bridge.requestConnectionCloud(environment, action, values || {})
    }
    function load() { if (bridge && local && !busy) request("read") }
    function confirm(action) { pendingAction = action; confirmation.open() }
    function cancel() { if (busy) bridge.cancelConnectionCloud(store.requestId) }
    function errorMessage(code) {
        var messages = {
            cancelled: "Authorization cancelled. Refresh to check any changes already saved by T3.",
            t3_connect_timeout: "T3 Connect timed out. Refresh before trying again.",
            t3_connect_authorization_denied: "Authorization was denied in the browser. Sign in again when ready.",
            t3_connect_authorization_expired: "The authorization code expired. Sign in again to request a new code.",
            t3_connect_cli_failed: "The native T3 Connect command failed. Check the account, connection and CLI version, then refresh.",
            t3_connect_spawn_failed: "The native T3 CLI could not be started.",
            t3_connect_invalid_status: "The native T3 CLI returned an unsupported status response.",
            t3_connect_disconnect_incomplete: "T3 disabled access locally, but relay revocation or the running tunnel could not be stopped. Refresh and retry unlink when reachable. Server control remains separate."
        }
        return T3I18n.tr(messages[code] || code || "")
    }
    function progress() {
        var messages = {
            starting: "Starting native T3 Connect…",
            authorization: "Waiting for browser approval…",
            "install-confirmation": "The native T3 relay client requires installation.",
            installing: "Installing the native T3 relay client…",
            refreshing: "Verifying saved T3 Connect state…"
        }
        return T3I18n.tr(messages[store.stage] || "Loading…")
    }
    Component.onCompleted: load()
    onEnvironmentChanged: { confirmation.close(); pendingAction = ""; load() }
    // The parent retains the job across page navigation. Cancellation is explicit.

    T3PreferenceSection {
        Layout.fillWidth: true
        title: "T3 Connect · " + T3I18n.tr("Account")
        icon: "cloud"
        body: Component {
            ColumnLayout {
                width: parent.width
                spacing: Style.space(12)
                Text {
                    Layout.fillWidth: true
                    Layout.margins: Style.space(14)
                    text: T3I18n.tr(root.local
                        ? "Sign in or create an account in your browser. T3 owns authorization and credential storage. Signing in does not enable remote access."
                        : "Account authorization must run on the environment host. This account control manages only this machine, not the selected remote environment.")
                    color: root.muted; wrapMode: Text.Wrap
                    font.pixelSize: Style.font.caption; textFormat: Text.PlainText
                }
                ColumnLayout {
                    Layout.fillWidth: true; Layout.margins: Style.space(14)
                    visible: root.local; spacing: Style.space(12)
                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            objectName: "connectionCloudAccountStatus"
                            Layout.fillWidth: true
                            text: T3I18n.tr(root.status.authenticated ? "Signed in to native T3 CLI" : root.store.status ? "Signed out" : "Account status unknown")
                            color: root.fg; font.pixelSize: Style.font.body; textFormat: Text.PlainText
                        }
                        T3IconButton { objectName: "connectionCloudRefresh"; name: "refresh-cw"; tooltipText: T3I18n.tr("Refresh"); enabled: !root.busy; onClicked: root.load() }
                    }
                    Text {
                        Layout.fillWidth: true; visible: !!root.status.cloudUserId
                        text: T3I18n.tr("Linked account") + " · " + (root.status.cloudUserId || "")
                        color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText
                    }
                    Text {
                        objectName: "connectionCloudSavedStatus"
                        Layout.fillWidth: true
                        text: T3I18n.tr(root.status.revoked ? "This environment was removed from T3 Connect. Link it again to repair access."
                            : root.status.desired && !root.status.linked ? "Link requested. T3 provisions it on the next server start."
                            : root.status.linked ? "Environment link is saved. This is not a live tunnel check."
                            : "This environment is not linked to T3 Connect.")
                        color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText
                    }
                    Text {
                        Layout.fillWidth: true
                        text: T3I18n.tr("Saved setup only. Live tunnel status and activity preferences are shown separately. No server is started, stopped or restarted here.")
                        color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: Style.space(8)
                        T3PickerButton { objectName: "connectionCloudLogin"; visible: !root.status.authenticated; text: T3I18n.tr("Sign in"); enabled: !root.busy; onClicked: root.request("login") }
                        T3PickerButton { objectName: "connectionCloudRegister"; primary: false; visible: !root.status.authenticated; text: T3I18n.tr("Create account"); enabled: !root.busy; onClicked: root.request("register") }
                        T3PickerButton { objectName: "connectionCloudLink"; primary: false; text: T3I18n.tr(root.status.revoked ? "Link again" : "Link environment"); enabled: !root.busy && !!root.status.authenticated; onClicked: root.confirm("link") }
                        T3PickerButton { objectName: "connectionCloudPublishOnly"; primary: false; text: T3I18n.tr("Link without tunnel"); enabled: !root.busy && !!root.status.authenticated; onClicked: root.confirm("link-publish-only") }
                        T3PickerButton { objectName: "connectionCloudUnlink"; primary: false; text: T3I18n.tr("Unlink environment"); enabled: !root.busy && (!!root.status.linked || !!root.status.desired); onClicked: root.confirm("unlink") }
                        T3PickerButton { objectName: "connectionCloudLogout"; primary: false; text: T3I18n.tr("Sign out"); enabled: !root.busy && !!root.status.authenticated; onClicked: root.confirm("logout") }
                    }
                    Text { Layout.fillWidth: true; visible: root.busy; text: root.progress(); color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                    Text { Layout.fillWidth: true; visible: root.busy && !!root.store.step; text: T3I18n.tr(root.store.step || ""); color: root.muted; font.pixelSize: Style.font.caption; textFormat: Text.PlainText }
                    ColumnLayout {
                        Layout.fillWidth: true; visible: root.busy && !!root.device
                        Text { Layout.fillWidth: true; text: T3I18n.tr("Open this native authorization URL to sign in or create an account, then approve the displayed code."); color: root.fg; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                        T3PickerField { objectName: "connectionCloudDeviceUrl"; Layout.fillWidth: true; text: root.device ? root.device.verificationUrl : ""; readOnly: true; selectByMouse: true; Accessible.name: T3I18n.tr("Authorization URL") }
                        T3PickerField { objectName: "connectionCloudDeviceCode"; Layout.fillWidth: true; text: root.device ? root.device.userCode : ""; readOnly: true; selectByMouse: true; Accessible.name: T3I18n.tr("Authorization code") }
                        Text { Layout.fillWidth: true; text: T3I18n.tr("Expires in minutes") + ": " + (root.device ? root.device.expiresInMinutes : ""); color: root.muted; font.pixelSize: Style.font.caption; textFormat: Text.PlainText }
                        RowLayout {
                            T3PickerButton { objectName: "connectionCloudOpenUrl"; primary: false; text: T3I18n.tr("Open in browser"); onClicked: if (root.device) Qt.openUrlExternally(root.device.verificationUrl) }
                            T3PickerButton { objectName: "connectionCloudCopyCode"; primary: false; text: T3I18n.tr("Copy code"); onClicked: if (root.device) root.bridge.copyText(root.device.userCode) }
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true; visible: root.busy && root.store.stage === "install-confirmation"
                        Text { Layout.fillWidth: true; text: T3I18n.tr("Allow the native CLI to download and verify its relay client? This does not transfer service ownership."); color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                        RowLayout {
                            T3PickerButton { objectName: "connectionCloudInstall"; primary: false; text: T3I18n.tr("Install relay client"); onClicked: root.bridge.confirmConnectionCloudInstall(root.store.requestId, true) }
                            T3PickerButton { primary: false; text: T3I18n.tr("Decline"); onClicked: root.bridge.confirmConnectionCloudInstall(root.store.requestId, false) }
                        }
                    }
                    T3PickerButton { objectName: "connectionCloudCancel"; primary: false; visible: root.busy; text: T3I18n.tr("Cancel"); onClicked: root.cancel() }
                    Text { objectName: "connectionCloudError"; Layout.fillWidth: true; visible: !!root.store.error; text: root.errorMessage(root.store.error); color: Color.urgent; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                    Text { Layout.fillWidth: true; visible: !!root.store.statusError; text: root.errorMessage(root.store.statusError); color: Color.urgent; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                    Text { objectName: "connectionCloudWarning"; Layout.fillWidth: true; visible: !!root.store.warning; text: root.errorMessage(root.store.warning); color: Color.urgent; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                }
            }
        }
    }
    Controls.Popup {
        id: confirmation
        parent: Controls.Overlay.overlay; anchors.centerIn: parent
        width: Math.min(Style.space(460), parent ? parent.width - Style.space(32) : Style.space(460))
        padding: Style.space(20); modal: true
        background: Rectangle { color: Color.popups.background; radius: Style.space(12); border.width: 1; border.color: root.muted }
        contentItem: ColumnLayout {
            spacing: Style.space(12)
            Text { Layout.fillWidth: true; text: T3I18n.tr(root.pendingAction === "logout" ? "Sign out of native T3 Connect?" : root.pendingAction === "unlink" ? "Unlink this environment?" : "Link this environment?"); color: root.fg; font.bold: true; wrapMode: Text.Wrap; textFormat: Text.PlainText }
            Text {
                Layout.fillWidth: true
                text: T3I18n.tr(root.pendingAction === "logout" ? "Disable this environment's link and remove the native CLI authorization. The Electron account session is separate. The background service remains managed separately."
                    : root.pendingAction === "unlink" ? "Disable this environment's T3 Connect link while retaining the CLI sign-in. Native T3 attempts to stop its tunnel and revoke the relay record."
                    : root.pendingAction === "link-publish-only" ? "Request a tunnel-free link for agent activity. The native CLI enables activity publishing; T3 provisions it on the next server start."
                    : "Request a managed T3 Connect tunnel so other devices can reach this environment. T3 provisions it on the next server start; no server is restarted here.")
                color: root.muted; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap; textFormat: Text.PlainText
            }
            RowLayout {
                Layout.fillWidth: true; Item { Layout.fillWidth: true }
                T3PickerButton { primary: false; text: T3I18n.tr("Cancel"); onClicked: confirmation.close() }
                T3PickerButton { objectName: "connectionCloudConfirm"; primary: false; text: T3I18n.tr("Confirm"); enabled: !root.busy; onClicked: { var action = root.pendingAction; confirmation.close(); root.request(action, {confirmed: true}) } }
            }
        }
    }
}
