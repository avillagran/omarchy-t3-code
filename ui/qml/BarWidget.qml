import QtQuick 6.11
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// The bar side of Omarchy T3 Code. The app itself runs in its own process
// (bin/ot3, app/shell.qml); this widget only shows the status the app
// publishes and asks bin/ot3 to show or hide ("minimize to the bar") its
// window. Nothing here talks to T3, so a stuck bar never blocks the app and a
// busy app never slows the bar.
BarWidget {
  id: root
  moduleName: "io.github.avillagran.omarchy-t3-code"
  readonly property string launcher: Qt.resolvedUrl("../../bin/ot3").toString().replace(/^file:\/\//, "")
  readonly property string statusPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/omarchy-t3-code/status.json"
  property var status: ({})
  property real now: Date.now()
  // A status older than 90 s means the app stopped without saying so.
  readonly property bool running: status.running === true && now - (status.updatedAt || 0) < 90000
  readonly property var counts: running && status.counts ? status.counts : ({ working: 0, done: 0, waiting: 0, failed: 0 })
  readonly property bool opened: running && status.opened === true
  readonly property var bridge: running ? status.omabridge : null
  readonly property bool bridgeIndicator: !!root.bridge
  readonly property real statusWidth: root.vertical ? Style.bar.statusSlot : badges.implicitWidth + Style.space(6)
  implicitWidth: root.statusWidth + (root.bridgeIndicator && !root.vertical ? Style.bar.statusSlot : 0)
  implicitHeight: Style.bar.statusSlot * (root.bridgeIndicator && root.vertical ? 2 : 1)

  function read() {
    try { root.status = JSON.parse(statusFile.text() || "{}") || ({}) } catch (e) { root.status = ({}) }
    root.now = Date.now()
  }
  FileView { id: statusFile; path: root.statusPath; watchChanges: true; printErrors: false; onLoaded: root.read(); onFileChanged: reload(); onLoadFailed: root.status = ({}) }
  Timer { interval: 30000; repeat: true; running: true; onTriggered: { root.now = Date.now(); if (!root.running) statusFile.reload() } }

  // Launcher calls run one at a time, in order; a newer anchor replaces a waiting one.
  property var queued: []
  function run(args) {
    if (launch.running) { root.queued = root.queued.filter(function(q) { return !(args[0] === "anchor" && q[0] === "anchor") }).concat([args]); return }
    launch.command = [root.launcher].concat(args); launch.running = true
  }
  Process { id: launch; onExited: if (root.queued.length) { var next = root.queued[0]; root.queued = root.queued.slice(1); root.run(next) } }
  // Where the T3 button sits, so the app can show its panel under it, as the
  // in-shell popup did (app/shell.qml draws a transparent twin of this bar).
  function anchor() {
    var w = button.QsWindow.window
    if (!w || !w.contentItem) return ""
    var p = button.mapToItem(w.contentItem, 0, 0)
    var position = root.bar && root.bar.position ? root.bar.position : "top"
    var barSize = root.barSize || Style.bar.statusSlot
    // Newer Omarchy bars span the whole screen (for dragging) and reserve only their
    // strip through an explicit exclusive zone: report that strip, not the window.
    var vertical = position === "left" || position === "right"
    var full = vertical ? w.width : w.height
    var strip = w.exclusionMode === ExclusionMode.Normal && w.exclusiveZone > 0 ? Math.min(full, w.exclusiveZone) : full
    var x = position === "right" ? p.x - (full - strip) : p.x
    var y = position === "bottom" ? p.y - (full - strip) : p.y
    return JSON.stringify({ screen: w.screen ? w.screen.name : "", position: position, barSize: barSize,
      windowWidth: vertical ? strip : w.width, windowHeight: vertical ? w.height : strip, x: x, y: y, width: button.width, height: button.height })
  }
  function anchored(command) { var a = anchor(); return a ? [command, a] : [command] }
  // Omarchy's summon (Super+Shift+A through the shell) and other plugins use these.
  function open() { run(anchored("show")) }
  function close() { run(["hide"]) }
  function togglePanel() { run(anchored("toggle")) }
  // The app starts hidden with the bar, so thread notifications keep arriving
  // as they did when it lived inside the shell; then it learns where this button is.
  Component.onCompleted: { run(["start"]); publishAnchor.restart() }
  Timer { id: publishAnchor; interval: 2500; onTriggered: { var a = root.anchor(); if (a) root.run(["anchor", a]) } }
  onXChanged: publishAnchor.restart()
  onYChanged: publishAnchor.restart()

  Loader {
    active: root.bridgeIndicator
    x: root.vertical ? 0 : root.statusWidth
    y: root.vertical ? Style.bar.statusSlot : 0
    width: Style.bar.statusSlot
    height: Style.bar.statusSlot
    sourceComponent: Component {
      BarIconButton {
        objectName: "omabridgeIndicator"
        bar: root.bar
        slotSize: Style.bar.statusSlot
        tooltipText: "OmaBridge · " + (root.bridge.text || "")
        iconComponent: Component { Item { implicitWidth: Style.space(16); implicitHeight: implicitWidth
          T3Icon { objectName: "omabridgeIndicatorIcon"; anchors.fill: parent; name: root.bridge.linked ? "omabridge" : "omabridge-off"; tint: root.bridge.linked ? Color.popups.text : Qt.rgba(Color.popups.text.r, Color.popups.text.g, Color.popups.text.b, 0.58) }
          Rectangle { objectName: "omabridgeIndicatorBadge"; visible: root.bridge.pending > 0; width: Style.space(8); height: width; radius: width / 2; color: Color.urgent; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: -Style.space(2) }
        } }
        onPressed: function(mouseButton) { root.run(mouseButton === Qt.MiddleButton ? ["call", "refresh"] : ["call", "preferences", "connections"]) }
      }
    }
  }
  BarIconButton {
    id: button
    x: 0
    y: 0
    width: root.vertical ? Style.bar.statusSlot : root.statusWidth
    height: Style.bar.statusSlot
    bar: root.bar
    active: root.opened
    slotSize: root.vertical ? Style.bar.statusSlot : root.statusWidth
    // The app writes its tooltip already translated; the bar loads no translations.
    tooltipText: root.running ? (root.status.tooltip || "T3 Code") : "T3 Code"
    iconComponent: Component { Item {} }
    T3StatusBadges { id: badges; anchors.centerIn: parent; counts: root.counts; stale: !root.running || root.status.stale === true; indicatorStyle: root.status.indicatorStyle || "orbit"; motion: root.status.animationsEnabled !== false; width: implicitWidth; height: Style.bar.statusSlot; scale: root.vertical ? Math.min(1, (root.barSize - Style.space(4)) / implicitWidth) : 1 }
    onPressed: function(mouseButton) {
      if (mouseButton === Qt.LeftButton) root.togglePanel()
      else if (mouseButton === Qt.MiddleButton) root.run(["call", "refresh"])
    }
  }
}
