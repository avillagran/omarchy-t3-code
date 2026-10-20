import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 property var descriptors:[]
 property var selection:({})
 property bool locked:false
 property string namePrefix:"t3Option_"
 readonly property color dim:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55)
 function optionValue(descriptor){var option=(selection.options || []).find(function(o){return o.id===descriptor.id});return option?option.value:descriptor.currentValue}
 function effortDescriptor(){return descriptors.find(function(d){return d.type==="select"&&/effort|reasoning|thinking/i.test(d.id+" "+d.label)})}
 // Steps the reasoning effort one option up or down, staying at the ends.
 function stepEffort(direction){var d=effortDescriptor();if(locked||!d)return false;var options=d.options||[];var current=options.findIndex(function(o){return o.id===optionValue(d)});var next=Math.max(0,Math.min(options.length-1,(current<0?0:current)+direction));if(next===current||!options[next])return false;bridge.chooseOption(d.id,options[next].id);return true}
 function focusEffort(){for(var i=0;i<rows.count;i++){var row=rows.itemAt(i);if(row&&/effort|reasoning|thinking/i.test(row.modelData.id+" "+row.modelData.label)){row.focusControl();return true}}return false}
 spacing:Style.space(6)
 Repeater{id:rows;model:root.descriptors
  RowLayout{required property var modelData;Layout.fillWidth:true
   function focusControl(){if(choice.visible){choice.forceActiveFocus();choice.popup.open()}else toggle.forceActiveFocus()}
   Text{Layout.preferredWidth:Style.space(125);text:T3I18n.tr(modelData.label);font.pixelSize:Style.font.caption;color:root.dim;textFormat:Text.PlainText;wrapMode:Text.Wrap}
   T3Select{id:choice;objectName:modelData.type==="select"?root.namePrefix+modelData.id:"";Layout.fillWidth:true;visible:modelData.type==="select";enabled:!root.locked;model:modelData.options || [];textRole:"label";translateLabels:true;currentIndex:model.findIndex(function(o){return o.id===root.optionValue(modelData)});onActivated:function(index){root.bridge.chooseOption(modelData.id,model[index].id)}}
   T3Switch{id:toggle;objectName:modelData.type==="boolean"?root.namePrefix+modelData.id:"";visible:modelData.type==="boolean";enabled:!root.locked;checked:!!root.optionValue(modelData);Accessible.name:T3I18n.tr(modelData.label);onToggled:root.bridge.chooseOption(modelData.id,checked)}
  }
 }
}
