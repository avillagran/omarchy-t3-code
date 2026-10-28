import QtQuick 6.11
import qs.Commons

// The T3 mark in the bar plus a state indicator (Preferences > Animations >
// Bar indicator). Colour follows the most urgent state: error, then a pending
// question, then work. It moves only while a thread works, in eight steps a
// second: the bar is a wide surface the compositor recomposes for every
// frame, so it never runs a display-rate animation.
Item {
  id: root
  // The O3 mark: "o3-logo", or "o3-omarchy-logo" when the app uses Omarchy's logo.
  property string logoName: "o3-logo"
  // "orbit" | "scanner" | "equalizer" | "heartbeat" | "ticks" | "logo"
  property string style: "orbit"
  property var counts: ({})
  property bool stale: false
  property bool motion: T3Motion.enabled
  readonly property bool working: (counts.working || 0) > 0
  readonly property string tone: (counts.failed || 0) > 0 ? "failed" : (counts.waiting || 0) > 0 ? "waiting" : working ? "working" : "idle"
  readonly property color toneColor: tone === "failed" ? T3Palette.failed : tone === "waiting" ? T3Palette.waiting : T3Palette.working
  readonly property bool shown: tone !== "idle"
  readonly property color mark: stale ? Color.muted : Color.accent
  readonly property real u: Style.space(1)
  width: Style.space(24); height: width
  property real t: 0
  Timer {
    interval: 125; repeat: true; running: root.motion && root.working && root.visible
    onTriggered: root.t = (root.t + 0.125) % 3600
    onRunningChanged: if (!running) root.t = 0.4
  }

  // Mark. The living logo style tints it; every other style keeps it neutral.
  T3Icon {
    id: logo; objectName: "t3BarLogo"; name: root.logoName; tint: root.style === "logo" && root.shown ? root.toneColor : root.mark
    implicitWidth: Style.space(root.style === "logo" ? 19 : root.style === "equalizer" ? 14 : 15)
    x: root.style === "equalizer" ? Style.space(1) : (root.width - width) / 2
    y: (root.height - height) / 2 - (root.style === "scanner" ? Style.space(1.5) : 0)
  }

  // A: a dot with a two-step tail circling the mark; a still dot when idle.
  Repeater {
    model: root.style === "orbit" && root.shown ? 3 : 0
    Rectangle {
      required property int index
      readonly property real angle: root.working ? root.t * 2 * Math.PI / 1.6 - index * 0.5 : -Math.PI / 4
      width: root.u * [3.4, 2.6, 2][index]; height: width; radius: width / 2; color: root.toneColor
      opacity: root.working ? [1, 0.55, 0.25][index] : index ? 0 : 1
      x: root.width / 2 + root.u * 11 * Math.cos(angle) - width / 2
      y: root.height / 2 + root.u * 10 * Math.sin(angle) - height / 2
    }
  }

  // B: a line under the mark with a highlight sweeping back and forth.
  Item {
    visible: root.style === "scanner" && root.shown
    x: Style.space(2); y: Style.space(19.5); width: Style.space(20); height: Style.space(1.6)
    Rectangle { anchors.fill: parent; radius: height / 2; color: root.toneColor; opacity: root.working ? 0.25 : 1 }
    Rectangle {
      visible: root.working; width: Style.space(7); height: parent.height; radius: height / 2; color: root.toneColor
      readonly property real sweep: { var p = (root.t / 1.4) % 2, q = p < 1 ? p : 2 - p; return q < 0.5 ? 2 * q * q : 1 - 2 * (1 - q) * (1 - q) }
      x: (parent.width - width) * sweep
    }
  }

  // C: three bars bouncing beside the mark; short and still when idle.
  Repeater {
    model: root.style === "equalizer" && root.shown ? 3 : 0
    Rectangle {
      required property int index
      readonly property real level: root.working ? 2.5 + 7 * Math.abs(Math.sin(root.t * [5.1, 6.7, 4.3][index] + index * 1.3)) : 2.5
      x: Style.space(16.5 + index * 2.6); width: Style.space(1.7); radius: width / 2; color: root.toneColor
      height: Style.space(level); y: Style.space(16.5) - height
    }
  }

  // D: a dot on the mark's corner sending out a ring each second.
  Item {
    visible: root.style === "heartbeat" && root.shown
    x: Style.space(21) - width / 2; y: Style.space(6) - height / 2; width: Style.space(4.6); height: width
    Rectangle {
      readonly property real p: (root.t / 1.2) % 1
      visible: root.working; anchors.centerIn: parent
      width: Style.space(4.6 + 9 * p); height: width; radius: width / 2
      color: "transparent"; border.width: Style.space(1); border.color: root.toneColor; opacity: (1 - p) * 0.9
    }
    Rectangle { anchors.fill: parent; radius: width / 2; color: root.toneColor; border.width: Style.space(0.8); border.color: Color.popups.background }
  }

  // E: twelve ticks around the mark lighting up in turn; all lit when idle.
  Repeater {
    model: root.style === "ticks" && root.shown ? 12 : 0
    Rectangle {
      required property int index
      readonly property int head: Math.floor(root.t * 12 / 1.5) % 12
      width: Style.space(1.3); height: Style.space(2.3); radius: width / 2; color: root.toneColor
      x: root.width / 2 - width / 2; y: root.height / 2 - Style.space(11.3)
      transform: Rotation { origin.x: Style.space(0.65); origin.y: Style.space(11.3); angle: index * 30 }
      opacity: root.working ? Math.max(0.12, 1 - ((head - index + 12) % 12) / 5) : 0.85
    }
  }

  // F: a light band crossing the tinted mark.
  Item {
    visible: root.style === "logo" && root.working
    readonly property real sweep: ((root.t / 2.2) % 1) * 1.6 - 0.3
    x: logo.x + logo.width * sweep - width / 2; y: logo.y; width: logo.width * 0.24; height: logo.height
    clip: true
    T3Icon { name: root.logoName; tint: "white"; implicitWidth: logo.width; x: logo.x - parent.x; opacity: 0.85 }
  }
}
