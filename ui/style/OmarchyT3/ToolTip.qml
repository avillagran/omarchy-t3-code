import QtQuick
import QtQuick.Window
import QtQuick.Templates as T
import qs.Commons as Commons
import qs.Ui

// ot3's Qt Quick Controls style is Fusion with Omarchy's unified tooltip
// (omarchy PR #15694): the theme's tooltip palette, border and corners, plain
// wrapped text, and the shape/border overrides of tooltips.json. bin/ot3
// selects it with QT_QUICK_CONTROLS_STYLE, so every Controls.ToolTip in ot3
// uses it without changes.
T.ToolTip {
  id: root
  readonly property var themeBorderSpec: Commons.Border.surfaceSpec("tooltip", "border", Commons.Color.tooltip.border, Commons.Style.normalBorderWidth)
  readonly property real maximumWidth: Commons.Style.space(360)

  x: Math.round(((parent ? Number(parent.width) || 0 : 0) - width) / 2)
  y: -height - margins
  margins: Commons.Style.spacing.sm
  padding: 0
  delay: 400
  timeout: -1
  focus: false
  closePolicy: T.Popup.CloseOnEscape | T.Popup.CloseOnPressOutsideParent
  readonly property real windowWidth: parent ? Number(parent.Window.width) || 0 : 0
  implicitWidth: Math.min(implicitContentWidth, windowWidth > 0 ? Math.max(1, windowWidth - margins * 2) : maximumWidth)
  implicitHeight: implicitContentHeight

  contentItem: BorderSurface {
    id: surface
    color: Commons.Color.tooltip.background
    borderSpec: ToolTipPreferences.borderSpec(root.themeBorderSpec)
    radius: ToolTipPreferences.radius(Commons.Style.cornerRadius)
    leftPadding: Commons.Style.spacing.controlPaddingX
    rightPadding: Commons.Style.spacing.controlPaddingX
    topPadding: Commons.Style.spacing.controlPaddingY
    bottomPadding: Commons.Style.spacing.controlPaddingY
    implicitWidth: Math.min(root.maximumWidth, label.implicitWidth + contentLeftInset + contentRightInset)
    implicitHeight: label.implicitHeight + contentTopInset + contentBottomInset
    Text {
      id: label
      x: surface.contentLeftInset
      y: surface.contentTopInset
      width: Math.max(1, surface.width - surface.contentLeftInset - surface.contentRightInset)
      textFormat: Text.PlainText
      text: root.text
      color: Commons.Color.tooltip.text
      font.family: Commons.Style.font.family
      font.pixelSize: Commons.Style.font.bodySmall
      wrapMode: Text.Wrap
    }
  }
  enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120 } }
  exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 80 } }
}
