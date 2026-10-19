import QtQuick 6.11
import qs.Commons

// Frame rate readout (Preferences > Animations). Counts the frames the window
// actually presents, so a still panel reads low: only motion produces frames.
// The longest gap in each half second shows stutter that an average hides;
// a panel at rest reads as ambient motion (Animation smoothness) or idle.
// The readout itself repaints twice a second at most.
Rectangle {
  id: root
  property bool active: false
  readonly property var target: Window.window
  property int frames: 0
  property real worst: 0
  property real last: 0
  property real fps: 0
  property real slowest: 0
  visible: active
  width: label.implicitWidth + Style.space(12); height: label.implicitHeight + Style.space(6)
  radius: Style.space(5)
  color: Qt.rgba(0, 0, 0, 0.62)
  border.width: 1; border.color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.45)
  Connections {
    target: root.active ? root.target : null
    function onFrameSwapped() {
      var now = Date.now()
      if (root.last) root.worst = Math.max(root.worst, now - root.last)
      root.last = now; root.frames++
    }
  }
  Timer {
    interval: 500; repeat: true; running: root.active
    onRunningChanged: { root.frames = 0; root.worst = 0; root.last = 0; root.fps = 0; root.slowest = 0 }
    onTriggered: {
      var fps = root.frames * 2, slowest = Math.round(root.worst)
      root.frames = 0; root.worst = 0
      if (fps !== root.fps || slowest !== root.slowest) { root.fps = fps; root.slowest = slowest }
    }
  }
  Text {
    id: label; anchors.centerIn: parent
    // Without scrolling or transitions the panel only paints the ambient
    // motion clock; say so instead of flagging it as a drop.
    readonly property int ambient: T3Ticker.interval > 0 ? Math.round(1000 / T3Ticker.interval) : 0
    readonly property bool resting: root.fps <= 4 || ambient > 0 && root.fps <= ambient * 1.6 + 2
    text: root.fps + " fps · " + (resting ? (ambient && root.fps > 4 ? T3I18n.tr("ambient motion") : T3I18n.tr("idle")) : T3I18n.tr("slowest") + " " + root.slowest + " ms")
    color: resting ? "#c9d1d9" : root.fps >= 100 ? "#7ee787" : root.fps >= 50 ? "#e3b341" : "#ff7b72"
    font.pixelSize: Style.font.caption; font.family: "monospace"
  }
}
