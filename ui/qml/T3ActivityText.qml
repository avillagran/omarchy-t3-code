import QtQuick 6.11
import qs.Commons

Item {
  id: root
  property string text: ""
  property bool active: false
  property color tint: Color.popups.text
  property int fontSize: Style.font.caption
  T3Tick { id: tick; running: root.animating }
  // Overridable for still captures.
  property real progress: (tick.time / 2.2) % 1
  readonly property bool animating: active && visible && width > 0
  implicitWidth: base.implicitWidth
  implicitHeight: base.implicitHeight
  clip: true
  Text { id: base; anchors.fill: parent; text: root.text; textFormat: Text.PlainText; color: root.tint; opacity: 0.50; font.pixelSize: root.fontSize; elide: Text.ElideRight }
  // Same 72px focus band and 2.2s cycle as T3's live-activity-focus CSS,
  // swept with a soft falloff on the shared T3Ticker clock. Only the small
  // cached glyph texture is shaded.
  Loader {
    active: root.animating
    width: Math.min(root.width,base.implicitWidth); height: root.height
    sourceComponent: Component {
      Item {
        Text { id: ink; anchors.fill:parent; text:root.text;textFormat:Text.PlainText;color:root.tint;font.pixelSize:root.fontSize;elide:Text.ElideRight }
        ShaderEffectSource { id: glyphs; sourceItem:ink;hideSource:true;visible:false }
        ShaderEffect {
          anchors.fill:parent
          property variant source:glyphs
          property real progress:root.progress
          property real textWidth:width
          property real bandWidth:Style.space(72)
          fragmentShader:Qt.resolvedUrl("../assets/shaders/activity.frag.qsb")
        }
      }
    }
  }
}
