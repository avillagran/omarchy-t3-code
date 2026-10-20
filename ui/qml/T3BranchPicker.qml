import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// The thread's branch, from T3's branch toolbar: search local and remote refs,
// switch the checkout or create a branch from what was typed.
Item {
 id:root;objectName:"t3BranchPicker"
 required property var bridge
 readonly property var state:bridge.branchRefs || ({})
 readonly property string branch:String(state.pendingBranch || bridge.selectedInfo.branch || "")
 readonly property string query:search.text.trim().split(/\s+/).join("-")
 readonly property var refs:state.refs || []
 readonly property bool canCreate:!!query&&!refs.some(function(r){return r.name===root.query})
 readonly property bool available:!!bridge.selectedThread&&!!bridge.selectedInfo.projectId&&bridge.selectedInfo.projectIsScratch!==true
 implicitHeight:Style.space(28);implicitWidth:Math.min(Style.space(190),trigger.implicitWidth)
 visible:available
 onVisibleChanged:if(!visible)popup.close()
 function open(){if(!available)return;search.text="";popup.open();bridge.requestBranchRefs("");search.forceActiveFocus()}
 function choose(ref){bridge.switchBranch(ref);popup.close()}
 function create(){if(canCreate){bridge.createBranch(query);popup.close()}}
 T3PickerButton {
  id:trigger;objectName:"t3BranchTrigger";anchors.fill:parent;primary:false;implicitHeight:Style.space(28);leftPadding:Style.space(8);rightPadding:Style.space(8)
  enabled:!root.state.switching&&!root.bridge.selectedInfo.working
  text:root.branch || T3I18n.tr("Branch")
  Accessible.name:T3I18n.tr("Branch")+" · "+text
  readonly property bool failed:!!root.state.error&&root.state.key===root.bridge.selectedKey&&!root.state.busy
  Controls.ToolTip.visible:hovered||failed;Controls.ToolTip.delay:failed?0:500
  Controls.ToolTip.text:failed?T3I18n.error(root.state.error):T3I18n.tr("Switch or create a branch")+" · Ctrl+Shift+G"
  onClicked:root.open()
  contentItem:RowLayout{spacing:Style.space(5)
   T3Icon{name:"git-branch";implicitWidth:Style.space(13);tint:trigger.failed?Color.urgent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.7)}
   Text{Layout.fillWidth:true;Layout.maximumWidth:Style.space(150);text:trigger.text;color:Color.popups.text;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
   WorkIndicator{visible:!!root.state.switching;active:visible;implicitWidth:Style.space(12);implicitHeight:implicitWidth}
  }
 }
 Timer{id:debounce;interval:180;onTriggered:if(popup.opened)root.bridge.requestBranchRefs(search.text)}
 Controls.Popup {
  id:popup;objectName:"t3BranchPopup";parent:root;x:Math.min(0,root.width-width);y:-height-Style.space(4)
  width:Style.space(320);height:Math.min(Style.space(400),content.implicitHeight+padding*2);padding:Style.space(8);focus:true
  closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutsideParent
  background:Rectangle{radius:Style.space(8);color:Color.popups.background;border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.14)}
  onClosed:debounce.stop()
  ColumnLayout {
   id:content;anchors.fill:parent;spacing:Style.space(5)
   Controls.TextField {
    id:search;objectName:"t3BranchSearch";Layout.fillWidth:true;maximumLength:256;placeholderText:T3I18n.tr("Search or create a branch...");font.pixelSize:Style.font.caption;color:Color.popups.text
    placeholderTextColor:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.5)
    background:Rectangle{radius:Style.space(5);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.04)}
    onTextEdited:debounce.restart()
    Keys.onDownPressed:if(list.count){list.currentIndex=0;list.forceActiveFocus()}
    onAccepted:{var exact=root.refs.find(function(r){return r.name===root.query});if(exact)root.choose(exact);else if(root.canCreate)root.create();else if(root.refs.length===1)root.choose(root.refs[0])}
   }
   T3PickerButton {objectName:"t3BranchCreate";Layout.fillWidth:true;visible:root.canCreate;primary:false;text:T3I18n.tr("Create branch {name}",{name:root.query});onClicked:root.create()}
   WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.state.busy;active:visible}
   Text{visible:!!root.state.error;Layout.fillWidth:true;text:T3I18n.error(root.state.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   Text{visible:!root.state.busy&&!root.state.error&&!root.refs.length;Layout.fillWidth:true;text:T3I18n.tr("No matching refs.");color:Color.popups.text;opacity:0.6;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   ListView {
    id:list;objectName:"t3BranchRows";Layout.fillWidth:true;Layout.fillHeight:true;implicitHeight:Math.min(Style.space(280),contentHeight);clip:true;model:root.refs;currentIndex:-1;keyNavigationEnabled:true
    T3Wheel{flickable:list}
    Controls.ScrollBar.vertical:Controls.ScrollBar{}
    Keys.onReturnPressed:if(currentIndex>=0)root.choose(root.refs[currentIndex])
    delegate:Rectangle {
     id:row;required property var modelData;required property int index;width:list.width;height:Style.space(30);radius:Style.space(4)
     color:list.activeFocus&&list.currentIndex===index||rowHover.hovered?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.13):"transparent"
     HoverHandler{id:rowHover;cursorShape:Qt.PointingHandCursor}
     TapHandler{onTapped:root.choose(row.modelData)}
     RowLayout{anchors.fill:parent;anchors.leftMargin:Style.space(8);anchors.rightMargin:Style.space(8);spacing:Style.space(6)
      T3Icon{name:row.modelData.current?"check":"git-branch";implicitWidth:Style.space(13);tint:row.modelData.current?Color.accent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55)}
      Text{Layout.fillWidth:true;text:row.modelData.name;color:Color.popups.text;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
      Text{visible:!!row.modelData.isDefault||!!row.modelData.isRemote||!!row.modelData.worktreePath;text:T3I18n.tr(row.modelData.isDefault?"default":row.modelData.worktreePath?"worktree":"remote");color:Color.popups.text;opacity:0.5;font.pixelSize:Style.font.caption}
     }
    }
   }
  }
 }
}
