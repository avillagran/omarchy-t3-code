import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/BaseRefChoices.js" as NativeChoices

// QML presentation of native DiffPanel's base-ref combobox. Pairing/filtering is upstream.
Item {
 id:root;objectName:"t3BaseRefPicker";implicitHeight:Style.space(30);implicitWidth:Style.space(210)
 required property var view
 property string selectedRef:""
 property string resolvedRef:""
 property string headRef:""
 readonly property var choices:NativeChoices.filterBaseRefChoices(NativeChoices.buildBaseRefChoices((view.local.refs || []).filter(function(r){return r.name!==root.headRef}),view.remote.refs || []),search.text)
 onVisibleChanged:if(!visible)popup.close()
 signal requested(string query,string kind,var cursor)
 signal chosen(string value)
 signal dismissed()
 function choose(value){chosen(value);popup.close()}
 function valueFor(choice){return choice.remote&&choice.remote.name===selectedRef?selectedRef:choice.local?choice.local.name:choice.remote?choice.remote.name:choice.id}
 function open(){search.text="";popup.open();requested("","",null);search.forceActiveFocus()}
 T3PickerButton {objectName:"t3BaseRefTrigger";anchors.fill:parent;primary:false;text:root.resolvedRef || root.selectedRef || T3I18n.tr("Automatic");onClicked:root.open();contentItem:RowLayout{spacing:Style.space(6);Text{Layout.fillWidth:true;text:parent.parent.text;color:Color.popups.text;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}T3Icon{name:"chevron-down";implicitWidth:Style.space(12);tint:Color.popups.text}}}
 Timer{id:debounce;interval:180;onTriggered:if(popup.opened)root.requested(search.text,"",null)}
 Controls.Popup {
  id:popup;objectName:"t3BaseRefPopup";parent:root;x:Math.min(0,root.width-width);y:root.height+Style.space(4);width:Math.min(Style.space(360),Math.max(root.width,Style.space(290)));height:Math.min(Style.space(430),content.implicitHeight+padding*2);padding:Style.space(8);focus:true;closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutsideParent
  background:Rectangle{radius:Style.space(8);color:Color.popups.background;border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.14)}
  onClosed:{debounce.stop();root.dismissed();search.text=""}
  ColumnLayout {
   id:content;anchors.fill:parent;spacing:Style.space(5)
   Controls.TextField {id:search;objectName:"t3BaseRefSearch";Layout.fillWidth:true;maximumLength:256;placeholderText:T3I18n.tr("Search refs...");font.pixelSize:Style.font.caption;color:Color.popups.text;placeholderTextColor:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.5);background:Rectangle{radius:Style.space(5);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.04)}
    onTextEdited:debounce.restart()
    Keys.onDownPressed:if(list.count){list.currentIndex=0;list.forceActiveFocus()}
    onAccepted:if(list.currentIndex>=0&&list.currentIndex<root.choices.length)root.choose(root.valueFor(root.choices[list.currentIndex]));else if(root.choices.length===1)root.choose(root.valueFor(root.choices[0]))
   }
   RowLayout {Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr("Branch");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}Text{text:T3I18n.tr("Remote");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}}
   T3PickerButton {objectName:"t3BaseRefAutomatic";Layout.fillWidth:true;visible:!search.text.trim();primary:!root.selectedRef;text:T3I18n.tr("Automatic");onClicked:root.choose("")}
   WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.local.busy||!!root.view.remote.busy;active:visible}
   Text{visible:!root.choices.length&&!root.view.local.busy&&!root.view.remote.busy;Layout.fillWidth:true;text:T3I18n.tr("No matching refs.");color:Color.popups.text;opacity:0.6;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   ListView {
    id:list;objectName:"t3BaseRefRows";Layout.fillWidth:true;Layout.fillHeight:true;Layout.minimumHeight:Style.space(48);implicitHeight:Math.min(Style.space(230),contentHeight);clip:true;model:root.choices;currentIndex:-1;keyNavigationEnabled:true;reuseItems:true
    T3Wheel{flickable:list}
    Controls.ScrollBar.vertical:Controls.ScrollBar{}
    Keys.onReturnPressed:if(currentIndex>=0&&currentIndex<root.choices.length)root.choose(root.valueFor(root.choices[currentIndex]))
    delegate:Rectangle {
     id:row;required property var modelData;required property int index;width:list.width;height:Style.space(32);radius:Style.space(4);color:list.activeFocus&&list.currentIndex===index?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.13):"transparent"
     RowLayout {anchors.fill:parent;spacing:Style.space(4)
     T3PickerButton {Layout.fillWidth:true;Layout.minimumWidth:0;primary:root.valueFor(row.modelData)===root.selectedRef;onClicked:root.choose(root.valueFor(row.modelData));contentItem:Text{text:row.modelData.label;color:Color.popups.text;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}}
     T3Switch {objectName:"t3RefRemote_"+row.index;visible:!!row.modelData.local&&!!row.modelData.remote;checked:!!row.modelData.remote&&root.selectedRef===row.modelData.remote.name;Accessible.name:T3I18n.tr("Remote")+" · "+row.modelData.label;onToggled:root.choose(checked?row.modelData.remote.name:row.modelData.local.name);padding:0;implicitWidth:Style.space(40);implicitHeight:Style.space(26)}
     T3Icon {visible:!row.modelData.local&&!!row.modelData.remote;name:"check";implicitWidth:Style.space(40);tint:Color.popups.text;Accessible.name:T3I18n.tr("Remote only")}
     }
    }
   }
   Repeater {model:["local","remote"]
    RowLayout {
     id:page;required property string modelData;Layout.fillWidth:true;property var state:root.view[modelData]
     Text{Layout.fillWidth:true;visible:!!page.state.error;text:T3I18n.error(page.state.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
     T3PickerButton {objectName:"t3RefMore_"+page.modelData;primary:false;visible:page.state.nextCursor!==null&&page.state.nextCursor!==undefined || !!page.state.error;enabled:!page.state.busy;text:T3I18n.tr(page.state.error?"Retry":"Load more")+" · "+T3I18n.tr(page.modelData==="local"?"Local":"Remote");onClicked:root.requested(search.text,page.modelData,page.state.error?page.state.lastCursor:page.state.nextCursor)}
    }
   }
  }
 }
}
