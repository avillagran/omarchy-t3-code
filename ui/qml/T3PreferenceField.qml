import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var field
 property bool writable:false
 property var edit
 signal changed(var value)
 signal staged(string value)
 readonly property string editText:typeof edit==="string"?edit:String(field.value || "")
 readonly property color fg:Color.popups.text
 spacing:Style.space(6)
 RowLayout {Layout.fillWidth:true;spacing:Style.space(12)
  Text{Layout.fillWidth:true;text:T3I18n.tr(root.field.label);color:root.fg;wrapMode:Text.Wrap;font.pixelSize:Style.font.body}
  Controls.CheckBox{objectName:"preference_"+root.field.key;visible:root.field.kind==="bool";enabled:root.writable;checked:root.field.value===true;onToggled:root.changed(checked)}
  T3Select{objectName:"preferenceChoice_"+root.field.key;visible:root.field.kind==="choice";enabled:root.writable;Layout.preferredWidth:Style.space(180);translateLabels:true;model:root.field.choices || [];textRole:"name";currentIndex:model.findIndex(function(c){return c.id===root.field.value});onActivated:function(i){root.changed(model[i].id)}}
  RowLayout {visible:root.field.kind==="days";spacing:Style.space(5)
   Controls.CheckBox{objectName:"preferenceEnabled_"+root.field.key;text:T3I18n.tr("Automatic");enabled:root.writable;checked:root.field.value!==null;onToggled:root.changed(checked?3:null)}
   Controls.SpinBox{objectName:"preferenceDays_"+root.field.key;enabled:root.writable&&root.field.value!==null;from:root.field.minimum || 1;to:root.field.maximum || 90;value:typeof root.field.value==="number"?root.field.value:3;editable:true;onValueModified:root.changed(value)}
  }
 }
 RowLayout {visible:root.field.kind==="text";Layout.fillWidth:true;spacing:Style.space(8)
  Controls.TextField {objectName:"preferenceText_"+root.field.key;visible:!root.field.multiline;Layout.fillWidth:true;enabled:root.writable;text:root.editText;maximumLength:root.field.maximumLength || 16384;color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;onTextEdited:root.staged(text)}
  Controls.ScrollView {id:instructionsScroll;visible:!!root.field.multiline;Layout.fillWidth:true;Layout.preferredHeight:Style.space(100);contentWidth:availableWidth
   T3Wheel{flickable:instructionsScroll.contentItem}
   Controls.TextArea {objectName:"preferenceInstructions_"+root.field.key;enabled:root.writable;text:root.editText;textFormat:TextEdit.PlainText;color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;wrapMode:TextEdit.Wrap;onTextChanged:if(activeFocus)root.staged(text)}
  }
  T3IconButton{objectName:"savePreference_"+root.field.key;name:"check";tooltipText:T3I18n.tr("Save");enabled:root.writable&&root.editText!==root.field.value&&root.editText.length<=(root.field.maximumLength || 16384);onClicked:root.changed(root.editText)}
 }
}
