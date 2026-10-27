import QtQuick 6.11
import qs.Commons

// A sidebar row travelling between sections (Working, inbox, Snoozed,
// Settled). The old row's picture morphs into the new one while the card
// follows the path chosen in Preferences > Animations; both real rows stay
// hidden until it lands. Without a visible destination row it sinks into the
// section header instead. Created by T3Panel.startFlights.
Item {
  id: root
  property Item source: null
  property Item target: null
  property Item landing: null
  property rect start: Qt.rect(0, 0, 0, 0)
  property string key: ""
  property string path: "arc"
  property int duration: T3Motion.duration(520)
  property real t: 0
  readonly property real bow: Math.sin(Math.PI * t)
  readonly property real blend: smooth(0.18, 0.82, t)
  // Taller travel bows further out; short hops stay close to the list.
  property real swing: 0
  property real tilt: 0
  readonly property color fg: Color.popups.text
  function smooth(a, b, v) { var k = Math.max(0, Math.min(1, (v - a) / (b - a))); return k * k * (3 - 2 * k) }
  function destination() {
    var item = target || landing
    if (!item || !item.visible && !target) return Qt.rect(start.x, start.y, start.width, start.height)
    var p = item.mapToItem(parent, 0, 0)
    return Qt.rect(p.x, p.y, target ? item.width : start.width, target ? item.height : item.height)
  }
  // A recycled delegate now shows another thread: let it go.
  function owns(item) { return !!item && !item.pooled && (item.thread.key || item.thread.id) === key }
  function place() {
    if (target && !owns(target)) target = null
    if (source && !owns(source)) source = null
    // The destination is read every frame: shelves above it may still be
    // opening or closing while the card travels.
    var to = destination(), k = t
    x = start.x + (to.x - start.x) * k + (path === "arc" ? swing * bow : 0)
    y = start.y + (to.y - start.y) * k
    width = start.width + (to.width - start.width) * k
    height = start.height + (to.height - start.height) * k
  }
  onTChanged: place()
  Component.onCompleted: {
    if (!key && source) key = source.thread.key || source.thread.id
    var to = destination()
    swing = path === "arc" ? Math.min(Style.space(26), Math.abs(to.y - start.y) * 0.14) : 0
    tilt = path === "arc" ? (to.y < start.y ? 1.4 : -1.4) : 0
    place(); flight.start()
  }
  opacity: target ? 1 : 1 - smooth(0.55, 1, t)
  scale: 1 + (path === "arc" ? 0.04 : 0.015) * bow
  rotation: tilt * bow
  z: 1 + bow
  // Lifted card: soft shadow and an accent rim that peak mid-flight, so the
  // ends sit exactly on the real rows without a visible seam.
  Rectangle {
    x: Style.space(2); y: Style.space(5) * root.bow; width: parent.width; height: parent.height
    radius: Style.space(8); color: Qt.rgba(0, 0, 0, 0.32 * root.bow)
  }
  Rectangle {
    anchors.fill: parent; radius: Style.space(6)
    color: Qt.tint(Color.popups.background, Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.06))
    opacity: Math.min(1, root.bow * 2.6)
    border.width: 1; border.color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.55 * root.bow)
  }
  Item {
    anchors.fill: parent; clip: true
    ShaderEffectSource {
      anchors.verticalCenter: parent.verticalCenter
      width: root.start.width; height: root.start.height
      sourceItem: root.source; hideSource: !!root.source; live: true
      opacity: 1 - root.blend
    }
    ShaderEffectSource {
      anchors.verticalCenter: parent.verticalCenter
      visible: !!root.target
      width: root.target ? root.target.width : 0; height: root.target ? root.target.height : 0
      sourceItem: root.target; hideSource: !!root.target; live: true
      opacity: root.blend
    }
  }
  NumberAnimation {
    id: flight; target: root; property: "t"; from: 0; to: 1; duration: T3Motion.duration(root.duration)
    easing.type: root.path === "arc" ? Easing.InOutQuart : Easing.InOutCubic
    onFinished: { if (root.target) root.target.flight = false; root.destroy() }
  }
}
