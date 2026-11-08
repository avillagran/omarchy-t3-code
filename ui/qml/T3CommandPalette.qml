import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's command palette: actions, recent threads, projects and settings pages,
// filtered by what you type. Groups come from the panel; this only searches and runs.
Controls.Popup {
 id:root;objectName:"t3CommandPalette"
 property var groups:[]
 // Results that arrive after typing (server-wide message search).
 property var liveGroups:[]
 signal searched(string query)
 signal rankingRequested(var input,var callback)
 Timer{id:searchDelay;interval:250;onTriggered:root.searched(root.query)}
 readonly property string query:search.text
 property var rankedGroups:[]
 property var shownOrder:null
 property string rankError:""
 property bool rankBusy:false
 property bool pendingAccept:false
 property int rankGeneration:0
 property string messageQuery:""
 Timer{id:rankDelay;interval:0;onTriggered:root.requestRanking()}
 function metadata(item){
  var title=T3I18n.tr(item.title),row={value:item.threadKey?"thread:"+item.threadKey:String(item.id),
    title:title,kind:"action",searchTerms:item.searchTerms||[title,String(item.title),String(item.detail||"")],secondary:item.secondary===true}
  if(item.threadKey)row.threadKey=item.threadKey
  if(item.messageMatch)Object.assign(row,item.messageMatch,{value:"thread:"+item.threadKey,threadKey:item.threadKey,title:title,searchTerms:row.searchTerms})
  return row
 }
 function originalItem(value,message){
  var all=message?liveGroups.concat(groups):groups.concat(liveGroups)
  for(var g=0;g<all.length;g++){
   var items=all[g].items||[]
   for(var i=0;i<items.length;i++)if(metadata(items[i]).value===value)return items[i]
  }
  return null
 }
 function titleText(item){
  function escape(value){return String(value).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}
  if(!item.titleParts)return escape(T3I18n.tr(item.title))
  return item.titleParts.map(function(part){var text=escape(part.text);return part.highlighted?"<b>"+text+"</b>":text}).join("")
 }
 function requestRanking(){
  if(!opened)return
  var generation=++rankGeneration,queryNow=query,all=groups.concat(liveGroups)
  var active=groups.map(function(group){return {value:group.value,label:group.label,items:(group.items||[]).map(metadata)}})
  function items(value){var group=all.find(function(group){return group.value===value});return group?(group.items||[]).map(metadata):[]}
  rankBusy=true
  rankingRequested({query:queryNow,activeGroups:active,threadSearchItems:items("recent-threads"),
    projectSearchItems:items("projects-search"),settingsSearchItems:items("settings-search"),
    messageMatches:items("threads-messages"),messageQuery:messageQuery,shownOrder:shownOrder},function(result,error){
   if(!root.opened||generation!==root.rankGeneration||queryNow!==root.query)return
   root.rankBusy=false;root.rankError=error||""
   if(error){root.rankedGroups=[];root.pendingAccept=false;return}
   root.shownOrder=result.shownOrder||null
   root.rankedGroups=(result.groups||[]).map(function(group){
    return {label:group.label,items:(group.items||[]).map(function(match){
      var item=root.originalItem(match.value,group.value==="threads-messages")
      return item?Object.assign({},item,{matchHint:match.matchHint||null,titleParts:match.titleParts||null,matchFind:match.find||null}):null
    }).filter(function(item){return item!==null})}
   }).concat(root.liveGroups.filter(function(group){return group.extension===true}).map(function(group){
      return {label:group.label,items:(group.items||[]).slice(0,group.limit||8)}
   }))
   if(root.pendingAccept){root.pendingAccept=false;root.run((root.runnable[root.current]||{}).item)}
  })
 }
 readonly property var rows:{
  var out=[]
  for(var g=0;g<rankedGroups.length;g++){
   var group=rankedGroups[g],items=group.items||[]
   if(!items.length)continue
   out.push({header:true,label:group.label})
   items.forEach(function(item){out.push({header:false,item:item})})
  }
  return out
 }
 readonly property var runnable:rows.filter(function(r){return !r.header})
 property int current:0
 property string currentValue:""
 onRowsChanged:{var index=runnable.findIndex(function(row){return row.item.id===root.currentValue});current=index>=0?index:Math.max(0,Math.min(current,runnable.length-1));currentValue=(runnable[current]||{}).item?String(runnable[current].item.id):""}
 onQueryChanged:{current=0;currentValue="";pendingAccept=false;rankGeneration++;rankBusy=opened;rankDelay.restart();searchDelay.restart()}
 onGroupsChanged:rankDelay.restart()
 onLiveGroupsChanged:rankDelay.restart()
 onMessageQueryChanged:rankDelay.restart()
 onOpened:rankDelay.restart()
 onClosed:{rankGeneration++;rankBusy=false;pendingAccept=false;shownOrder=null;rankedGroups=[]}
 function show(){rankGeneration++;rankedGroups=[];shownOrder=null;pendingAccept=false;search.text="";rankBusy=true;current=0;currentValue="";open();search.forceActiveFocus()}
 function run(item){if(rankBusy||!item||item.enabled===false)return;var picked=query;close();Qt.callLater(function(){item.run(picked,item.matchFind)})}
 function move(step){if(!runnable.length)return;current=(current+step+runnable.length)%runnable.length;currentValue=String(runnable[current].item.id);var index=rows.indexOf(runnable[current]);list.positionViewAtIndex(index,ListView.Contain)}
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
   onAccepted:{if(root.rankBusy){root.pendingAccept=true;return}root.run((root.runnable[root.current]||{}).item)}
  }
  Text{visible:!root.runnable.length&&!root.rankBusy;Layout.fillWidth:true;Layout.margins:Style.space(8);text:T3I18n.tr(root.rankError?"Search is unavailable.":"No results.");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}
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
      Text{Layout.fillWidth:true;Layout.minimumWidth:0;text:row.modelData.header?"":root.titleText(row.modelData.item);color:Color.popups.text;elide:Text.ElideRight;textFormat:Text.StyledText;font.pixelSize:Style.font.body}
      Text{visible:!row.modelData.header&&!!(row.modelData.item.matchHint||row.modelData.item.detail);Layout.maximumWidth:Style.space(200);text:row.modelData.header?"":row.modelData.item.matchHint?T3I18n.tr(row.modelData.item.matchHint.label)+": "+row.modelData.item.matchHint.text:String(row.modelData.item.detail||"");color:Color.popups.text;opacity:0.5;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
      Text{visible:!row.modelData.header&&!!row.modelData.item.shortcut;text:row.modelData.header?"":String(row.modelData.item.shortcut||"");color:Color.popups.text;opacity:0.45;font.pixelSize:Style.font.caption}
     }
    }
   }
  }
 }
}
