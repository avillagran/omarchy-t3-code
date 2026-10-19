import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
import qs.Ui
// Optional OmaBridge mini widget next to the T3 status in the bar: the device's
// state at a glance, a badge for requests waiting for approval, and a popup to
// approve them or finish pairing (T3OmaBridgeQuick). Everything else stays in Preferences.
Item {
 id:root
 required property var bridge
 property var bar:null
 property bool vertical:false
 signal settingsRequested()
 readonly property var store:bridge.connectionOmaBridge || ({})
 readonly property var value:store.data || ({})
 readonly property var published:value.published || ({})
 readonly property int pendingCount:(value.pending || []).length
 readonly property color muted:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.58)
 property bool popupOpen:false
 function close(){popupOpen=false;quick.confirming=null}
 function refresh(){if(!store.busy)bridge.requestConnectionOmaBridge("read")}
 implicitWidth:Style.bar.statusSlot
 implicitHeight:Style.bar.statusSlot
 Component.onCompleted:refresh()
 Timer{interval:60000;repeat:true;running:true;onTriggered:root.refresh()}
 BarIconButton{
  id:button;objectName:"omabridgeIndicator"
  anchors.fill:parent
  bar:root.bar
  active:root.popupOpen
  slotSize:Style.bar.statusSlot
  tooltipText:"OmaBridge · "+quick.statusText()+(root.pendingCount?" · "+T3I18n.tr("Waiting for your approval"):"")
  iconComponent:Component{Item{implicitWidth:Style.space(16);implicitHeight:implicitWidth
   T3Icon{anchors.fill:parent;name:"plug";tint:root.published.active || (root.value.peers || []).length?Color.popups.text:root.muted}
   Rectangle{objectName:"omabridgeIndicatorBadge";visible:root.pendingCount>0;width:Style.space(8);height:width;radius:width/2;color:Color.urgent;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:-Style.space(2)}
  }}
  onPressed:function(mouseButton){
   if(mouseButton===Qt.MiddleButton){root.refresh();return}
   root.popupOpen=!root.popupOpen;if(root.popupOpen)root.refresh();else quick.confirming=null
  }
 }
 PopupCard{
  id:popup
  anchorItem:root;owner:root;bar:root.bar;open:root.popupOpen
  contentWidth:popup.fittedContentWidth(Style.space(320))
  contentHeight:popup.fittedContentHeight(quick.implicitHeight)
  T3OmaBridgeQuick{id:quick;width:parent.width;bridge:root.bridge;onSettingsRequested:{root.close();root.settingsRequested()}}
 }
}
