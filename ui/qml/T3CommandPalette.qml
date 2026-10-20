import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's command palette: actions, recent threads, projects and settings pages,
// filtered by what you type. Groups come from the panel; this only searches and runs.
Controls.Popup {
 id:root;objectName:"t3CommandPalette"
 property var groups:[]
 readonly property string query:search.text.trim().toLocaleLowerCase()
 // Every query word must appear in the title or detail; earlier and title matches rank first.
 function score(item){
  if(!query)return 1
  var title=T3I18n.tr(item.title).toLocaleLowerCase(),english=String(item.title).toLocaleLowerCase(),detail=String(item.detail||"").toLocaleLowerCase(),words=query.split(/\s+/),total=0
  for(var i=0;i<words.length;i++){var w=words[i],at=title.indexOf(w);if(at<0)at=english.indexOf(w);if(at>=0){total+=at===0?3:2;continue}if(detail.indexOf(w)>=0){total+=1;continue}return 0}
  return total
 }
 readonly property var rows:{
  var out=[]
  for(var g=0;g<groups.length;g++){
   var group=groups[g],items=(group.items||[]).map(function(item){return {item:item,score:root.score(item)}}).filter(function(x){return x.score>0})
   if(query)items.sort(function(a,b){return b.score-a.score})
   items=items.slice(0,group.limit||(query?12:8))
   if(!items.length)continue
   out.push({header:true,label:group.label})
   items.forEach(function(x){out.push({header:false,item:x.item})})
  }
  return out
 }
 readonly property var runnable:rows.filter(function(r){return !r.header})
 property int current:0
 onQueryChanged:current=0
 function show(){search.text="";current=0;open();search.forceActiveFocus()}
 function run(item){if(!item||item.enabled===false)return;close();Qt.callLater(function(){item.run()})}
 function move(step){if(!runnable.length)return;current=(current+step+runnable.length)%runnable.length;var index=rows.indexOf(runnable[current]);list.positionViewAtIndex(index,ListView.Contain)}
 modal:true;focus:true;padding:Style.space(8)
 x:Math.round((parent.width-width)/2);y:Math.round(parent.height*0.12)
 width:Math.min(Style.space(560),parent.width-Style.space(40));height:Math.min(Style.space(460),content.implicitHeight+padding*2)
 background:Rectangle{radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.16)}
 ColumnLayout {
  id:content;anchors.fill:parent;spacing:Style.space(6)
  Controls.TextField {
   id:search;objectName:"t3CommandPaletteSearch";Layout.fillWidth:true;maximumLength:256
   placeholderText:T3I18n.tr("Search commands, threads and settings...");font.pixelSize:Style.font.body;color:Color.popups.text
   placeholderTextColor:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.5)
   background:Rectangle{radius:Style.space(6);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.05)}
   Keys.onDownPressed:root.move(1)
   Keys.onUpPressed:root.move(-1)
   Keys.onTabPressed:root.move(1)
   Keys.onBacktabPressed:root.move(-1)
   onAccepted:root.run((root.runnable[root.current]||{}).item)
  }
  Text{visible:!root.runnable.length;Layout.fillWidth:true;Layout.margins:Style.space(8);text:T3I18n.tr("No results.");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}
  ListView {
   id:list;objectName:"t3CommandPaletteRows";Layout.fillWidth:true;Layout.fillHeight:true;implicitHeight:Math.min(Style.space(380),contentHeight);clip:true;model:root.rows;boundsBehavior:Flickable.StopAtBounds
   T3Wheel{flickable:list}
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
   delegate:Item {
    id:row;required property var modelData;required property int index;width:list.width
    readonly property bool selected:!modelData.header&&root.runnable[root.current]===modelData
    height:modelData.header?Style.space(24):Style.space(34)
    Text{visible:row.modelData.header;anchors.left:parent.left;anchors.leftMargin:Style.space(8);anchors.bottom:parent.bottom;anchors.bottomMargin:Style.space(3);text:T3I18n.tr(row.modelData.label||"");color:Color.popups.text;opacity:0.5;font.pixelSize:Style.font.caption;font.weight:Font.DemiBold}
    Rectangle {
     visible:!row.modelData.header;anchors.fill:parent;radius:Style.space(6)
     objectName:row.modelData.header?"":"t3PaletteItem_"+(row.modelData.item.id||"")
     opacity:row.modelData.header||row.modelData.item.enabled!==false?1:0.45
     color:row.selected||rowHover.hovered?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.14):"transparent"
     HoverHandler{id:rowHover;cursorShape:Qt.PointingHandCursor}
     TapHandler{onTapped:if(!row.modelData.header)root.run(row.modelData.item)}
     RowLayout {
      anchors.fill:parent;anchors.leftMargin:Style.space(8);anchors.rightMargin:Style.space(10);spacing:Style.space(9)
      T3Icon{visible:!row.modelData.header;name:row.modelData.header?"":(row.modelData.item.icon||"chevron-right");implicitWidth:Style.space(15);tint:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.7)}
      Text{Layout.fillWidth:true;Layout.minimumWidth:0;text:row.modelData.header?"":T3I18n.tr(row.modelData.item.title);color:Color.popups.text;elide:Text.ElideRight;textFormat:Text.PlainText;font.pixelSize:Style.font.body}
      Text{visible:!row.modelData.header&&!!row.modelData.item.detail;Layout.maximumWidth:Style.space(200);text:row.modelData.header?"":String(row.modelData.item.detail||"");color:Color.popups.text;opacity:0.5;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
      Text{visible:!row.modelData.header&&!!row.modelData.item.shortcut;text:row.modelData.header?"":String(row.modelData.item.shortcut||"");color:Color.popups.text;opacity:0.45;font.pixelSize:Style.font.caption}
     }
    }
   }
  }
 }
}
