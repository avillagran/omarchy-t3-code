import QtQuick 6.11
import QtQuick.Window 6.11
import QtQuick.Shapes
import QtQuick.Effects
import qs.Commons
import "../js/BrandPixels.js" as Pixels

// The O3 Code wordmark. At rest it reads "O3 Code" with an empty line below.
// Every few seconds the O becomes the T of T3 (with a squash, a glow, a sheen
// and a spark burst), Omarchy's pixel wordmark drops in letter by letter,
// holds, falls away, and the T turns back into an O.
// variant "o": a vector O whose outline morphs into the T, with chromatic ghosts.
// variant "omarchy": Omarchy's logo, which breaks into its blocks and rebuilds as the T.
// Holds run on a plain Timer so the window renders nothing between scenes.
Item {
  id: root
  property color fg: Color.popups.text
  property color dim: Color.popups.text
  property color accent: Color.accent
  // The cycle only runs while the panel is shown and motion is on.
  property bool running: true
  readonly property bool animate: running && T3Motion.enabled
  readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software
  property string variant: "o"
  readonly property bool blocks: variant === "omarchy"
  readonly property var wordmark: Pixels.WORDMARK
  property int restMs: 7000
  property int holdMs: 3600
  readonly property real pace: Math.max(0.3, T3Motion.pace)
  readonly property int morphMs: Math.round((blocks ? 1400 : 1100) * pace)

  // Glyph geometry, in the logo's SVG units (viewBox 15.5309 37 94.3941 56.96).
  readonly property real unit: Style.space(32) / 94.3941
  readonly property real originX: 15.5309
  readonly property real originY: 37
  readonly property string threePath: "M86.7253 93.96C82.832 93.96 78.9653 93.4533 75.1253 92.44C71.2853 91.3733 68.032 89.88 65.3653 87.96L70.4053 78.04C72.5386 79.5867 75.0186 80.8133 77.8453 81.72C80.672 82.6267 83.5253 83.08 86.4053 83.08C89.6586 83.08 92.2186 82.44 94.0853 81.16C95.952 79.88 96.8853 78.12 96.8853 75.88C96.8853 73.7467 96.0586 72.0667 94.4053 70.84C92.752 69.6133 90.0853 69 86.4053 69H80.4853V60.44L96.0853 42.76L97.5253 47.4H68.1653V37H107.365V45.4L91.8453 63.08L85.2853 59.32H89.0453C95.9253 59.32 101.125 60.8667 104.645 63.96C108.165 67.0533 109.925 71.0267 109.925 75.88C109.925 79.0267 109.099 81.9867 107.445 84.76C105.792 87.48 103.259 89.6933 99.8453 91.4C96.432 93.1067 92.0586 93.96 86.7253 93.96Z"

  // 0 = O, 1 = T.
  property real morph: 0
  property real squashX: 1
  property real squashY: 1
  property real chroma: 0
  property real glow: 0
  property real sheen: -0.4
  property real burst: 1
  property real wordShift: 0
  property int step: 0
  // Reduced motion shows the full brand, still.
  readonly property bool spelled: !T3Motion.enabled

  readonly property var shapes: buildShapes(96, 40)
  readonly property var outline: blend(shapes, blocks ? 1 : morph)
  // Logo blocks: a square as tall as the 3, a small gap before it. The 3 and
  // Code make room for the logo and slide back as it becomes the T.
  readonly property real logoSize: 56.96
  readonly property real logoX: originX
  readonly property real logoY: originY
  readonly property real logoRoom: blocks ? (logoX + logoSize + Style.space(10) / 3 / unit - 65.3653) * unit : 0
  readonly property real threeShift: logoRoom * (1 - morph)
  readonly property real cellSize: logoSize / Pixels.LOGO_GRID
  readonly property var flights: buildFlights()
  // Wordmark pixels: whole device pixels, about as wide as the word above.
  // Wordmark pixels: its letters (17 rows tall) stand half as tall as the O3.
  readonly property real pixel: mark.height * 0.5 / 17

  implicitWidth: word.width
  implicitHeight: word.height + Style.space(3) + letters.height

  function lerp(a, b, t) { return a + (b - a) * t }
  // Edges of the T, clockwise from the middle of its top, so every corner is a sample.
  function buildShapes(outerCount, innerCount) {
    var t = [[39.9309, 37], [64.3309, 37], [64.3309, 47.56], [46.4109, 47.56], [46.4109, 93], [33.4509, 93], [33.4509, 47.56], [15.5309, 47.56], [15.5309, 37]]
    var lengths = [], total = 0
    for (var i = 0; i < t.length; i++) {
      var a = t[i], b = t[(i + 1) % t.length], l = Math.hypot(b[0] - a[0], b[1] - a[1])
      lengths.push(l); total += l
    }
    var counts = lengths.map(function(l) { return Math.max(1, Math.round(outerCount * l / total)) })
    var tOuter = []
    for (i = 0; i < t.length; i++) {
      a = t[i]; b = t[(i + 1) % t.length]
      for (var k = 0; k < counts[i]; k++) tOuter.push([lerp(a[0], b[0], k / counts[i]), lerp(a[1], b[1], k / counts[i])])
    }
    // The O shares the T's width and baseline so neither nudges the 3.
    var n = tOuter.length, oOuter = [], oInner = [], tInner = []
    for (i = 0; i < n; i++) {
      var angle = i / n * Math.PI * 2
      oOuter.push([39.2 + 24.8 * Math.sin(angle), 65.2 - 28.6 * Math.cos(angle)])
    }
    for (i = 0; i < innerCount; i++) {
      angle = i / innerCount * Math.PI * 2
      oInner.push([39.2 + 12 * Math.sin(angle), 65.2 - 16.2 * Math.cos(angle)])
      tInner.push([39.93, 72])
    }
    return { oOuter: oOuter, tOuter: tOuter, oInner: oInner, tInner: tInner }
  }
  // Each logo block flies to a cell of the T, on its own delay and arc.
  function buildFlights() {
    var from = Pixels.LOGO_CELLS.map(function(c) { return [logoX + c[0] * cellSize, logoY + c[1] * cellSize] })
    var to = [], cols = Math.round(48.8 / cellSize), rows = Math.round(56 / cellSize)
    for (var j = 0; j < rows; j++) for (var i = 0; i < cols; i++) {
      var x = 15.5309 + i * 48.8 / cols, y = 37 + j * 56 / rows, cx = x + cellSize / 2, cy = y + cellSize / 2
      if (cy < 47.56 || cx > 33.4509 && cx < 46.4109) to.push([x, y])
    }
    var n = Math.max(from.length, to.length), out = []
    for (i = 0; i < n; i++) {
      var a = from[Math.floor(i * from.length / n)], b = to[Math.floor(i * to.length / n)]
      out.push({ ax: a[0], ay: a[1], bx: b[0], by: b[1], delay: (i * 37 % n) / n, arc: ((i * 53 % 17) - 8) * 1.4, lift: 4 + (i * 29 % 7) })
    }
    return out
  }
  // The counter closes ahead of the outline, so the stem never shows a hole.
  function blend(s, m) {
    var hole = Math.min(1, m * 1.4), outer = [], inner = []
    for (var i = 0; i < s.oOuter.length; i++) outer.push(Qt.point(lerp(s.oOuter[i][0], s.tOuter[i][0], m), lerp(s.oOuter[i][1], s.tOuter[i][1], m)))
    outer.push(outer[0])
    for (i = 0; i < s.oInner.length; i++) inner.push(Qt.point(lerp(s.oInner[i][0], s.tInner[i][0], hole), lerp(s.oInner[i][1], s.tInner[i][1], hole)))
    inner.push(inner[0])
    return [outer, inner]
  }

  function rest() {
    cycle.stop(); morphScene.stop()
    morph = 0; squashX = 1; squashY = 1; chroma = 0; glow = 0; sheen = -0.4; burst = 1; wordShift = 0
    for (var i = 0; i < letterRepeater.count; i++) { var cell = letterRepeater.itemAt(i); if (cell) cell.reset(spelled) }
    step = 0
  }
  function play(toT) { morphScene.target = toT ? 1 : 0; morphScene.restart() }
  function spell(show) {
    for (var i = 0; i < letterRepeater.count; i++) { var cell = letterRepeater.itemAt(i); if (cell) show ? cell.enter() : cell.leave() }
  }
  // rest → O becomes T → letters fall in → hold → letters fall away → T becomes O.
  function advance() {
    var count = wordmark.length
    if (step === 0) { play(true); step = 1; cycle.interval = morphMs + 120 }
    else if (step === 1) { spell(true); step = 2; cycle.interval = Math.round((count * 95 + 900) * pace) + holdMs }
    else if (step === 2) { spell(false); step = 3; cycle.interval = Math.round((count * 70 + 560) * pace) }
    else if (step === 3) { play(false); step = 4; cycle.interval = morphMs }
    else { step = 0; cycle.interval = restMs }
    cycle.restart()
  }
  function start() { rest(); if (animate) { cycle.interval = 2400; cycle.restart() } }
  onAnimateChanged: start()
  Component.onCompleted: start()
  Timer { id: cycle; repeat: false; onTriggered: root.advance() }

  // Hovering the mark at rest plays the scene right away.
  HoverHandler { onHoveredChanged: if (hovered && root.animate && root.step === 0) { cycle.stop(); root.advance() } }

  ParallelAnimation {
    id: morphScene
    property real target: 1
    // Anticipation squash, the morph itself, then a springy settle.
    SequentialAnimation {
      ParallelAnimation {
        NumberAnimation { target: root; property: "squashX"; to: 1.1; duration: Math.round(170 * root.pace); easing.type: Easing.OutQuad }
        NumberAnimation { target: root; property: "squashY"; to: 0.86; duration: Math.round(170 * root.pace); easing.type: Easing.OutQuad }
      }
      ParallelAnimation {
        NumberAnimation { target: root; property: "morph"; to: morphScene.target; duration: Math.round((root.blocks ? 920 : 620) * root.pace); easing.type: root.blocks ? Easing.Linear : Easing.InOutCubic }
        NumberAnimation { target: root; property: "squashX"; to: 0.92; duration: Math.round(260 * root.pace); easing.type: Easing.OutQuad }
        NumberAnimation { target: root; property: "squashY"; to: 1.12; duration: Math.round(260 * root.pace); easing.type: Easing.OutQuad }
      }
      ParallelAnimation {
        NumberAnimation { target: root; property: "squashX"; to: 1; duration: Math.round(320 * root.pace); easing.type: Easing.OutBack; easing.overshoot: 3 }
        NumberAnimation { target: root; property: "squashY"; to: 1; duration: Math.round(320 * root.pace); easing.type: Easing.OutBack; easing.overshoot: 3 }
        NumberAnimation { target: root; property: "burst"; from: 0; to: 1; duration: Math.round(520 * root.pace); easing.type: Easing.OutCubic }
      }
    }
    SequentialAnimation {
      PauseAnimation { duration: Math.round(150 * root.pace) }
      NumberAnimation { target: root; property: "chroma"; to: 1; duration: Math.round(260 * root.pace); easing.type: Easing.OutQuad }
      NumberAnimation { target: root; property: "chroma"; to: 0; duration: Math.round(420 * root.pace); easing.type: Easing.InQuad }
    }
    SequentialAnimation {
      PauseAnimation { duration: Math.round(250 * root.pace) }
      NumberAnimation { target: root; property: "glow"; to: 1; duration: Math.round(380 * root.pace); easing.type: Easing.OutQuad }
      NumberAnimation { target: root; property: "glow"; to: 0; duration: Math.round(700 * root.pace); easing.type: Easing.InOutSine }
    }
    SequentialAnimation {
      PauseAnimation { duration: Math.round(380 * root.pace) }
      NumberAnimation { target: root; property: "sheen"; from: -0.4; to: 1.4; duration: Math.round(720 * root.pace); easing.type: Easing.InOutQuad }
    }
    // The word recoils from the glyph, then settles.
    SequentialAnimation {
      PauseAnimation { duration: Math.round(170 * root.pace) }
      NumberAnimation { target: root; property: "wordShift"; to: Style.space(2); duration: Math.round(200 * root.pace); easing.type: Easing.OutQuad }
      NumberAnimation { target: root; property: "wordShift"; to: 0; duration: Math.round(460 * root.pace); easing.type: Easing.OutElastic; easing.amplitude: 1.2; easing.period: 0.4 }
    }
  }

  Item {
    id: word
    objectName: "t3BrandWord"
    width: mark.width + Style.space(4) + codeLabel.implicitWidth
    height: Math.max(mark.height, codeLabel.implicitHeight)
    LayoutMirroring.enabled: false

    // Glow: a blurred, accent-coloured copy of the glyph behind it.
    MultiEffect {
      x: mark.x; y: mark.y; width: glyph.width; height: glyph.height
      visible: !root.software && root.glow > 0.01
      source: glyph
      opacity: root.glow
      autoPaddingEnabled: true
      blurEnabled: true; blur: 1; blurMax: 24
      colorization: 1; colorizationColor: root.accent
      brightness: 0.4
    }

    Item {
      id: ink
      anchors.fill: parent
      Item {
        id: mark
        width: 94.3941 * root.unit + root.threeShift; height: 56.96 * root.unit
        anchors.verticalCenter: parent.verticalCenter

        // Chromatic ghosts split around the glyph while it changes shape.
        Repeater {
          model: [{ dx: -1, tint: root.accent }, { dx: 1, tint: Qt.hsla((root.accent.hslHue + 0.5) % 1, Math.max(0.6, root.accent.hslSaturation), 0.6, 1) }]
          delegate: Item {
            required property var modelData
            width: mark.width; height: mark.height
            visible: !root.blocks && root.chroma > 0.01
            x: modelData.dx * root.chroma * Style.space(3)
            opacity: root.chroma * 0.7
            transform: Scale { origin.x: 39.9 * root.unit - root.originX * root.unit; origin.y: mark.height; xScale: root.squashX; yScale: root.squashY }
            Shape {
              x: -root.originX * root.unit; y: -root.originY * root.unit
              width: 120; height: 100; scale: root.unit; transformOrigin: Item.TopLeft
              preferredRendererType: Shape.CurveRenderer
            layer.enabled: true; layer.samples: 4; layer.smooth: true; layer.mipmap: true
              ShapePath { fillColor: modelData.tint; strokeColor: "transparent"; fillRule: ShapePath.OddEvenFill; PathMultiline { paths: root.outline } }
            }
          }
        }

        Item {
          id: glyph
          objectName: "t3BrandGlyph"
          width: mark.width; height: mark.height
          transform: Scale { origin.x: 39.9 * root.unit - root.originX * root.unit; origin.y: mark.height; xScale: root.squashX; yScale: root.squashY }
          Shape {
            objectName: "t3BrandLogo"
            visible: root.blocks && opacity > 0
            opacity: root.morph < 0.002 ? 1 : 0
            x: (root.logoX - root.originX) * root.unit; y: (root.logoY - root.originY) * root.unit
            width: 1200; height: 1200; scale: root.unit * root.logoSize / 1200; transformOrigin: Item.TopLeft
            preferredRendererType: Shape.CurveRenderer
            ShapePath { fillColor: root.fg; strokeColor: "transparent"; fillRule: ShapePath.OddEvenFill; PathSvg { path: Pixels.LOGO_PATH } }
          }
          // The logo's blocks in flight: they leave in a staggered wave, arc
          // up and sideways, glow with the accent mid-air, and land as the T.
          Repeater {
            model: root.blocks ? root.flights : []
            delegate: Rectangle {
              required property var modelData
              readonly property real span: 0.55
              readonly property real local: Math.max(0, Math.min(1, (root.morph - modelData.delay * (1 - span)) / span))
              readonly property real ease: local < 0.5 ? 4 * local * local * local : 1 - Math.pow(-2 * local + 2, 3) / 2
              readonly property real bump: Math.sin(Math.PI * local)
              visible: root.morph > 0.002 && root.morph < 0.998
              x: (root.lerp(modelData.ax, modelData.bx, ease) + modelData.arc * bump - root.originX) * root.unit
              y: (root.lerp(modelData.ay, modelData.by, ease) - modelData.lift * bump - root.originY) * root.unit
              width: root.cellSize * root.unit * (1 + 0.6 * bump); height: width
              rotation: 90 * bump * (modelData.arc > 0 ? 1 : -1)
              color: Qt.tint(root.fg, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, bump))
            }
          }
          Shape {
            visible: opacity > 0
            opacity: root.blocks ? (root.morph > 0.998 ? 1 : 0) : 1
            x: -root.originX * root.unit; y: -root.originY * root.unit
            width: 120; height: 100; scale: root.unit; transformOrigin: Item.TopLeft
            preferredRendererType: Shape.CurveRenderer
            layer.enabled: true; layer.samples: 4; layer.smooth: true; layer.mipmap: true
            ShapePath { fillColor: root.fg; strokeColor: "transparent"; fillRule: ShapePath.OddEvenFill; PathMultiline { paths: root.outline } }
          }
        }

        // The 3 rasterised from its SVG, like the original logo, so its curves stay smooth.
        Image {
          x: root.wordShift + root.threeShift
          width: 94.3941 * root.unit; height: 56.96 * root.unit
          source: "data:image/svg+xml," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="15.5309 37 94.3941 56.96"><path d="' + root.threePath + '" fill="' + Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 1) + '"/></svg>')
          sourceSize.width: Math.round(width * Screen.devicePixelRatio)
          sourceSize.height: Math.round(height * Screen.devicePixelRatio)
          opacity: root.fg.a
          smooth: true
        }

        // Sparks thrown off when the new glyph lands.
        Repeater {
          model: 9
          delegate: Rectangle {
            required property int index
            readonly property real angle: -Math.PI * (0.08 + 0.84 * index / 8)
            readonly property real reach: Style.space(14) + (index % 3) * Style.space(4)
            visible: root.burst < 0.999
            width: Style.space(2); height: width; radius: width / 2
            color: index % 2 ? root.accent : root.fg
            x: (39.9 - root.originX) * root.unit + Math.cos(angle) * reach * root.burst - width / 2
            y: mark.height * 0.45 + Math.sin(angle) * reach * root.burst - height / 2
            opacity: 1 - root.burst
            scale: 1.4 - root.burst
          }
        }
      }

      Text {
        id: codeLabel
        x: mark.width + Style.space(4) + root.wordShift
        anchors.verticalCenter: parent.verticalCenter
        text: "Code"; font.pixelSize: Style.space(18); font.bold: true; color: root.fg
      }
    }

    // A sheen: two bands of light, a soft wide one and a bright core, that
    // reveal a brightened copy of the letters as they sweep across.
    Repeater {
      model: [{ band: Style.space(16), light: 0.45 }, { band: Style.space(5), light: 1 }]
      delegate: Item {
        required property var modelData
        visible: !root.software && root.sheen > -0.35 && root.sheen < 1.35
        x: root.sheen * word.width - width / 2
        width: modelData.band; height: word.height
        clip: true
        MultiEffect {
          x: -parent.x; width: word.width; height: word.height
          source: ink
          brightness: modelData.light
          opacity: modelData.light
        }
      }
    }
  }

  Item {
    id: letters
    objectName: "t3BrandSubtitle"
    // Starts under the T's stem.
    x: (33.4509 - root.originX) * root.unit
    // A third closer than the word's natural line gap.
    y: word.height + Style.space(3) - ((word.height - mark.height) / 2 + Style.space(3)) / 3
    z: -1
    width: Pixels.WORDMARK_COLUMNS * root.pixel; height: Pixels.WORDMARK_ROWS * root.pixel
    LayoutMirroring.enabled: false
    Repeater {
      id: letterRepeater
      model: root.wordmark
      delegate: Item {
        id: cell
        required property int index
        required property var modelData
        property real drop: 0
        property real tilt: 0
        property real heat: 0
        property real flash: 0
        readonly property color ink: Qt.tint(root.dim, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, heat))
        x: modelData.x * root.pixel
        width: modelData.w * root.pixel; height: letters.height
        opacity: 0
        function reset(shown) { fall.stop(); away.stop(); drop = 0; tilt = 0; heat = 0; flash = 0; opacity = shown ? 1 : 0 }
        function enter() { away.stop(); fall.restart() }
        function leave() { fall.stop(); away.restart() }
        Item {
          width: parent.width; height: parent.height
          y: cell.drop; rotation: cell.tilt
          Repeater {
            model: cell.modelData.runs
            delegate: Rectangle {
              required property var modelData
              x: modelData[0] * root.pixel; y: modelData[1] * root.pixel
              width: modelData[2] * root.pixel; height: root.pixel
              color: cell.ink
            }
          }
        }
        // The dust line where the letter hits.
        Rectangle {
          anchors.horizontalCenter: parent.horizontalCenter
          y: 17 * root.pixel
          height: root.pixel
          width: cell.width * (0.4 + cell.flash * 1.4)
          color: root.accent
          opacity: cell.flash > 0 ? 1 - cell.flash : 0
        }
        SequentialAnimation {
          id: fall
          PauseAnimation { duration: Math.round(cell.index * 95 * root.pace) }
          ScriptAction { script: { cell.drop = -Style.space(26); cell.tilt = (cell.index % 2 ? 1 : -1) * (14 + cell.index * 3); cell.heat = 1; cell.flash = 0 } }
          ParallelAnimation {
            NumberAnimation { target: cell; property: "drop"; to: 0; duration: Math.round(640 * root.pace); easing.type: Easing.OutBounce }
            NumberAnimation { target: cell; property: "opacity"; to: 1; duration: Math.round(160 * root.pace) }
            NumberAnimation { target: cell; property: "tilt"; to: 0; duration: Math.round(560 * root.pace); easing.type: Easing.OutBack; easing.overshoot: 2.4 }
            SequentialAnimation {
              PauseAnimation { duration: Math.round(230 * root.pace) }
              NumberAnimation { target: cell; property: "flash"; from: 0; to: 1; duration: Math.round(380 * root.pace); easing.type: Easing.OutCubic }
            }
          }
          NumberAnimation { target: cell; property: "heat"; to: 0; duration: Math.round(900 * root.pace); easing.type: Easing.InOutSine }
        }
        SequentialAnimation {
          id: away
          PauseAnimation { duration: Math.round(cell.index * 70 * root.pace) }
          ScriptAction { script: cell.heat = 0.6 }
          ParallelAnimation {
            NumberAnimation { target: cell; property: "drop"; to: Style.space(22); duration: Math.round(480 * root.pace); easing.type: Easing.InBack; easing.overshoot: 2.2 }
            NumberAnimation { target: cell; property: "tilt"; to: (cell.index % 2 ? -1 : 1) * (20 + cell.index * 4); duration: Math.round(480 * root.pace); easing.type: Easing.InQuad }
            NumberAnimation { target: cell; property: "opacity"; to: 0; duration: Math.round(480 * root.pace); easing.type: Easing.InQuad }
            NumberAnimation { target: cell; property: "heat"; to: 0; duration: Math.round(480 * root.pace) }
          }
        }
      }
    }
  }
}
