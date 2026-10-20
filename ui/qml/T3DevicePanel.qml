import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Devices tab of the side panel: simulators and emulators from native T3
// (Preferences > Integrations > Devices), and the screen of one of them with
// touch, hardware buttons and keyboard input. Opening a device starts its
// session for this chat (device.open); "Back to devices" only stops viewing.
Item {
  id: root
  objectName: "t3DevicePanel"
  required property var bridge
  signal integrationsRequested()
  readonly property string environment: bridge.selectedEnvironment
  readonly property var session: (bridge.deviceView.sessions || {})[environment] || null
  readonly property var deviceData: session ? session.data : null
  readonly property var deviceState: deviceData ? deviceData.state : null
  readonly property bool supportOn: !!deviceData && deviceData.settings.enableDeviceSupport === true
  readonly property var devices: deviceState ? (deviceState.devices || []) : []
  readonly property var hosts: deviceState ? (deviceState.hosts || []) : []
  readonly property var screen: bridge.deviceScreen
  readonly property var frame: bridge.deviceFrame
  readonly property bool viewing: screen.stage !== "idle"
  readonly property bool android: !!screen.target && screen.target.platform === "android"
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.6)
  // Deferred: requesting replaces bridge.deviceView, which this panel binds to.
  function watch() { Qt.callLater(function() { if (root.visible && root.environment) root.bridge.requestDevices([{environment: root.environment, projectId: null}]) }) }
  onVisibleChanged: { if (!visible) listedFor = ""; watch() }
  onEnvironmentChanged: watch()
  Component.onCompleted: watch()
  readonly property string hostStatus: deviceState ? deviceState.hostStatus : ""
  readonly property var statuses: deviceState ? Object.keys(deviceState.hostStatuses || {}).map(function(k) { return deviceState.hostStatuses[k].status }).concat([hostStatus]) : []
  readonly property bool hostBusy: statuses.indexOf("ready") < 0 && (statuses.indexOf("installing") >= 0 || statuses.indexOf("starting") >= 0)
  readonly property var operation: bridge.deviceOperations[environment] || ({})
  readonly property bool listing: operation.state === "pending" && operation.action === "refresh"
  readonly property var booting: deviceState ? (deviceState.bootingDevices || []).filter(function(d) { return d.threadId === root.bridge.selectedThread }) : []
  readonly property var unavailable: hosts.reduce(function(all, h) { return all.concat((h.platforms || []).filter(function(p) { return !p.available && p.reason }).map(function(p) { return h.label + " · " + (p.platform === "ios" ? "iOS" : "Android") + ": " + p.reason })) }, [])
  // Like native T3's panel: listing devices installs or starts the hub when
  // needed, so it runs each time the tab shows on an enabled host.
  property string listedFor: ""
  function listDevices() {
    if (!visible || !deviceData || !supportOn || hostStatus === "disabled") return
    var key = environment + "/" + (deviceData.originKey || "")
    if (listedFor === key || !bridge.deviceAllowed(environment, "refresh")) return
    listedFor = key
    bridge.deviceAction("refresh", {}, [environment])
  }
  onDeviceDataChanged: Qt.callLater(listDevices)
  function hostLabel(id) { var h = hosts.filter(function(x) { return x.id === id })[0]; return h ? h.label : id }
  // Sessions name a booted device by its serial; a listed device matches by
  // id (booted devices are listed by serial too).
  function sessionFor(d) { return (deviceState && deviceState.sessions || []).filter(function(s) { return s.threadId === root.bridge.selectedThread && s.hostId === d.hostId && s.deviceId === d.id })[0] || null }
  function open(d) {
    var s = sessionFor(d)
    bridge.openDeviceScreen(environment, s ? Object.assign({}, d, {id: s.deviceId}) : d, deviceState ? deviceState.hubBasePath : "", !s)
  }
  function deviceError(code) {
    var known = {
      device_unavailable: "The device is not available. Start it on its host and try again.",
      device_stream_failed: "The screen stream stopped.",
      device_stream_ended: "The screen stream stopped.",
      unauthorized: "T3 rejected the screen connection.",
      device_input_unavailable: "Touch input is not available for this device.",
      unreachable: "The device host is unreachable."
    }
    return known[code] ? T3I18n.tr(known[code]) : T3I18n.error(code)
  }
  function stageText() {
    switch (screen.stage) {
    case "starting": case "opening": return T3I18n.tr("Starting the device…")
    case "connecting": return T3I18n.tr("Connecting to the screen…")
    case "reconnecting": return T3I18n.tr("Reconnecting…")
    case "closing": return T3I18n.tr("Closing the session…")
    case "error": return deviceError(screen.error)
    default: return ""
    }
  }
  // Keys: Android takes text and keycodes; iOS takes HID usages.
  function sendKey(event) {
    if (android) {
      var codes = {}; codes[Qt.Key_Return] = 66; codes[Qt.Key_Enter] = 66; codes[Qt.Key_Backspace] = 67; codes[Qt.Key_Tab] = 61; codes[Qt.Key_Escape] = 111
      codes[Qt.Key_Up] = 19; codes[Qt.Key_Down] = 20; codes[Qt.Key_Left] = 21; codes[Qt.Key_Right] = 22
      if (codes[event.key]) bridge.deviceScreenInput({type: "key", keycode: codes[event.key]})
      else if (event.text && event.text.length && event.text.charCodeAt(0) >= 32) bridge.deviceScreenInput({type: "text", text: event.text})
      else return false
      return true
    }
    var usage = 0, shift = false, t = event.text || ""
    if (/^[a-z]$/.test(t)) usage = 4 + t.charCodeAt(0) - 97
    else if (/^[A-Z]$/.test(t)) { usage = 4 + t.charCodeAt(0) - 65; shift = true }
    else if (/^[1-9]$/.test(t)) usage = 29 + Number(t)
    else if (t === "0") usage = 39
    else if (t === " ") usage = 44
    else usage = ({}).hasOwnProperty.call(iosKeys, event.key) ? iosKeys[event.key] : 0
    if (!usage) return false
    if (shift) bridge.deviceScreenInput({type: "key", down: true, usage: 225})
    bridge.deviceScreenInput({type: "key", down: true, usage: usage})
    bridge.deviceScreenInput({type: "key", down: false, usage: usage})
    if (shift) bridge.deviceScreenInput({type: "key", down: false, usage: 225})
    return true
  }
  readonly property var iosKeys: { var k = {}; k[Qt.Key_Return] = 40; k[Qt.Key_Enter] = 40; k[Qt.Key_Escape] = 41; k[Qt.Key_Backspace] = 42; k[Qt.Key_Tab] = 43; k[Qt.Key_Right] = 79; k[Qt.Key_Left] = 80; k[Qt.Key_Down] = 81; k[Qt.Key_Up] = 82; return k }

  // Device list.
  ColumnLayout {
    anchors.fill: parent; anchors.margins: Style.space(14); spacing: Style.space(10)
    visible: !root.viewing
    Text { text: T3I18n.tr("Simulators and emulators"); color: root.fg; font.pixelSize: Style.font.body; font.bold: true }
    Text {
      Layout.fillWidth: true; wrapMode: Text.Wrap; color: root.dim; font.pixelSize: Style.font.caption
      visible: !root.deviceData || !root.supportOn || !!(root.session && root.session.error) || root.screen.error === "device_shutdown_pending"
      text: root.screen.error === "device_shutdown_pending" ? T3I18n.tr("The device is still shutting down.")
        : root.session && root.session.error ? T3I18n.error(root.session.error)
        : !root.deviceData ? T3I18n.tr("Loading devices…")
        : T3I18n.tr("Device support is off. Turn it on and add a host with simulators or emulators in Preferences > Integrations.")
    }
    T3PickerButton {
      objectName: "t3DeviceOpenIntegrations"; primary: false; visible: !!root.deviceData && !root.supportOn
      text: T3I18n.tr("Open Integrations"); onClicked: root.integrationsRequested()
    }
    RowLayout {
      Layout.fillWidth: true; spacing: Style.space(8)
      visible: root.supportOn && (root.hostBusy || root.listing)
      WorkIndicator { active: parent.visible; implicitWidth: Style.space(14) }
      Text {
        Layout.fillWidth: true; wrapMode: Text.Wrap; color: root.dim; font.pixelSize: Style.font.caption
        text: root.hostStatus === "installing" ? (root.deviceState.hostStatusDetail || T3I18n.tr("Installing device support…")) : T3I18n.tr("Finding devices…")
      }
    }
    Text {
      objectName: "t3DeviceHostFailed"
      Layout.fillWidth: true; wrapMode: Text.Wrap; color: Color.urgent; font.pixelSize: Style.font.caption
      visible: root.supportOn && (root.hostStatus === "failed" || root.operation.state === "failed")
      text: root.hostStatus === "failed" ? (root.deviceState.hostStatusDetail || T3I18n.tr("The device hub failed to start.")) : T3I18n.error(root.operation.error || "")
    }
    Text {
      Layout.fillWidth: true; wrapMode: Text.Wrap; color: root.dim; font.pixelSize: Style.font.caption
      visible: root.booting.length > 0
      text: T3I18n.tr("Starting {devices}… This can take a minute.", {devices: root.booting.map(function(d) { return d.name }).join(", ")})
    }
    Text {
      Layout.fillWidth: true; wrapMode: Text.Wrap; color: root.dim; font.pixelSize: Style.font.caption
      visible: root.supportOn && root.devices.length === 0 && !root.hostBusy && !root.listing && root.hostStatus !== "failed"
      text: T3I18n.tr("No simulators or emulators were found on this environment.") + (root.unavailable.length ? "\n" + root.unavailable.join("\n") : "")
    }
    // Keeps the messages at the top while the list is hidden.
    Item { Layout.fillHeight: true; visible: !deviceList.visible }
    ListView {
      id: deviceList
      objectName: "t3DeviceList"
      Layout.fillWidth: true; Layout.fillHeight: true; clip: true; spacing: Style.space(4)
      visible: root.supportOn
      model: root.devices
      delegate: Rectangle {
        required property var modelData
        width: ListView.view.width; height: Style.space(48); radius: Style.space(6)
        color: rowHover.hovered ? Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.06) : "transparent"
        HoverHandler { id: rowHover }
        RowLayout {
          anchors.fill: parent; anchors.leftMargin: Style.space(10); anchors.rightMargin: Style.space(6); spacing: Style.space(8)
          T3Icon { name: "smartphone"; tint: modelData.booted ? Color.accent : root.dim; implicitWidth: Style.space(16) }
          ColumnLayout {
            Layout.fillWidth: true; spacing: 0
            Text { Layout.fillWidth: true; text: modelData.name; color: root.fg; font.pixelSize: Style.font.body; elide: Text.ElideRight; textFormat: Text.PlainText }
            Text {
              Layout.fillWidth: true; elide: Text.ElideRight; color: root.dim; font.pixelSize: Style.font.caption; textFormat: Text.PlainText
              objectName: "t3DeviceDetail"
              // Native versions already name the platform ("Android 14.0").
              text: (modelData.version && /^(ios|android)\b/i.test(modelData.version) ? modelData.version : (modelData.platform === "ios" ? "iOS" : "Android") + (modelData.version ? " " + modelData.version : "")) + " · " + root.hostLabel(modelData.hostId) + (modelData.booted ? " · " + T3I18n.tr("Running") : "")
            }
          }
          T3PickerButton {
            objectName: "t3DeviceOpen_" + modelData.id
            primary: !root.sessionFor(modelData); text: T3I18n.tr(root.sessionFor(modelData) ? "Show" : modelData.booted ? "Open" : "Start")
            enabled: !!root.bridge.selectedThread && root.deviceData.permissions.operate === true
            onClicked: root.open(modelData)
          }
        }
      }
    }
  }

  // Device screen.
  ColumnLayout {
    anchors.fill: parent; anchors.margins: Style.space(12); spacing: Style.space(8)
    visible: root.viewing
    RowLayout {
      Layout.fillWidth: true; spacing: Style.space(6)
      T3IconButton { objectName: "t3DeviceBack"; name: "arrow-left"; tooltipText: T3I18n.tr("Back to devices"); onClicked: root.bridge.closeDeviceScreen(false, false) }
      Text {
        Layout.fillWidth: true; elide: Text.ElideRight; color: root.fg; font.pixelSize: Style.font.body; textFormat: Text.PlainText
        text: root.screen.target ? root.screen.target.name : ""
      }
      // Close only stops viewing (the session stays); power off ends the
      // session and shuts the device down, like native T3.
      T3IconButton { objectName: "t3DeviceSettingsToggle"; name: "sliders-horizontal"; tooltipText: T3I18n.tr("Device settings"); tint: settingsBar.visible ? Color.accent : root.dim; enabled: root.screen.stage === "live"; onClicked: { settingsBar.shown = !settingsBar.shown; if (settingsBar.shown) root.bridge.deviceSetting(null) } }
      T3PickerButton { objectName: "t3DevicePowerOff"; primary: false; text: T3I18n.tr("Power off"); enabled: root.screen.stage !== "closing"; onClicked: root.bridge.closeDeviceScreen(true, true) }
    }
    // Device settings and actions from T3's device panel (device.action).
    Flow {
      id: settingsBar
      objectName: "t3DeviceSettings"
      property bool shown: false
      readonly property var settings: ((root.bridge.deviceSettings || {}).detail || {}).settings || ({})
      readonly property bool busy: !!(root.bridge.deviceSettings || {}).busy
      Layout.fillWidth: true; spacing: Style.space(6); visible: shown && root.screen.stage === "live"
      T3PickerButton { objectName: "t3DeviceAppearance"; primary: false; implicitHeight: Style.space(28); enabled: !settingsBar.busy; text: T3I18n.tr(settingsBar.settings.appearance === "dark" ? "Light appearance" : "Dark appearance"); onClicked: root.bridge.deviceSetting({ type: "setAppearance", value: settingsBar.settings.appearance === "dark" ? "light" : "dark" }) }
      T3PickerButton { objectName: "t3DeviceRotate"; primary: false; implicitHeight: Style.space(28); enabled: !settingsBar.busy; text: T3I18n.tr("Rotate"); onClicked: { var o = root.screen.config && root.screen.config.orientation; root.bridge.deviceSetting({ type: "setOrientation", value: o === "landscape_left" || o === "landscape_right" ? "portrait" : "landscape_left" }) } }
      T3PickerButton { objectName: "t3DeviceNetwork"; primary: false; implicitHeight: Style.space(28); enabled: !settingsBar.busy; text: T3I18n.tr(settingsBar.settings.networkEnabled === false ? "Turn network on" : "Turn network off"); onClicked: root.bridge.deviceSetting({ type: "setToggle", setting: "networkEnabled", value: settingsBar.settings.networkEnabled === false }) }
      T3Select { objectName: "t3DeviceTextSize"; implicitWidth: Style.space(130); enabled: !settingsBar.busy; model: [T3I18n.tr("Small text"), T3I18n.tr("Default text"), T3I18n.tr("Large text"), T3I18n.tr("Extra large text")]; currentIndex: Math.max(0, ["small", "default", "large", "extra-large"].indexOf(settingsBar.settings.textSize || "default")); onActivated: function(i) { root.bridge.deviceSetting({ type: "setTextSize", value: ["small", "default", "large", "extra-large"][i] }) } }
      T3PickerButton { objectName: "t3DeviceShake"; primary: false; implicitHeight: Style.space(28); enabled: !settingsBar.busy; text: T3I18n.tr("Shake"); onClicked: root.bridge.deviceSetting({ type: "shake" }) }
      Controls.TextField { id: deviceUrl; objectName: "t3DeviceUrl"; implicitWidth: Style.space(200); placeholderText: T3I18n.tr("Open URL on device"); color: root.fg; font.pixelSize: Style.font.caption; onAccepted: if (text.trim()) { root.bridge.deviceSetting({ type: "openUrl", url: text.trim() }); text = "" } }
      Text { visible: !!(root.bridge.deviceSettings || {}).error; text: T3I18n.error((root.bridge.deviceSettings || {}).error || ""); color: Color.urgent; font.pixelSize: Style.font.caption }
    }
    Text {
      Layout.fillWidth: true; wrapMode: Text.Wrap; visible: text.length > 0 && root.screen.stage !== "live"
      text: root.stageText(); color: root.screen.stage === "error" ? Color.urgent : root.dim; font.pixelSize: Style.font.caption
    }
    Item {
      id: stage
      objectName: "t3DeviceScreen"
      Layout.fillWidth: true; Layout.fillHeight: true
      activeFocusOnTab: true
      Keys.onPressed: function(event) { if (root.sendKey(event)) event.accepted = true }
      WorkIndicator { anchors.centerIn: parent; visible: !root.frame.url && root.screen.stage !== "error"; active: visible; implicitWidth: Style.space(22) }
      Image {
        id: shot
        objectName: "t3DeviceFrame"
        anchors.fill: parent
        source: root.frame.url ? root.frame.url + "#" + root.frame.revision : ""
        fillMode: Image.PreserveAspectFit; cache: false; asynchronous: true; retainWhileLoading: true; smooth: true
        // Normalized point on the device screen, or null outside it.
        function unit(x, y) {
          var w = paintedWidth, h = paintedHeight
          if (w <= 0 || h <= 0) return null
          var nx = (x - (width - w) / 2) / w, ny = (y - (height - h) / 2) / h
          return nx < 0 || ny < 0 || nx > 1 || ny > 1 ? null : {x: nx, y: ny}
        }
        MouseArea {
          anchors.fill: parent; enabled: root.screen.stage === "live" && root.screen.input; cursorShape: Qt.PointingHandCursor
          property bool down: false
          function touch(phase, m) { var p = shot.unit(m.x, m.y); if (!p) { if (phase !== "up") return; p = {x: Math.max(0, Math.min(1, (m.x - (shot.width - shot.paintedWidth) / 2) / shot.paintedWidth)), y: Math.max(0, Math.min(1, (m.y - (shot.height - shot.paintedHeight) / 2) / shot.paintedHeight))} } root.bridge.deviceScreenInput({type: "touch", phase: phase, x: p.x, y: p.y}) }
          onPressed: function(m) { stage.forceActiveFocus(); down = true; touch("down", m) }
          onPositionChanged: function(m) { if (down) touch("move", m) }
          onReleased: function(m) { if (down) touch("up", m); down = false }
          onCanceled: down = false
        }
      }
    }
    RowLayout {
      Layout.alignment: Qt.AlignHCenter; spacing: Style.space(10)
      visible: root.screen.stage === "live" && root.screen.input
      T3IconButton { objectName: "t3DeviceButtonBack"; visible: root.android; name: "arrow-left"; tooltipText: T3I18n.tr("Back"); onClicked: root.bridge.deviceScreenInput({type: "button", button: "back"}) }
      T3IconButton { objectName: "t3DeviceButtonHome"; name: "house"; tooltipText: T3I18n.tr("Home"); onClicked: root.bridge.deviceScreenInput({type: "button", button: "home"}) }
      T3IconButton { objectName: "t3DeviceButtonRecents"; name: "square"; tooltipText: T3I18n.tr("Recent apps"); onClicked: root.bridge.deviceScreenInput({type: "button", button: "recents"}) }
      T3IconButton { objectName: "t3DeviceButtonPower"; name: "power"; tooltipText: T3I18n.tr(root.android ? "Power" : "Lock"); onClicked: root.bridge.deviceScreenInput({type: "button", button: "power"}) }
    }
  }
}
