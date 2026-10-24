import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Item {
 id:root
 objectName:"preferenceField_"+field.key
 property alias optionsPopup:modelOptions
 required property var field
 property bool writable:false
 property var edit
 property bool divided:false
 property string controlName:""
 property var backend:null
 property string environment:""
 property var modelCatalog:({})
 signal inheritRequested()
 readonly property bool resettable:!field.projectScoped&&typeof field.resetValue!=="undefined"&&JSON.stringify(field.value)!==JSON.stringify(field.resetValue)
 readonly property var selectedModel:(modelCatalog.models || []).find(function(m){return root.field.value&&m.instanceId===root.field.value.instanceId&&m.model===root.field.value.model}) || ({})
 QtObject {
  id:modelBridge
  property var catalog:root.modelCatalog
  property string selectedEnvironment:root.environment
  property var favoritePending:root.backend?root.backend.favoritePending || ({}):({})
  property string favoriteError:root.backend?root.backend.favoriteError || "":""
  function refreshCatalog(){if(root.backend&&typeof root.backend.refreshCatalogIn==="function")root.backend.refreshCatalogIn(root.environment)}
  function chooseModel(index){var m=(catalog.models || [])[index];if(m&&m.available!==false)root.changed({instanceId:m.instanceId,model:m.model,options:[]})}
  function toggleModelFavorite(instance,model){if(root.backend&&typeof root.backend.toggleModelFavoriteIn==="function")root.backend.toggleModelFavoriteIn(root.environment,instance,model)}
  function chooseOption(id,value){if(!root.field.value)return;var selection=Object.assign({},root.field.value),options=(selection.options || []).filter(function(o){return o.id!==id});options.push({id:id,value:value});selection.options=options;root.changed(selection)}
 }

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
  ColumnLayout{Layout.fillWidth:true;Layout.alignment:Qt.AlignVCenter;spacing:Style.space(4)
   Text{objectName:"preferenceLabel_"+root.field.key;Layout.fillWidth:true;text:T3I18n.tr(root.field.label);color:root.fg;wrapMode:Text.Wrap;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
   RowLayout{visible:!!root.field.projectScoped||!!root.field.mixed;Layout.fillWidth:true
    Text{objectName:"preferenceSource_"+root.field.key;text:root.field.mixed?T3I18n.tr("Mixed values"):root.field.source==="project"?T3I18n.tr("Project override"):root.field.source==="t3.json"?"t3.json":root.field.source==="mixed"?T3I18n.tr("Mixed sources"):T3I18n.tr("Inherited from environment");color:Color.accent;font.pixelSize:Style.font.caption;Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    T3IconButton{objectName:"inheritPreference_"+root.field.key;visible:!!root.field.projectScoped&&!!root.field.overridden;enabled:root.writable;name:"refresh-cw";tooltipText:T3I18n.tr("Use inherited value");onClicked:root.inheritRequested()}
   }
   Text{objectName:"preferenceDescription_"+root.field.key;Layout.fillWidth:true;visible:!!root.field.description;text:T3I18n.tr(root.field.description || "");color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.58);wrapMode:Text.Wrap;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  }
  RowLayout {visible:root.field.kind!=="text"||!root.field.multiline;Layout.alignment:Qt.AlignRight;Layout.fillWidth:form.columns===1;spacing:Style.space(10)
   T3IconButton{objectName:"resetPreference_"+root.field.key;visible:root.resettable;enabled:root.writable;name:"refresh-cw";tooltipText:T3I18n.tr("Reset");onClicked:root.changed(root.field.resetValue)}
   T3ModelPicker{objectName:"preferenceModel_"+root.field.key;visible:root.field.kind==="model";enabled:root.writable&&(root.modelCatalog.models || []).some(function(m){return m.available!==false});Layout.preferredWidth:Style.space(220);Layout.fillWidth:form.columns===1;bridge:modelBridge;selection:root.field.mixed?({}):root.field.value || ({});emptyText:T3I18n.tr(root.field.mixed?"Mixed values":"Automatic")}
   T3IconButton{objectName:"automaticPreference_"+root.field.key;visible:root.field.kind==="model"&&!!root.field.projectScoped&&root.field.key!=="textGenerationModelSelection"&&(!!root.field.value||!!root.field.mixed);enabled:root.writable;name:"x";tooltipText:T3I18n.tr("Automatic");onClicked:root.changed(null)}
   T3IconButton{objectName:"preferenceModelOptions_"+root.field.key;visible:root.field.kind==="model"&&!!root.field.value&&(root.selectedModel.descriptors || []).length>0;enabled:root.writable;name:"sliders-horizontal";tooltipText:T3I18n.tr("Model options");onClicked:modelOptions.open()}

   Controls.TextField {id:line;objectName:"preferenceText_"+root.field.key;visible:root.field.kind==="text"&&!root.field.multiline;Layout.preferredWidth:Style.space(220);Layout.fillWidth:form.columns===1;implicitHeight:Style.space(34);enabled:root.writable;text:root.editText;maximumLength:root.field.maximumLength || 16384;color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;leftPadding:Style.space(10);rightPadding:Style.space(10);onTextEdited:root.staged(text);onEditingFinished:if(root.field.commitOnBlur&&root.writable&&text!==String(root.field.value||""))root.changed(text);placeholderText:T3I18n.tr(root.field.mixed?"Mixed":root.field.placeholder||"")
    background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:line.activeFocus?Color.accent:root.muted}
   }
   Controls.Button{id:inlineSave;objectName:root.field.multiline?"":"savePreference_"+root.field.key;visible:root.field.kind==="text"&&!root.field.multiline&&!root.field.commitOnBlur&&root.editText!==root.field.value;text:T3I18n.tr("Save");enabled:root.writable&&root.editText!==root.field.value&&root.editText.length<=(root.field.maximumLength || 16384);implicitHeight:Style.space(28);implicitWidth:Style.space(64);padding:Style.space(6);onClicked:root.changed(root.editText)
    contentItem:Text{text:inlineSave.text;color:inlineSave.enabled?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.35);font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Style.space(5);color:inlineSave.hovered&&inlineSave.enabled?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):"transparent";border.width:1;border.color:inlineSave.activeFocus?Color.accent:root.muted}
   }
   T3Switch{objectName:root.controlName || "preference_"+root.field.key;visible:root.field.kind==="bool";enabled:root.writable;Accessible.name:T3I18n.tr(root.field.label);checked:root.field.value===true;onToggled:root.changed(checked)}
   T3Select{objectName:"preferenceChoice_"+root.field.key;visible:root.field.kind==="choice";enabled:root.writable;Layout.preferredWidth:Style.space(170);Layout.fillWidth:form.columns===1;popupAbove:false;translateLabels:true;model:root.field.choices || [];textRole:"name";currentIndex:root.field.mixed?-1:model.findIndex(function(c){return c.id===root.field.value});displayText:root.field.mixed?T3I18n.tr("Mixed values"):currentText;onActivated:function(i){root.changed(model[i].id)}}
   RowLayout {visible:root.field.kind==="days";spacing:Style.space(12);layoutDirection:root.field.retention?Qt.RightToLeft:Qt.LeftToRight
    T3Switch{objectName:"preferenceEnabled_"+root.field.key;Accessible.name:T3I18n.tr(root.field.label);enabled:root.writable;checked:root.field.value!==null;onToggled:root.changed(checked?(root.field.enabledValue || 3):null)}
    Text{visible:!!root.field.retention;text:T3I18n.tr(root.field.value===null?"Off":"days");color:root.fg;font.pixelSize:Style.font.caption}
    Controls.SpinBox{ id:days;visible:!root.field.retention||root.field.value!==null;objectName:"preferenceDays_"+root.field.key;implicitWidth:Style.space(100);implicitHeight:Style.space(30);enabled:root.writable&&root.field.value!==null;from:root.field.minimum || 1;to:root.field.maximum || 90;value:typeof root.field.value==="number"?root.field.value:(root.field.enabledValue || 3);editable:true;onValueModified:root.changed(value)
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
    Controls.TextArea {id:memo;objectName:"preferenceInstructions_"+root.field.key;enabled:root.writable;text:root.editText;textFormat:TextEdit.PlainText;placeholderText:T3I18n.tr(root.field.placeholder||"");color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;wrapMode:TextEdit.Wrap;padding:Style.space(10);background:null;onTextChanged:if(activeFocus)root.staged(text)}
   }
   Controls.Button{id:saveButton;objectName:root.field.multiline?"savePreference_"+root.field.key:"";Layout.alignment:Qt.AlignRight;visible:root.editText!==root.field.value;text:T3I18n.tr("Save");enabled:root.writable&&root.editText!==root.field.value&&root.editText.length<=(root.field.maximumLength || 16384);implicitHeight:Style.space(28);implicitWidth:Style.space(64);padding:Style.space(6);onClicked:root.changed(root.editText)
    contentItem:Text{text:saveButton.text;color:saveButton.enabled?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.35);font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Style.space(5);color:saveButton.hovered&&saveButton.enabled?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):"transparent";border.width:1;border.color:saveButton.activeFocus?Color.accent:root.muted}
   }
  }
 }
 Controls.Popup{id:modelOptions;objectName:"preferenceModelOptionsPopup_"+root.field.key;parent:Controls.Overlay.overlay;anchors.centerIn:parent;modal:true;width:Math.min(Style.space(420),parent?parent.width-Style.space(32):Style.space(420));padding:Style.space(16)
  background:Rectangle{color:Color.popups.background;radius:Style.space(9);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(12)
   RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr("Model options");color:root.fg;font.pixelSize:Style.font.body}T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:modelOptions.close()}}
   T3ModelOptions{Layout.fillWidth:true;bridge:modelBridge;descriptors:root.selectedModel.descriptors || [];selection:root.field.value || ({});locked:!root.writable;namePrefix:"preferenceOption_"+root.field.key+"_"}
  }
 }
}
