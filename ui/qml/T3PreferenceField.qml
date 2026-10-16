import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Item {
 id:root
 required property var field
 property bool writable:false
 property var edit
 property bool divided:false
 property string controlName:""
 signal changed(var value)
 signal staged(string value)
 readonly property string editText:typeof edit==="string"?edit:String(field.value || "")
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.1)
 readonly property bool narrow:width<Style.space(560)
 implicitHeight:form.implicitHeight+Style.space(28)
 Rectangle{visible:root.divided;anchors.top:parent.top;anchors.left:parent.left;anchors.right:parent.right;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.065)}
 GridLayout {
  id:form;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(16);anchors.topMargin:Style.space(14)
  columns:root.narrow&&root.field.kind!=="bool"?1:2;columnSpacing:Style.space(24);rowSpacing:Style.space(10)
  Text{objectName:"preferenceLabel_"+root.field.key;Layout.fillWidth:true;Layout.alignment:Qt.AlignVCenter;text:T3I18n.tr(root.field.label);color:root.fg;wrapMode:Text.Wrap;font.pixelSize:Style.font.body;font.weight:Font.Medium;textFormat:Text.PlainText}
  RowLayout {visible:root.field.kind!=="text"||!root.field.multiline;Layout.alignment:Qt.AlignRight;Layout.fillWidth:form.columns===1;spacing:Style.space(10)
   Controls.TextField {id:line;objectName:"preferenceText_"+root.field.key;visible:root.field.kind==="text"&&!root.field.multiline;Layout.preferredWidth:Style.space(240);Layout.fillWidth:form.columns===1;implicitHeight:Style.space(34);enabled:root.writable;text:root.editText;maximumLength:root.field.maximumLength || 16384;color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;leftPadding:Style.space(10);rightPadding:Style.space(10);onTextEdited:root.staged(text)
    background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:line.activeFocus?Color.accent:root.muted}
   }
   Controls.Button{id:inlineSave;objectName:root.field.multiline?"":"savePreference_"+root.field.key;visible:root.field.kind==="text"&&!root.field.multiline&&root.editText!==root.field.value;text:T3I18n.tr("Save");enabled:root.writable&&root.editText!==root.field.value&&root.editText.length<=(root.field.maximumLength || 16384);implicitHeight:Style.space(28);implicitWidth:Style.space(64);padding:Style.space(6);onClicked:root.changed(root.editText)
    contentItem:Text{text:inlineSave.text;color:inlineSave.enabled?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.35);font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Style.space(5);color:inlineSave.hovered&&inlineSave.enabled?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):"transparent";border.width:1;border.color:inlineSave.activeFocus?Color.accent:root.muted}
   }
   T3Switch{objectName:root.controlName || "preference_"+root.field.key;visible:root.field.kind==="bool";enabled:root.writable;Accessible.name:T3I18n.tr(root.field.label);checked:root.field.value===true;onToggled:root.changed(checked)}
   T3Select{objectName:"preferenceChoice_"+root.field.key;visible:root.field.kind==="choice";enabled:root.writable;Layout.preferredWidth:Style.space(190);Layout.fillWidth:form.columns===1;popupAbove:false;translateLabels:true;model:root.field.choices || [];textRole:"name";currentIndex:model.findIndex(function(c){return c.id===root.field.value});onActivated:function(i){root.changed(model[i].id)}}
   RowLayout {visible:root.field.kind==="days";spacing:Style.space(12)
    T3Switch{objectName:"preferenceEnabled_"+root.field.key;Accessible.name:T3I18n.tr(root.field.label);enabled:root.writable;checked:root.field.value!==null;onToggled:root.changed(checked?3:null)}
    Controls.SpinBox{ id:days;objectName:"preferenceDays_"+root.field.key;implicitWidth:Style.space(100);implicitHeight:Style.space(30);enabled:root.writable&&root.field.value!==null;from:root.field.minimum || 1;to:root.field.maximum || 90;value:typeof root.field.value==="number"?root.field.value:3;editable:true;onValueModified:root.changed(value)
     background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.035);border.width:1;border.color:days.activeFocus?Color.accent:root.muted;opacity:days.enabled?1:0.4}
     contentItem:TextInput{text:days.textFromValue(days.value,days.locale);font.pixelSize:Style.font.caption;color:root.fg;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter;readOnly:!days.editable;validator:days.validator;inputMethodHints:Qt.ImhDigitsOnly;opacity:days.enabled?1:0.4}
     up.indicator:Item{x:days.width-width;width:Style.space(25);height:days.height;T3Icon{anchors.centerIn:parent;name:"plus";implicitWidth:Style.space(12);tint:root.fg}}
     down.indicator:Item{width:Style.space(25);height:days.height;T3Icon{anchors.centerIn:parent;name:"minus";implicitWidth:Style.space(12);tint:root.fg}}
    }
   }
  }
  ColumnLayout {visible:root.field.kind==="text"&&!!root.field.multiline;Layout.columnSpan:form.columns;Layout.fillWidth:true;spacing:Style.space(8)
   Controls.ScrollView {id:instructionsScroll;visible:!!root.field.multiline;Layout.fillWidth:true;Layout.preferredHeight:Style.space(88);contentWidth:availableWidth
    background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:memo.activeFocus?Color.accent:root.muted}
    T3Wheel{flickable:instructionsScroll.contentItem}
    Controls.TextArea {id:memo;objectName:"preferenceInstructions_"+root.field.key;enabled:root.writable;text:root.editText;textFormat:TextEdit.PlainText;color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;wrapMode:TextEdit.Wrap;padding:Style.space(10);background:null;onTextChanged:if(activeFocus)root.staged(text)}
   }
   Controls.Button{id:saveButton;objectName:root.field.multiline?"savePreference_"+root.field.key:"";Layout.alignment:Qt.AlignRight;visible:root.editText!==root.field.value;text:T3I18n.tr("Save");enabled:root.writable&&root.editText!==root.field.value&&root.editText.length<=(root.field.maximumLength || 16384);implicitHeight:Style.space(28);implicitWidth:Style.space(64);padding:Style.space(6);onClicked:root.changed(root.editText)
    contentItem:Text{text:saveButton.text;color:saveButton.enabled?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.35);font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Style.space(5);color:saveButton.hovered&&saveButton.enabled?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):"transparent";border.width:1;border.color:saveButton.activeFocus?Color.accent:root.muted}
   }
  }
 }
}
