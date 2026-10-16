import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle{
 id:root
 required property var bridge
 signal closed()
 property var initialEntry:null
 property string environment:initialEntry&&initialEntry.environmentId || bridge.selectedEnvironment || "local"
 readonly property bool threadScoped:!!initialEntry&&Array.isArray(initialEntry.references)
 property string stateFilter:threadScoped?"all":"open"
 property string involvement:"all"
 property var selected:null
 readonly property bool narrow:width<Style.space(650)
 property int tab:0
 property var entries:[]
 property bool appending:false
 property string patch:""
 property bool appendPatch:false
 readonly property var listResult:bridge.prOperations.list || ({})
 readonly property var detailResult:bridge.prOperations.detail || ({})
 readonly property var detail:detailResult.data || selected || ({})
 readonly property var tabResult:bridge.prOperations[tab===1?"checks":tab===2?"activity":"diff"] || ({})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 color:Color.popups.background
 function statusLabel(state){return T3I18n.tr(({"approved":"Approved","changes-requested":"Changes requested","review-required":"Review required","pending":"Pending","action-required":"Action required","success":"Done","failure":"Failed","skipped":"Skipped","cancelled":"Cancelled","neutral":"Done"})[state] || state)}
 function tint(state){return ["failure","failing","closed","changes-requested"].indexOf(state)>=0?T3Palette.failed:["success","passing","open","approved"].indexOf(state)>=0?T3Palette.done:state==="merged"?Color.accent:dim}
 function reference(){return selected?{projectId:selected.projectId,host:selected.host,repository:selected.repository,number:selected.number}:null}
 function loadList(more){if(threadScoped){var needle=search.text.toLowerCase();entries=initialEntry.references.map(function(e){return Object.assign({},e,e.snapshot || {},{title:e.snapshot&&e.snapshot.title || "#"+e.number,state:e.snapshot&&e.snapshot.state || "unknown"})}).filter(function(e){return (stateFilter==="all"||e.state===stateFilter)&&(!needle||(e.title+" "+e.repository+" "+e.number).toLowerCase().indexOf(needle)>=0)});return}appending=!!more;bridge.requestPullRequests("list",{environment:environment,state:stateFilter,involvement:involvement,query:search.text,cursors:more&&listResult.data?listResult.data.nextCursors:undefined})}
 function openEntry(entry){selected=entry;tab=0;patch="";bridge.requestPullRequests("detail",{environment:environment,reference:reference()})}
 function loadTab(index){tab=index;if(!selected||index===0)return;appendPatch=false;bridge.requestPullRequests(index===1?"checks":index===2?"activity":"diff",{environment:environment,reference:reference()})}
 onListResultChanged:if(!threadScoped&&listResult.data){var next=appending?entries.slice():[];(listResult.data.entries || []).forEach(function(e){var i=next.findIndex(function(x){return x.host===e.host&&x.repository===e.repository&&x.number===e.number});if(i<0)next.push(e);else next[i]=e});entries=next.sort(function(a,b){return Date.parse(b.updatedAt)-Date.parse(a.updatedAt)})}
 onTabResultChanged:if(tab===3&&tabResult.data)patch=(appendPatch?patch:"")+(tabResult.data.patch || "")
 Component.onCompleted:{bridge.resetPullRequests();loadList(false);if(initialEntry&&!initialEntry.multiple)openEntry(entries.find(function(e){return e.number===initialEntry.number&&e.host===initialEntry.host&&e.repository===initialEntry.repository}) || initialEntry)}
 Component.onDestruction:bridge.resetPullRequests()
 ColumnLayout{
  anchors.fill:parent;anchors.margins:Style.space(16);spacing:Style.space(10)
  RowLayout{
   Layout.fillWidth:true
   T3IconButton{name:"chevron-left";tooltipText:T3I18n.tr("Back to chat");onClicked:if(root.narrow&&root.selected)root.selected=null;else root.closed()}
   T3Icon{name:"git-pull-request";tint:Color.accent}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Pull requests");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
   T3Select{enabled:!root.threadScoped;Layout.preferredWidth:Style.space(130);model:root.environments;textRole:"name";currentIndex:Math.max(0,model.findIndex(function(e){return e.id===root.environment}));onActivated:function(i){root.environment=model[i].id;root.selected=null;root.entries=[];root.bridge.resetPullRequests();root.loadList(false)}}
   T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:{root.loadList(false);if(root.selected)root.openEntry(root.selected)}}
  }
  RowLayout{
   Layout.fillWidth:true
   Controls.TextField{id:search;Layout.fillWidth:true;placeholderText:T3I18n.tr("Search pull requests…");color:root.fg;placeholderTextColor:root.dim;font.pixelSize:Style.font.caption;implicitHeight:Style.space(32);background:Rectangle{radius:Style.space(7);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04);border.width:1;border.color:search.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12)} onTextEdited:searchDelay.restart();onAccepted:{searchDelay.stop();root.loadList(false)}}
   T3Select{Layout.preferredWidth:Style.space(110);translateLabels:true;model:["Open","Merged","Closed","All"];currentIndex:["open","merged","closed","all"].indexOf(root.stateFilter);onActivated:function(i){root.stateFilter=["open","merged","closed","all"][i];root.loadList(false)}}
   T3Select{enabled:!root.threadScoped;Layout.preferredWidth:Style.space(115);translateLabels:true;model:["All","Authored","Reviewing"];onActivated:function(i){root.involvement=["all","authored","reviewing"][i];root.loadList(false)}}
  }
  Text{Layout.fillWidth:true;visible:!!root.listResult.error;text:T3I18n.error(root.listResult.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Text{Layout.fillWidth:true;visible:!!root.listResult.data&&(root.listResult.data.errors || []).length>0;text:root.listResult.data?(root.listResult.data.errors || []).map(function(e){return e.projectTitle+": "+e.message}).join("\n"):"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
  RowLayout{
   Layout.fillWidth:true;Layout.fillHeight:true;spacing:Style.space(12)
   ColumnLayout{
    visible:!root.narrow||!root.selected
    Layout.preferredWidth:root.selected?Math.max(Style.space(180),root.width*0.34):root.width;Layout.fillWidth:!root.selected;Layout.fillHeight:true
    ListView{
     id:list;objectName:"pullRequestList";Layout.fillWidth:true;Layout.fillHeight:true;model:root.entries;clip:true;reuseItems:false;cacheBuffer:0;spacing:Style.space(3)
     T3Wheel{flickable:list}
     Controls.ScrollBar.vertical:Controls.ScrollBar{}
     delegate:Rectangle{
      id:entry;required property var modelData
      width:list.width;height:row.implicitHeight+Style.space(18);radius:Style.space(7)
      color:root.selected&&root.selected.url===modelData.url?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.1):hover.containsMouse?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent"
      activeFocusOnTab:true;Accessible.role:Accessible.Button;Accessible.name:modelData.title
      Keys.onReturnPressed:root.openEntry(modelData)
      ColumnLayout{id:row;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(9)} spacing:Style.space(5)
       RowLayout{Layout.fillWidth:true;T3Icon{name:"git-pull-request";tint:root.tint(entry.modelData.state);implicitWidth:Style.space(14)}Text{Layout.fillWidth:true;text:entry.modelData.title;color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}T3Icon{name:entry.modelData.checksState==="passing"?"check":"circle-alert";visible:!!entry.modelData.checksState;tint:root.tint(entry.modelData.checksState);implicitWidth:Style.space(13)}}
       Text{Layout.fillWidth:true;text:entry.modelData.repository+" #"+entry.modelData.number+" · "+(entry.modelData.author?entry.modelData.author.login:"");color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
       Text{Layout.fillWidth:true;visible:!!entry.modelData.isDraft||!!entry.modelData.reviewDecision;text:entry.modelData.isDraft?T3I18n.tr("Draft"):root.statusLabel(entry.modelData.reviewDecision || "");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      }
      MouseArea{id:hover;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:{entry.forceActiveFocus();root.openEntry(entry.modelData)}}
     }
     Text{anchors.centerIn:parent;visible:!list.count;text:root.listResult.busy?T3I18n.tr("Loading…"):T3I18n.tr("No pull requests");color:root.dim;font.pixelSize:Style.font.caption}
    }
    Controls.Button{Layout.alignment:Qt.AlignHCenter;visible:!!root.listResult.data&&Object.keys(root.listResult.data.nextCursors || {}).length>0;text:T3I18n.tr("Load more");enabled:!root.listResult.busy;onClicked:root.loadList(true)}
   }
   Rectangle{Layout.fillHeight:true;implicitWidth:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1);visible:!!root.selected&&!root.narrow}
   ColumnLayout{
    Layout.fillWidth:true;Layout.fillHeight:true;visible:!!root.selected;spacing:Style.space(8)
    RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:root.detail.title || "";color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}T3IconButton{name:"external-link";tooltipText:T3I18n.tr("Open in browser");onClicked:if(/^https?:\/\//.test(root.detail.url || ""))Qt.openUrlExternally(root.detail.url)}}
    Text{Layout.fillWidth:true;text:(root.detail.headBranch || "")+" → "+(root.detail.baseBranch || "")+"   +"+(root.detail.additions || 0)+" −"+(root.detail.deletions || 0);color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    RowLayout{Layout.fillWidth:true;Repeater{model:["Overview","Checks","Conversation","Code"];T3IconButton{required property string modelData;required property int index;name:["list-ordered","circle-check","square-pen","file"][index];tooltipText:T3I18n.tr(modelData);tint:root.tab===index?Color.accent:root.dim;onClicked:root.loadTab(index)}}Text{Layout.fillWidth:true;text:T3I18n.tr(["Overview","Checks","Conversation","Code"][root.tab]);color:root.dim;font.pixelSize:Style.font.caption}}
    Text{Layout.fillWidth:true;visible:!!root.detailResult.error||!!root.tabResult.error;text:T3I18n.error(root.detailResult.error || root.tabResult.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Controls.ScrollView{
     id:description;Layout.fillWidth:true;Layout.fillHeight:true;visible:root.tab===0;clip:true;contentWidth:availableWidth
     T3Wheel{flickable:description.contentItem}
     Text{id:prDescription;HoverHandler{cursorShape:prDescription.hoveredLink?Qt.PointingHandCursor:Qt.ArrowCursor}width:description.availableWidth;text:root.detailResult.busy?T3I18n.tr("Loading…"):(root.detail.body || "");color:root.fg;wrapMode:Text.Wrap;textFormat:Text.MarkdownText;font.pixelSize:Style.font.body;onLinkActivated:function(link){if(/^https?:\/\//.test(link))Qt.openUrlExternally(link)}}
    }
    ListView{
     id:checks;Layout.fillWidth:true;Layout.fillHeight:true;visible:root.tab===1;model:root.tabResult.data?(root.tabResult.data.checks || []):[];clip:true;reuseItems:false;cacheBuffer:0
     T3Wheel{flickable:checks}Controls.ScrollBar.vertical:Controls.ScrollBar{}
     delegate:Text{required property var modelData;width:checks.width;height:implicitHeight+Style.space(10);text:(modelData.status==="success"?"✓ ":"● ")+modelData.name+" · "+root.statusLabel(modelData.status)+(modelData.description?"\n"+modelData.description:"");color:root.tint(modelData.status);wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
    }
    ListView{
     id:comments;Layout.fillWidth:true;Layout.fillHeight:true;visible:root.tab===2;model:root.tabResult.data?(root.tabResult.data.comments || []):[];clip:true;reuseItems:false;cacheBuffer:0;spacing:Style.space(14)
     T3Wheel{flickable:comments}Controls.ScrollBar.vertical:Controls.ScrollBar{}
     delegate:Column{required property var modelData;width:comments.width;spacing:Style.space(5);Text{width:parent.width;text:modelData.author?modelData.author.login:"";color:root.dim;font.bold:true;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}Text{width:parent.width;text:modelData.body;color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.MarkdownText}}
    }
    ListView{
     id:code;Layout.fillWidth:true;Layout.fillHeight:true;visible:root.tab===3;model:root.patch.split("\n");clip:true;reuseItems:false;cacheBuffer:0
     T3Wheel{flickable:code}Controls.ScrollBar.vertical:Controls.ScrollBar{}
     delegate:Text{required property string modelData;width:code.width;text:modelData;color:modelData.startsWith("+")?T3Palette.done:modelData.startsWith("-")?T3Palette.failed:modelData.startsWith("@@")?Color.accent:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText}
    }
    Controls.Button{visible:root.tab===3&&!!root.tabResult.data&&!!root.tabResult.data.nextCursor;text:T3I18n.tr("Load more");enabled:!root.tabResult.busy;onClicked:{root.appendPatch=true;root.bridge.requestPullRequests("diff",{environment:root.environment,reference:root.reference(),cursor:root.tabResult.data.nextCursor})}}
    Text{visible:!!root.tabResult.busy;text:T3I18n.tr("Loading…");color:root.dim;font.pixelSize:Style.font.caption}
   }
  }
 }
 Timer{id:searchDelay;interval:300;onTriggered:root.loadList(false)}
}
