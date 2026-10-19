import QtQuick 6.11
import qs.Commons

Item {
  id: root
  property string text: ""
  property bool active: false
  property color tint: Color.popups.text
  property int fontSize: Style.font.caption
  property real progress: 0
  readonly property bool animating: active && visible && width > 0
  implicitWidth: base.implicitWidth
  implicitHeight: base.implicitHeight
  clip: true
  Text { id: base; anchors.fill: parent; text: root.text; textFormat: Text.PlainText; color: root.tint; opacity: 0.50; font.pixelSize: root.fontSize; elide: Text.ElideRight }
  // Same 72px focus band and 2.2s cycle as T3's live-activity-focus CSS, but
  // swept continuously with a soft falloff instead of 30 hard steps. Only
  // the small cached glyph texture is shaded.
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
  NumberAnimation on progress { from: 0; to: 1; duration: 2200; loops: Animation.Infinite; running: root.animating }
}
