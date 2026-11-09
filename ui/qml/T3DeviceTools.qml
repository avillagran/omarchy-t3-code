import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Host commands use the captured open screen, never an editable device target.
Controls.ScrollView {
  id: root
  objectName: "t3DeviceTools"
  required property var bridge
  readonly property var screen: bridge.deviceScreen
  readonly property var state: bridge.deviceSettings || ({})
  readonly property var detail: state.detail
  readonly property var settings: detail && detail.settings || ({})
  readonly property bool ios: !!screen.target && screen.target.platform === "ios"
  readonly property bool hostCommands: !!screen.target && ["local", "ssh"].indexOf(screen.target.hostKind || "") >= 0
  readonly property bool blocked: state.busy || !detail || screen.stage !== "live" || !hostCommands
  readonly property string foreground: detail && detail.foregroundApp ? detail.foregroundApp.id || "" : ""
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.65)
  readonly property string deviceRequest: screen.requestId || ""
  onDeviceRequestChanged: { appInput.text=""; url.text=""; latitude.text=""; longitude.text=""; push.text="" }
  clip: true
  contentWidth: availableWidth
  function act(action) { if (!blocked) bridge.deviceSetting(action) }
  function appId() { return appInput.text.trim() || foreground }
  function locationValid() {
    var lat=Number(latitude.text),lon=Number(longitude.text)
    return !!latitude.text.trim() && !!longitude.text.trim() && isFinite(lat) && isFinite(lon) && lat>=-90 && lat<=90 && lon>=-180 && lon<=180
  }
  // Reads refresh the device state; commands are never automatically retried.
  Timer { interval: 2500; running: root.visible && root.screen.stage === "live" && root.hostCommands; repeat: true; triggeredOnStart: true; onTriggered: if (!root.state.busy) root.bridge.deviceSetting(null) }
  ColumnLayout {
    width: root.availableWidth; spacing: Style.space(8)
    RowLayout {
      Layout.fillWidth: true
      Text { text: T3I18n.tr("Device settings"); color: root.fg; font.bold: true; font.pixelSize: Style.font.body }
      Item { Layout.fillWidth: true }
      WorkIndicator { active: root.state.busy; visible: active; implicitWidth: Style.space(14) }
      T3IconButton { objectName: "t3DeviceToolsRefresh"; name: "refresh-cw"; tooltipText: T3I18n.tr("Refresh"); enabled: !root.state.busy; onClicked: root.bridge.deviceSetting(null) }
    }
    Text { Layout.fillWidth: true; wrapMode: Text.Wrap; visible: !!root.state.error; color: Color.urgent; textFormat: Text.PlainText; text: root.state.error === "device_action_pending" ? T3I18n.tr("A device command is still running.") : T3I18n.error(root.state.error || ""); font.pixelSize: Style.font.caption }
    Text { Layout.fillWidth: true; wrapMode: Text.Wrap; visible: !root.hostCommands; color: root.dim; text: T3I18n.tr("Device tools require a local or SSH host."); font.pixelSize: Style.font.caption }
    Text { Layout.fillWidth: true; wrapMode: Text.Wrap; visible: !!(root.screen.target && root.screen.target.hostDiagnostics); color: root.dim; textFormat: Text.PlainText; text: root.screen.target ? root.screen.target.hostDiagnostics || "" : ""; font.pixelSize: Style.font.caption }

    Text { text: T3I18n.tr("App"); color: root.fg; font.bold: true; font.pixelSize: Style.font.caption }
    Text { Layout.fillWidth: true; color: root.dim; textFormat: Text.PlainText; elide: Text.ElideMiddle; text: T3I18n.tr("Foreground app") + ": " + (root.foreground || "—"); font.pixelSize: Style.font.caption }
    Controls.TextField { id: appInput; objectName: "t3DeviceAppId"; Layout.fillWidth: true; placeholderText: T3I18n.tr("App ID"); font.pixelSize: Style.font.caption; color: root.fg }
    RowLayout {
      T3PickerButton { objectName: "t3DeviceLaunchApp"; primary: false; text: T3I18n.tr("Launch app"); enabled: !root.blocked && !!root.appId(); onClicked: root.act({type:"launchApp",appId:root.appId()}) }
      T3PickerButton { objectName: "t3DeviceTerminateApp"; primary: false; text: T3I18n.tr("Terminate app"); enabled: !root.blocked && !!root.appId(); onClicked: root.act({type:"terminateApp",appId:root.appId()}) }
    }
    RowLayout {
      Layout.fillWidth: true
      Controls.TextField { id: url; objectName: "t3DeviceUrl"; Layout.fillWidth: true; placeholderText: T3I18n.tr("Open URL on device"); font.pixelSize: Style.font.caption; color: root.fg; onAccepted: if (text.trim()) root.act({type:"openUrl",url:text.trim()}) }
      T3PickerButton { objectName: "t3DeviceOpenUrl"; primary: false; text: T3I18n.tr("Open"); enabled: !root.blocked && !!url.text.trim(); onClicked: root.act({type:"openUrl",url:url.text.trim()}) }
    }
    Flow {
      Layout.fillWidth: true; spacing: Style.space(5)
      T3PickerButton { objectName: "t3DeviceAppearance"; primary: false; text: T3I18n.tr(root.settings.appearance === "dark" ? "Light appearance" : "Dark appearance"); enabled: !root.blocked && !!root.settings.appearance; onClicked: root.act({type:"setAppearance",value:root.settings.appearance === "dark" ? "light" : "dark"}) }
      T3Select { objectName: "t3DeviceTextSize"; implicitWidth: Style.space(145); enabled: !root.blocked && !!root.settings.textSize; model: [T3I18n.tr("Small text"),T3I18n.tr("Default text"),T3I18n.tr("Large text"),T3I18n.tr("Extra large text")]; currentIndex: Math.max(0,["small","default","large","extra-large"].indexOf(root.settings.textSize || "default")); onActivated: function(i) { root.act({type:"setTextSize",value:["small","default","large","extra-large"][i]}) } }
      T3Select { objectName: "t3DeviceOrientation"; implicitWidth: Style.space(145); visible: !root.ios; enabled: !root.blocked; model: [T3I18n.tr("Portrait"),T3I18n.tr("Landscape left"),T3I18n.tr("Upside down"),T3I18n.tr("Landscape right")]; onActivated: function(i) { root.act({type:"setOrientation",value:["portrait","landscape_left","portrait_upside_down","landscape_right"][i]}) } }
      T3PickerButton { objectName: "t3DeviceShake"; primary: false; visible: root.ios; text: T3I18n.tr("Shake"); enabled: !root.blocked; onClicked: root.act({type:"shake"}) }
    }
    RowLayout {
      visible: root.ios; Layout.fillWidth: true
      T3Select { objectName: "t3DeviceLiquidGlass"; Layout.fillWidth: true; enabled: !root.blocked && root.settings.liquidGlass !== undefined; model: [T3I18n.tr("Clear glass"),T3I18n.tr("Tinted glass")]; currentIndex: root.settings.liquidGlass === "tinted" ? 1 : 0; onActivated: function(i) { root.act({type:"setLiquidGlass",value:i ? "tinted" : "clear"}) } }
      T3Select { objectName: "t3DeviceColorFilter"; Layout.fillWidth: true; enabled: !root.blocked && root.settings.colorFilter !== undefined; model: [T3I18n.tr("No color filter"),T3I18n.tr("Grayscale"),T3I18n.tr("Red / green"),T3I18n.tr("Green / red"),T3I18n.tr("Blue / yellow")]; currentIndex: Math.max(0,["none","grayscale","red-green","green-red","blue-yellow"].indexOf(root.settings.colorFilter || "none")); onActivated: function(i) { root.act({type:"setColorFilter",value:["none","grayscale","red-green","green-red","blue-yellow"][i]}) } }
    }
    Repeater {
      model: [{key:"reduceMotion",label:"Reduce motion",ios:false},{key:"increaseContrast",label:"Increase contrast",ios:true},{key:"reduceTransparency",label:"Reduce transparency",ios:true},{key:"showBorders",label:"Show borders",ios:true},{key:"voiceOver",label:"VoiceOver",ios:true},{key:"networkEnabled",label:"Network",android:true}]
      Controls.CheckBox {
        required property var modelData
        objectName: "t3DeviceToggle_" + modelData.key
        visible: modelData.ios ? root.ios : modelData.android ? !root.ios : true
        text: T3I18n.tr(modelData.label); font.pixelSize: Style.font.caption
        checked: root.settings[modelData.key] === true
        enabled: !root.blocked && typeof root.settings[modelData.key] === "boolean"
        onClicked: root.act({type:"setToggle",setting:modelData.key,value:checked})
      }
    }
    Text { text: T3I18n.tr("Location"); color: root.fg; font.bold: true; font.pixelSize: Style.font.caption }
    RowLayout {
      Layout.fillWidth: true
      Controls.TextField { id: latitude; objectName: "t3DeviceLatitude"; Layout.fillWidth: true; placeholderText: T3I18n.tr("Latitude"); color: root.fg; font.pixelSize: Style.font.caption }
      Controls.TextField { id: longitude; objectName: "t3DeviceLongitude"; Layout.fillWidth: true; placeholderText: T3I18n.tr("Longitude"); color: root.fg; font.pixelSize: Style.font.caption }
    }
    RowLayout {
      T3PickerButton { objectName: "t3DeviceSetLocation"; primary: false; text: T3I18n.tr("Set location"); enabled: !root.blocked && root.locationValid(); onClicked: root.act({type:"setLocation",latitude:Number(latitude.text),longitude:Number(longitude.text)}) }
      T3PickerButton { objectName: "t3DeviceClearLocation"; primary: false; visible: root.ios; text: T3I18n.tr("Clear location"); enabled: !root.blocked; onClicked: root.act({type:"clearLocation"}) }
    }
    Text { text: T3I18n.tr("Permissions"); color: root.fg; font.bold: true; font.pixelSize: Style.font.caption }
    RowLayout {
      Layout.fillWidth: true
      T3Select { id: permission; objectName: "t3DevicePermission"; Layout.fillWidth: true; property var values: root.ios ? ["camera","microphone","photos","contacts","calendar","reminders","location","notifications","motion","media-library","faceid"] : ["camera","microphone","photos","contacts","calendar","location","notifications","motion"]; model: values.map(function(v){return T3I18n.tr(({camera:"Camera",microphone:"Microphone",photos:"Photos",contacts:"Contacts",calendar:"Calendar",reminders:"Reminders",location:"Location",notifications:"Notifications",motion:"Motion","media-library":"Media library",faceid:"Face ID"})[v])}) }
      T3PickerButton { objectName: "t3DeviceGrantPermission"; primary: false; text: T3I18n.tr("Grant"); enabled: !root.blocked && !!root.appId(); onClicked: root.act({type:"setPermission",appId:root.appId(),permission:permission.values[permission.currentIndex],decision:"grant"}) }
      T3PickerButton { objectName: "t3DeviceRevokePermission"; primary: false; text: T3I18n.tr("Revoke"); enabled: !root.blocked && !!root.appId(); onClicked: root.act({type:"setPermission",appId:root.appId(),permission:permission.values[permission.currentIndex],decision:"revoke"}) }
      T3PickerButton { objectName: "t3DeviceResetPermission"; primary: false; visible: root.ios; text: T3I18n.tr("Reset"); enabled: !root.blocked && !!root.appId(); onClicked: root.act({type:"setPermission",appId:root.appId(),permission:permission.values[permission.currentIndex],decision:"reset"}) }
    }
    Text { visible: root.ios; text: T3I18n.tr("Push notification"); color: root.fg; font.bold: true; font.pixelSize: Style.font.caption }
    RowLayout {
      visible: root.ios; Layout.fillWidth: true
      Controls.TextField { id: push; objectName: "t3DevicePushText"; Layout.fillWidth: true; placeholderText: T3I18n.tr("Alert text"); color: root.fg; font.pixelSize: Style.font.caption }
      T3PickerButton { objectName: "t3DeviceSendPush"; primary: false; text: T3I18n.tr("Send"); enabled: !root.blocked && !!root.appId() && !!push.text.trim(); onClicked: root.act({type:"sendPush",appId:root.appId(),payload:push.text.trim()}) }
    }
  }
}
