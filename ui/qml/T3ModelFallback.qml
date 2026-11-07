import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "../js/UsageLimits.js" as Usage
ColumnLayout {
 id:root
 required property var bridge
 readonly property var notice:bridge.modelFallbackNotice
 visible:!!notice
 spacing:Style.space(5)
 RowLayout {
  Layout.fillWidth:true
  Text{Layout.fillWidth:true;text:root.notice?T3I18n.tr("Using {model}",{model:root.bridge.fallbackModelName(root.notice.selection)}):"";color:Color.popups.text;font.pixelSize:Style.font.caption;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  T3IconButton{objectName:"dismissModelFallback";name:"x";tooltipText:T3I18n.tr("Dismiss fallback model notice");onClicked:root.bridge.dismissModelFallback()}
 }
 Text{Layout.fillWidth:true;text:root.notice?T3I18n.tr("{provider} is out of usage",{provider:root.notice.provider.displayName || Usage.label(root.notice.provider.driver)})+(root.notice.resetsAt?" · "+root.bridge.formatTimestamp(root.notice.resetsAt):""):"";color:Color.popups.text;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;wrapMode:Text.Wrap}
 Button{objectName:"useFallbackDefault";text:root.notice?T3I18n.tr("Use {model}",{model:root.bridge.fallbackModelName(root.notice.defaultSelection)}):"";enabled:!root.bridge.draft.busy&&!root.bridge.draft.attempt;onClicked:root.bridge.useFallbackDefault()}
}
