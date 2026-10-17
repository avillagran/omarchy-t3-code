import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeKeybindings.js" as Native
import "../js/QtKeybindings.js" as KeysAdapter
import "../js/KeybindingLabels.js" as Labels
Item {
 id:root
 objectName:isNew?"keybindingNewRow":"keybindingRow_"+row.command
 required property var row
 required property var allRows
 property bool isNew:false
 property bool writable:false
 property bool saving:false
 property bool divided:false
 property string keyDraft:""
 property string whenDraft:""
 property bool recording:false
 readonly property bool captureActive:recording||captureInput.activeFocus
 signal saveRequested(var input)
 signal removeRequested(var target)
 signal resetRequested(var input)
 signal captureChanged(bool active)
 readonly property var k:Native.nativeKeys
 readonly property var parsed:k.parseWhenExpressionDraft(whenDraft)
 readonly property string canonicalWhen:parsed.ok?k.whenAstToExpression(parsed.value):whenDraft
 readonly property bool dirty:isNew?!!keyDraft:keyDraft!==row.key||canonicalWhen!==row.when
 readonly property bool valid:!!keyDraft.trim()&&keyDraft.length<=64&&!!k.parseKeybindingShortcut(keyDraft)&&parsed.ok&&whenDraft.length<=256
 readonly property var conflicts:k.keybindingConflictLabels(allRows,{rowId:row.id,key:keyDraft,when:canonicalWhen})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 implicitHeight:Math.max(Style.space(68),labels.implicitHeight+Style.space(28))
 onRowChanged:resetDraft()
 onCaptureActiveChanged:captureChanged(captureActive)
 Component.onCompleted:resetDraft()
 Component.onDestruction:if(captureActive)captureChanged(false)
 function resetDraft(){if(!row)return;keyDraft=row.key;whenDraft=row.when;recording=false}
 function target(){var t={command:row.command,key:row.key};if(row.when)t.when=row.when;return t}
 function commit(){if(!writable||saving||!dirty||!valid)return;var p={command:row.command,key:keyDraft.trim()};if(!isNew)p.replace=target();if(canonicalWhen)p.when=canonicalWhen;saveRequested(p)}
 function reset(){if(!writable||saving||row.source!=="Custom"||!row.defaultKey)return;var p={command:row.command,key:row.defaultKey,replace:target()};if(row.defaultWhen)p.when=row.defaultWhen;resetRequested(p)}
 function capture(event){
  if(event.key===Qt.Key_Tab&&!recording){event.accepted=false;return}
  event.accepted=true
  if(event.key===Qt.Key_Escape){keyDraft=row.key;recording=false;return}
  var value=KeysAdapter.capture(event);if(value){keyDraft=value;recording=false}
 }
 Rectangle{visible:root.divided;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)}
 RowLayout{anchors.fill:parent;anchors.margins:Style.space(14);spacing:Style.space(10)
  ColumnLayout{id:labels;Layout.fillWidth:true;spacing:Style.space(4)
   RowLayout{Layout.fillWidth:true;spacing:Style.space(7)
    Text{Layout.fillWidth:true;text:Labels.command(root.row.command,function(key){return T3I18n.tr(key)});color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{visible:!root.isNew&&root.row.source!=="Default";text:T3I18n.tr(root.row.source);color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   }
   Controls.AbstractButton{id:whenButton;objectName:"keybindingWhen_"+root.row.command;enabled:root.writable&&!root.saving;implicitWidth:Math.min(whenContent.implicitWidth,labels.width);implicitHeight:Style.space(22);hoverEnabled:true;onClicked:conditionPopup.open()
    contentItem:RowLayout{id:whenContent;spacing:Style.space(6)
     Text{text:T3I18n.tr("When");color:root.dim;font.pixelSize:Style.font.caption;opacity:0.7}
     Text{Layout.fillWidth:true;text:root.whenDraft || T3I18n.tr("Always");color:whenButton.hovered?root.fg:root.dim;font.pixelSize:Style.font.caption;font.family:"monospace";elide:Text.ElideRight;textFormat:Text.PlainText}
     T3Icon{name:"chevron-down";implicitWidth:Style.space(10);tint:root.dim}
    }
   }
  }
  T3IconButton{visible:root.conflicts.length>0;name:"triangle-alert";tint:Color.accent;tooltipText:root.conflicts.map(function(label){return Labels.text(label,function(key){return T3I18n.tr(key)})}).join(", ")}
  T3PickerButton{objectName:"keybindingSave_"+root.row.command;visible:root.dirty;primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Save");enabled:root.writable&&!root.saving&&root.valid;onClicked:root.commit()}
  T3PickerButton{objectName:"keybindingActions_"+root.row.command;visible:!root.isNew&&root.row.source!=="Default";text:"⋯";primary:false;implicitHeight:Style.space(28);leftPadding:Style.space(6);rightPadding:leftPadding;enabled:root.writable&&!root.saving;onClicked:actions.popup();Accessible.name:T3I18n.tr("Keyboard shortcuts")}
  Controls.AbstractButton{objectName:"keybindingPill_"+root.row.command;visible:!root.recording&&root.keyDraft===root.row.key;enabled:root.writable&&!root.saving;implicitWidth:pills.implicitWidth+Style.space(12);implicitHeight:Style.space(30);hoverEnabled:true;onClicked:{root.recording=true;Qt.callLater(function(){captureInput.forceActiveFocus()})}
   background:Rectangle{radius:Style.space(5);color:parent.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent";border.width:parent.activeFocus?1:0;border.color:Color.accent}
   contentItem:RowLayout{id:pills;spacing:Style.space(3)
    Repeater{model:root.row.key.split("+")
     Rectangle{required property string modelData;implicitWidth:pillText.implicitWidth+Style.space(9);implicitHeight:Style.space(23);radius:Style.space(4);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)
      Text{id:pillText;anchors.centerIn:parent;text:parent.modelData==="mod"?"Ctrl":parent.modelData==="ctrl"?"⌃":parent.modelData==="shift"?"⇧":parent.modelData==="alt"?"Alt":parent.modelData.length===1?parent.modelData.toUpperCase():parent.modelData;color:root.fg;font.pixelSize:Style.font.caption;font.family:"monospace"}
     }
    }
   }
  }
  Controls.TextField{id:captureInput;objectName:"keybindingCapture_"+root.row.command;visible:root.recording||root.keyDraft!==root.row.key;Layout.preferredWidth:Style.space(176);implicitHeight:Style.space(30);enabled:root.writable&&!root.saving;readOnly:true;text:root.recording?"":root.keyDraft;placeholderText:T3I18n.tr("Press shortcut");placeholderTextColor:root.dim;color:root.fg;font.family:"monospace";font.pixelSize:Style.font.caption
   onActiveFocusChanged:root.recording=activeFocus
   Keys.priority:Keys.BeforeItem
   Keys.onPressed:function(event){root.capture(event)}
   background:Rectangle{radius:Style.space(5);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04);border.width:1;border.color:parent.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12)}
  }
 }
 Controls.Menu{id:actions;palette.window:Color.popups.background;palette.text:root.fg
  background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:root.dim}
  Controls.MenuItem{objectName:"keybindingReset_"+root.row.command;visible:root.row.source==="Custom"&&!!root.row.defaultKey;height:visible?implicitHeight:0;text:T3I18n.tr("Reset to default");enabled:root.writable&&!root.saving;onTriggered:root.reset()}
  Controls.MenuItem{objectName:"keybindingRemove_"+root.row.command;text:T3I18n.tr("Remove");enabled:root.writable&&!root.saving;palette.text:Color.urgent;onTriggered:root.removeRequested(root.target())}
 }
 Controls.Popup{id:conditionPopup;objectName:"keybindingConditionPopup";parent:root;x:0;y:root.height;width:Math.min(Style.space(540),root.width);padding:Style.space(14);focus:true
  closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  background:Rectangle{color:Color.popups.background;radius:Style.space(8);border.width:1;border.color:root.dim}
  contentItem:Loader{active:conditionPopup.visible;sourceComponent:Component{T3WhenEditor{expression:root.whenDraft;onEdited:function(value){root.whenDraft=value}}}}
 }
}
