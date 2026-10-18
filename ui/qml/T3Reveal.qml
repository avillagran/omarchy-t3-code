import QtQuick 6.11
import qs.Commons

// Shows and hides its single child with the transition chosen in
// Preferences. The child keeps its natural size while the reveal clips it
// from the attached edge (top, or the start edge when horizontal), so text
// never reflows mid-animation. Only while moving is the child rendered
// through the effect shader; a settled region costs nothing.
Item {
  id: root
  default property alias content: holder.data
  property bool shown: true
  property bool horizontal: false
  readonly property Item child: holder.children.length ? holder.children[0] : null
  // Natural size along the axis; defaults to the child's implicit size.
  property real size: child && child.visible ? (horizontal ? child.implicitWidth : child.implicitHeight) : 0
  readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software
  readonly property string style: T3Motion.style === "instant" ? "instant"
    : software || T3Motion.style === "spring" ? "spring" : T3Motion.style
  readonly property bool shaded: style !== "spring" && style !== "instant"
  readonly property bool reverse: horizontal && LayoutMirroring.enabled
  property real progress: shown ? 1 : 0
  readonly property real extent: Math.max(0, Math.min(1, progress))
  // Share of the natural size the layout gives it. Nebula dissolves in place
  // first and only then closes the gap (and opens it first when showing).
  readonly property real reach: style === "nebula" ? Math.min(1, extent / 0.6) : extent
  readonly property bool moving: motion.running
  // Visible from the moment it is asked to show, so focus can land at once.
  readonly property bool present: shown || progress > 0
  Behavior on progress {
    enabled: root.style !== "instant"
    NumberAnimation {
      id: motion
      duration: ({nebula: 560, warp: 460, fold: 500, spring: 520})[root.style] * T3Motion.pace
      easing.type: ({nebula: Easing.InOutCubic, warp: Easing.InOutQuart, fold: Easing.InOutSine, spring: Easing.OutBack})[root.style] || Easing.InOutCubic
      easing.overshoot: 1.3
    }
  }
  visible: present
  clip: reach < 1 || moving
  implicitWidth: horizontal ? size * reach : child ? child.implicitWidth : 0
  implicitHeight: horizontal ? (child ? child.implicitHeight : 0) : size * reach
  // Per-instance seed so neighbouring regions never dissolve identically.
  readonly property real seed: Math.random()
  Item {
    id: holder
    readonly property real slide: root.style === "spring" ? (1 - root.extent) * root.size : 0
    width: root.horizontal ? root.size : root.width
    // Once settled it may grow with the layout (fillHeight shelves).
    height: root.horizontal ? root.height : root.extent >= 1 && !root.moving ? Math.max(root.size, root.height) : root.size
    x: root.horizontal ? (root.reverse ? root.width - root.size + slide : -slide) : 0
    y: root.horizontal ? 0 : -slide
    opacity: root.style === "spring" ? Math.min(1, root.extent * 1.8) : 1
    // Spring overshoot stretches the content a touch along the axis.
    transform: Scale {
      origin.x: root.reverse ? holder.width : 0; origin.y: 0
      xScale: root.horizontal && root.style === "spring" ? Math.max(0.9, root.progress) : 1
      yScale: !root.horizontal && root.style === "spring" ? Math.max(0.9, root.progress) : 1
    }
    layer.enabled: root.moving && root.shaded
    layer.effect: ShaderEffect {
      property real progress: root.progress
      property real style: ["nebula", "warp", "fold"].indexOf(root.style)
      property real horizontal: root.horizontal ? 1 : 0
      property real reverse: root.reverse ? 1 : 0
      property real seed: root.seed
      property vector2d size: Qt.vector2d(holder.width, holder.height)
      property color glow: Color.accent
      fragmentShader: Qt.resolvedUrl("../assets/shaders/reveal.frag.qsb")
    }
  }
}
