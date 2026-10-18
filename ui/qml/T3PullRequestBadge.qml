import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons
Item {
 id:root
 property var badge:null
 property bool active:true
 function closePreview(){if(openDelay)openDelay.stop();if(preview)preview.close()}
 onActiveChanged:if(!active)closePreview()
 onBadgeChanged:closePreview()
 readonly property var links:badge&&badge.links || []
 readonly property color dim:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55)
 readonly property color tone:colorFor(badge&&badge.state)
 implicitWidth:mark.implicitWidth;implicitHeight:Style.space(16)
 activeFocusOnTab:visible
 Accessible.role:Accessible.Button
 Accessible.name:T3I18n.tr("Pull requests")+" "+(badge?badge.text:"")
 signal activated(var reference,bool multiple)
 signal previewOpened()
 function colorFor(state){return state==="open"?T3Palette.done:state==="merged"?T3Palette.pick("magenta",Color.accent):state==="closed"?T3Palette.failed:dim}
 function iconFor(state,stack){return stack?"layers":state==="merged"?"git-merge":state==="closed"?"git-pull-request-closed":state==="draft"?"git-pull-request-draft":"git-pull-request"}
 function activate(){if(badge&&badge.current)activated(badge.current,badge.kind!=="single")}
 Keys.onReturnPressed:activate()
 Keys.onEnterPressed:activate()
 Keys.onSpacePressed:activate()
 Row{id:mark;anchors.verticalCenter:parent.verticalCenter;spacing:Style.space(2)
  T3Icon{anchors.verticalCenter:parent.verticalCenter;implicitWidth:Style.space(12);name:root.iconFor(root.badge&&root.badge.state,root.badge&&root.badge.kind==="stack");tint:root.tone}
  Text{text:root.badge?root.badge.text:"";font.pixelSize:Style.font.caption;color:root.tone;textFormat:Text.PlainText}
 }
 Rectangle{anchors.fill:parent;color:"transparent";border.width:root.activeFocus?1:0;border.color:Color.accent;radius:Style.space(2)}
 MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:root.activate()}
 HoverHandler{id:badgeHover}
 Timer{id:openDelay;interval:350;running:root.active&&root.visible&&badgeHover.hovered;onTriggered:{root.previewOpened();preview.open()}}
 Timer{interval:150;running:preview.visible&&!badgeHover.hovered&&!previewHover.hovered;onTriggered:preview.close()}
 onVisibleChanged:if(!visible&&preview)preview.close()
 Controls.Popup{
  id:preview;objectName:"threadPullRequestPreview";width:Style.space(320);x:Math.min(0,root.width-width);y:-height-Style.space(2);padding:Style.space(8);focus:false
  background:Rectangle{color:Color.popups.background;radius:Style.space(10);border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.15)}
  contentItem:ListView{
   implicitHeight:Math.min(Style.space(220),root.links.length*Style.space(42));clip:true;model:root.links
   HoverHandler{id:previewHover}
   delegate:Item{required property var modelData;width:ListView.view.width;height:Style.space(42)
    readonly property string prState:modelData.snapshot&&modelData.snapshot.isDraft?"draft":modelData.snapshot&&modelData.snapshot.state || "unknown"
    Row{anchors{left:parent.left;right:parent.right;verticalCenter:parent.verticalCenter;margins:Style.space(5)} spacing:Style.space(6)
     T3Icon{name:root.iconFor(parent.parent.prState,false);tint:root.colorFor(parent.parent.prState);implicitWidth:Style.space(12)}
     Column{width:parent.width-Style.space(24);spacing:Style.space(3)
      Text{text:"#"+modelData.number+" · "+modelData.repository;color:root.colorFor(parent.parent.parent.prState);font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;width:parent.width;textFormat:Text.PlainText}
      Text{text:modelData.snapshot&&modelData.snapshot.title || modelData.url;color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;width:parent.width;textFormat:Text.PlainText}
     }
    }
    MouseArea{anchors.fill:parent;cursorShape:Qt.PointingHandCursor;onClicked:{root.activated(modelData,false);preview.close()}}
   }
   Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
  }
 }
}
