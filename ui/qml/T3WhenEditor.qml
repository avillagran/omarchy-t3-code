import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeKeybindings.js" as Native
ColumnLayout {
 id:root
 property string expression:""
 property var lastValidNode:null
 readonly property var parsed:Native.nativeKeys.parseWhenExpressionDraft(expression)
 readonly property bool valid:parsed.ok&&expression.length<=256
 readonly property var node:lastValidNode
 readonly property var unknown:parsed.ok?Native.nativeKeys.unknownWhenVariables(parsed.value):[]
 signal edited(string value)
 spacing:Style.space(10)
 function rememberValid(){var p=Native.nativeKeys.parseWhenExpressionDraft(expression);if(p.ok)lastValidNode=p.value}
 onExpressionChanged:rememberValid()
 Component.onCompleted:rememberValid()
 function update(value){expression=Native.nativeKeys.whenAstToExpression(value);edited(expression)}
 RowLayout{Layout.fillWidth:true
  Text{Layout.fillWidth:true;text:T3I18n.tr("When");color:Color.popups.text;font.pixelSize:Style.font.body}
  T3PickerButton{objectName:"whenRootCondition";primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Condition");onClicked:root.update(root.node?{type:"and",left:root.node,right:Native.nativeKeys.defaultWhenCondition()}:Native.nativeKeys.defaultWhenCondition())}
  T3PickerButton{objectName:"whenRootGroup";primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Group");onClicked:root.update(root.node?{type:"and",left:root.node,right:Native.nativeKeys.defaultWhenGroup("or")}:Native.nativeKeys.defaultWhenGroup("or"))}
 }
 Loader{Layout.fillWidth:true;active:!!root.node;visible:active;sourceComponent:Component{T3WhenNode{node:root.node;removable:true;onChanged:function(value){root.update(value)}
onRemoved:root.update(undefined)}}}
 Controls.TextField{objectName:"whenExpressionInput";Layout.fillWidth:true;text:root.expression;selectByMouse:true;maximumLength:256;color:Color.popups.text;placeholderText:"composerFocus && !terminalFocus";placeholderTextColor:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.6);font.family:"monospace";font.pixelSize:Style.font.caption;onTextEdited:{root.expression=text;root.edited(text)}
background:Rectangle{color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.04);radius:Style.space(5);border.width:1;border.color:root.valid?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.12):Color.urgent}}
 Text{visible:!root.valid;Layout.fillWidth:true;text:T3I18n.tr("Use variables with {operators} and parentheses.",{operators:"!, &&, ||"});color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{visible:root.unknown.length>0;Layout.fillWidth:true;text:T3I18n.tr("Unknown condition variables: {variables}",{variables:root.unknown.join(", ")});color:Color.accent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
}
