import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeKeybindings.js" as Native
Rectangle {
 id:root
 property var node:null
 property int depth:0
 property bool removable:false
 signal changed(var value)
 signal removed()
 readonly property var k:Native.nativeKeys
 readonly property var condition:node?k.conditionParts(node):null
 readonly property bool negatedGroup:!!node&&!condition&&node.type==="not"
 readonly property string operator:node&&node.type==="or"?"or":"and"
 // Derive membership directly from node: intermediate bound flags can be stale
 // during synchronous Loader creation and accidentally instantiate another leaf.
 readonly property var childrenNodes:!node||k.conditionParts(node)||node.type==="not"?[]:k.flattenWhenChildren(node,node.type==="or"?"or":"and")
 readonly property color fg:Color.popups.text
 implicitHeight:body.implicitHeight+Style.space(16)
 color:Qt.rgba(fg.r,fg.g,fg.b,0.025);radius:Style.space(7);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.12)
 function replaceChild(index,value){var copy=childrenNodes.slice();copy[index]=value;changed(k.buildWhenExpressionGroup(copy,operator))}
 function removeChild(index){var copy=childrenNodes.slice();copy.splice(index,1);changed(k.buildWhenExpressionGroup(copy,operator)||k.defaultWhenCondition())}
 function add(group){var next=childrenNodes.slice();next.push(group?k.defaultWhenGroup(operator==="and"?"or":"and"):k.defaultWhenCondition());changed(k.buildWhenExpressionGroup(next,operator))}
 onNodeChanged:if(notLoader&&notLoader.item&&node&&node.type==="not"&&!k.conditionParts(node))notLoader.item.node=node.node
 ColumnLayout{id:body;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(8);spacing:Style.space(8)
  RowLayout{Layout.fillWidth:true;spacing:Style.space(6)
   Controls.Button{id:negation;objectName:"whenNot_"+root.depth;visible:!!root.condition||root.negatedGroup;text:T3I18n.tr("Not");checkable:true;checked:root.condition?root.condition.negated:true;implicitHeight:Style.space(28);implicitWidth:Style.space(44);onClicked:root.changed(root.condition?root.k.setConditionNegated(root.node,checked):root.node.node)
    contentItem:Text{text:negation.text;color:root.fg;font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Style.space(5);color:negation.checked?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.09):"transparent";border.width:1;border.color:negation.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
   }
   T3Select{objectName:"whenVariable_"+root.depth;visible:!!root.condition;Layout.fillWidth:true;model:{var options=root.k.buildWhenVariableOptions();if(root.condition&&options.indexOf(root.condition.identifier)<0)options.unshift(root.condition.identifier);return options}
currentIndex:root.condition?model.indexOf(root.condition.identifier):-1;popupAbove:false;onActivated:function(i){root.changed(root.k.setConditionIdentifier(root.node,model[i]))}}
   T3Select{objectName:"whenOperator_"+root.depth;visible:!root.condition&&!root.negatedGroup;Layout.preferredWidth:Style.space(85);model:[{id:"and",name:T3I18n.tr("and")},{id:"or",name:T3I18n.tr("or")}];textRole:"name";currentIndex:root.operator==="or"?1:0;popupAbove:false;onActivated:function(i){root.changed(root.k.buildWhenExpressionGroup(root.childrenNodes,model[i].id))}}
   T3PickerButton{objectName:"whenAddCondition_"+root.depth;visible:!root.condition&&!root.negatedGroup;primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Condition");onClicked:root.add(false)}
   T3PickerButton{objectName:"whenAddGroup_"+root.depth;visible:!root.condition&&!root.negatedGroup;primary:false;implicitHeight:Style.space(28);text:T3I18n.tr("Group");onClicked:root.add(true)}
   Item{visible:!root.condition;Layout.fillWidth:true}
   T3IconButton{visible:root.removable;name:"minus";tooltipText:root.node?T3I18n.tr(root.k.whenNodeRemoveLabel(root.node,root.depth)):"";onClicked:root.removed()}
  }
  Loader{id:notLoader;Layout.fillWidth:true;Layout.leftMargin:Style.space(16);active:root.negatedGroup&&root.depth<64;visible:active;source:active?Qt.resolvedUrl("T3WhenNode.qml"):""
   onLoaded:{item.depth=root.depth+1;item.node=root.node?root.node.node:null;item.changed.connect(function(value){root.changed({type:"not",node:value})})}
  }
  Repeater{model:root.depth<64?root.childrenNodes:[]
   Loader{id:childLoader;required property var modelData;required property int index;Layout.fillWidth:true;Layout.leftMargin:Style.space(16);source:Qt.resolvedUrl("T3WhenNode.qml")
    onModelDataChanged:if(item)item.node=modelData
    onLoaded:{item.depth=root.depth+1;item.node=childLoader.modelData;item.removable=true;item.changed.connect(function(value){root.replaceChild(childLoader.index,value)});item.removed.connect(function(){root.removeChild(childLoader.index)})}
   }
  }
 }
}
