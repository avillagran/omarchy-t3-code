import QtQuick 6.11
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
 property string labelFilter:""
 property string authorFilter:""
 property string filterKind:"labels"
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
 readonly property var writeResult:bridge.prOperations.write || ({})
 readonly property var permissions:root.detail.viewerPermissions || ({})
 readonly property var allowed:(permissions.actions || []).filter(function(a){var d=root.detail;if(a==="merge")return d.state==="open"&&!d.isDraft;if(a==="ready")return d.state==="open"&&d.isDraft;if(a==="draft")return d.state==="open"&&!d.isDraft;if(a==="close")return d.state==="open";if(a==="reopen")return d.state==="closed";return false})
 readonly property var mergeMethods:["squash","merge","rebase"].filter(function(m){return !!(root.detail.mergeCapabilities || {})[m]})
 property string mergeMethod:""
 property string confirming:""
 function write(values){bridge.pullRequestWrite(environment,Object.assign({reference:reference()},values))}
 Connections{target:root.bridge;ignoreUnknownSignals:true;function onPullRequestThreadOpened(){root.closed()}}
 Connections{target:root.bridge;function onPullRequestWritten(data){if(data.error||!root.selected)return;if(data.result&&/-candidates$/.test(data.result.kind)){people.candidates=(data.result.result||{}).candidates||[];return}if(people.opened)root.write({kind:people.mode==="labels"?"label-candidates":"reviewer-candidates"});if(data.result&&data.result.kind!=="action")reply.text="";root.confirming="";root.bridge.requestPullRequests("detail",{environment:root.environment,reference:root.reference()});if(root.tab>0)root.loadTab(root.tab)}}
 // This thread's links to pull requests, for link/unlink/watch.
 readonly property var threadLinks:((bridge.selectedInfo.pullRequestBadge || {}).links) || []
 readonly property var currentLink:{var d=root.detail;return root.threadLinks.find(function(l){return l.number===d.number&&String(l.repository).toLowerCase()===String(d.repository||"").toLowerCase()}) || null}
 function linkRef(){var d=root.detail;var host=root.selected&&root.selected.host||(String(d.url||"").match(/^https?:\/\/([^\/]+)/)||[])[1]||"";return {host:host,repository:d.repository,number:d.number,url:d.url}}
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 color:Color.popups.background
 function statusLabel(state){return T3I18n.tr(({"approved":"Approved","changes-requested":"Changes requested","review-required":"Review required","pending":"Pending","action-required":"Action required","success":"Done","failure":"Failed","skipped":"Skipped","cancelled":"Cancelled","neutral":"Done"})[state] || state)}
 function tint(state){return ["failure","failing","closed","changes-requested"].indexOf(state)>=0?T3Palette.failed:["success","passing","open","approved"].indexOf(state)>=0?T3Palette.done:state==="merged"?Color.accent:dim}
 function reference(){return selected?{projectId:selected.projectId,host:selected.host,repository:selected.repository,number:selected.number}:null}
 function loadList(more){if(threadScoped){var needle=search.text.toLowerCase();entries=initialEntry.references.map(function(e){return Object.assign({},e,e.snapshot || {},{title:e.snapshot&&e.snapshot.title || "#"+e.number,state:e.snapshot&&e.snapshot.state || "unknown"})}).filter(function(e){return (stateFilter==="all"||e.state===stateFilter)&&(!needle||(e.title+" "+e.repository+" "+e.number).toLowerCase().indexOf(needle)>=0)});return}appending=!!more;bridge.requestPullRequests("list",{environment:environment,state:stateFilter,involvement:involvement,query:search.text,filters:{labels:labelFilter?[labelFilter]:[],author:authorFilter||undefined},cursors:more&&listResult.data?listResult.data.nextCursors:undefined})}
 function openEntry(entry){selected=entry;tab=0;patch="";bridge.requestPullRequests("detail",{environment:environment,reference:reference()});bridge.requestPullRequests("change-breakdown",{environment:environment,reference:reference()})}
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
  RowLayout {
   Layout.fillWidth:true;visible:!root.threadScoped
   T3PickerButton{objectName:"prLabelFilter";primary:false;text:root.labelFilter||T3I18n.tr("Labels");onClicked:{root.filterKind="labels";filterSearch.text="";root.bridge.requestPullRequests("filter-options",{environment:root.environment,kind:"labels"});filterPopup.open()}}
   T3PickerButton{objectName:"prAuthorFilter";primary:false;text:root.authorFilter||T3I18n.tr("Author");onClicked:{root.filterKind="authors";filterSearch.text="";root.bridge.requestPullRequests("filter-options",{environment:root.environment,kind:"authors"});filterPopup.open()}}
   T3IconButton{visible:!!root.labelFilter||!!root.authorFilter;name:"x";tooltipText:T3I18n.tr("Clear filters");onClicked:{root.labelFilter="";root.authorFilter="";root.loadList(false)}}
  }
  Flow {
   Layout.fillWidth:true;visible:!!root.selected&&!!((root.bridge.prOperations["change-breakdown"]||{}).data);spacing:Style.space(10)
   Repeater{model:["code","tests","mocks","generated","docs"];delegate:Text{required property string modelData;readonly property var counts:((root.bridge.prOperations["change-breakdown"]||{}).data||{})[modelData]||({});text:T3I18n.tr(({code:"Code",tests:"Tests",mocks:"Mocks",generated:"Generated",docs:"Documentation"})[modelData])+" +"+(counts.additions||0)+" −"+(counts.deletions||0);textFormat:Text.PlainText;color:root.dim;font.pixelSize:Style.font.caption}}
  }
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
    // Host actions the viewer may take, as in T3's pull request header. Merging and
    // closing ask once more before running.
    Flow{
     objectName:"pullRequestActions";Layout.fillWidth:true;spacing:Style.space(6);visible:root.tab===0&&!root.detailResult.busy&&(root.allowed.length>0||!!root.detail.url)
     Repeater{model:root.allowed
      T3PickerButton{required property string modelData;objectName:"pullRequestAction_"+modelData;implicitHeight:Style.space(30);primary:modelData==="merge";enabled:!root.writeResult.busy&&(modelData!=="merge"||root.detail.mergeability!=="conflicting")
       text:root.confirming===modelData?T3I18n.tr("Confirm"):T3I18n.tr(({merge:"Merge",ready:"Ready for review",draft:"Convert to draft",close:"Close",reopen:"Reopen"})[modelData])
       onClicked:{if((modelData==="merge"||modelData==="close")&&root.confirming!==modelData){root.confirming=modelData;return}var v={kind:"action",action:modelData};if(modelData==="merge"&&root.mergeMethods.length)v.mergeMethod=root.mergeMethod||root.mergeMethods[0];root.write(v)}}
     }
     T3Select{objectName:"pullRequestMergeMethod";visible:root.allowed.indexOf("merge")>=0&&root.mergeMethods.length>1;implicitWidth:Style.space(130);model:root.mergeMethods.map(function(m){return T3I18n.tr(({squash:"Squash and merge",merge:"Merge commit",rebase:"Rebase and merge"})[m])});currentIndex:Math.max(0,root.mergeMethods.indexOf(root.mergeMethod));onActivated:function(i){root.mergeMethod=root.mergeMethods[i]}}
     T3PickerButton{objectName:"pullRequestLink";visible:!!root.bridge.selectedThread&&!!root.detail.url;implicitHeight:Style.space(30);primary:false;enabled:!(root.bridge.prOperations.link||{}).busy;text:T3I18n.tr(root.currentLink?"Unlink from thread":"Link to thread");onClicked:root.bridge.threadPullRequest(root.currentLink?"unlink":"link",root.linkRef())}
     T3PickerButton{objectName:"pullRequestWatch";visible:!!root.currentLink&&root.detail.state==="open";implicitHeight:Style.space(30);primary:false;enabled:!(root.bridge.prOperations.link||{}).busy;text:T3I18n.tr(root.currentLink&&root.currentLink.watch?"Stop watching":"Watch checks");onClicked:root.bridge.threadPullRequest(root.currentLink.watch?"unwatch":"watch",root.linkRef())}
     T3PickerButton{objectName:"pullRequestOpenThread";visible:!!root.detail.workspaceRoot&&!!root.detail.url;implicitHeight:Style.space(30);primary:false;enabled:!(root.bridge.prOperations.prThread||{}).busy;text:T3I18n.tr("Open in a thread");onClicked:root.bridge.openPullRequestThread(root.environment,root.detail,"local")}
     T3PickerButton{objectName:"pullRequestOpenWorktree";visible:!!root.detail.workspaceRoot&&!!root.detail.url;implicitHeight:Style.space(30);primary:false;enabled:!(root.bridge.prOperations.prThread||{}).busy;text:T3I18n.tr("Open in a worktree");onClicked:root.bridge.openPullRequestThread(root.environment,root.detail,"worktree")}
     T3PickerButton{objectName:"pullRequestReviewers";visible:root.permissions.requestReviewers===true;implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Reviewers");onClicked:people.show("reviewers")}
     T3PickerButton{objectName:"pullRequestLabels";visible:root.permissions.labels===true;implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Labels");onClicked:people.show("labels")}
     Text{visible:root.detail.mergeability==="conflicting";text:T3I18n.tr("This branch has conflicts that must be resolved.");color:Color.urgent;font.pixelSize:Style.font.caption}
    }
    Text{Layout.fillWidth:true;visible:!!root.writeResult.error;text:T3I18n.error(root.writeResult.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
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
    // Reply and review, as in T3's conversation tab.
    ColumnLayout{
     objectName:"pullRequestReply";Layout.fillWidth:true;visible:root.tab===2&&(root.permissions.comment===true||(root.permissions.verdicts || []).length>0);spacing:Style.space(6)
     Controls.TextArea{id:reply;objectName:"pullRequestReplyText";Layout.fillWidth:true;Layout.preferredHeight:Style.space(70);wrapMode:TextEdit.Wrap;placeholderText:T3I18n.tr("Leave a comment");color:root.fg;font.pixelSize:Style.font.caption;enabled:!root.writeResult.busy;background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}}
     RowLayout{Layout.fillWidth:true;spacing:Style.space(6)
      Item{Layout.fillWidth:true}
      T3PickerButton{objectName:"pullRequestComment";visible:root.permissions.comment===true;implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Comment");enabled:!root.writeResult.busy&&!!reply.text.trim();onClicked:root.write({kind:"comment",body:reply.text})}
      T3PickerButton{objectName:"pullRequestRequestChanges";visible:(root.permissions.verdicts || []).indexOf("request-changes")>=0;implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Request changes");enabled:!root.writeResult.busy&&!!reply.text.trim();onClicked:root.write({kind:"review",verdict:"request-changes",body:reply.text})}
      T3PickerButton{objectName:"pullRequestApprove";visible:(root.permissions.verdicts || []).indexOf("approve")>=0;implicitHeight:Style.space(30);text:T3I18n.tr("Approve");enabled:!root.writeResult.busy;onClicked:root.write({kind:"review",verdict:"approve",body:reply.text})}
     }
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
 Controls.Popup {
  id:filterPopup;width:Math.min(root.width-Style.space(24),Style.space(360));height:Style.space(340);anchors.centerIn:parent;modal:true
  ColumnLayout{
   anchors.fill:parent
   T3TextField{id:filterSearch;Layout.fillWidth:true;placeholderText:T3I18n.tr("Search");onTextChanged:filterTimer.restart()}
   Timer{id:filterTimer;interval:250;onTriggered:root.bridge.requestPullRequests("filter-options",{environment:root.environment,kind:root.filterKind,query:filterSearch.text})}
   Text{Layout.fillWidth:true;text:(root.bridge.prOperations["filter-options"]||{}).error||"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   ListView{
    Layout.fillWidth:true;Layout.fillHeight:true;clip:true
    model:(((root.bridge.prOperations["filter-options"]||{}).data||{})[root.filterKind])||[]
    delegate:T3PickerButton{required property var modelData;width:ListView.view.width;primary:false;text:modelData.login||modelData.name||"";onClicked:{if(root.filterKind==="labels")root.labelFilter=modelData.name;else root.authorFilter=modelData.login;filterPopup.close();root.loadList(false)}}
   }
  }
 }
 // Reviewers or labels to toggle, from the host's candidates.
 Controls.Popup{
  id:people;objectName:"pullRequestPeople";property string mode:"reviewers";property var candidates:[]
  function show(m){mode=m;candidates=[];open();root.write({kind:m==="labels"?"label-candidates":"reviewer-candidates"})}
  parent:root;modal:true;focus:true;x:Math.round((root.width-width)/2);y:Math.round((root.height-height)/3);width:Math.min(Style.space(360),root.width-Style.space(30));height:Math.min(Style.space(420),peopleCol.implicitHeight+padding*2);padding:Style.space(12)
  background:Rectangle{radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
  ColumnLayout{id:peopleCol;anchors.fill:parent;spacing:Style.space(6)
   Text{text:T3I18n.tr(people.mode==="labels"?"Labels":"Reviewers");color:root.fg;font.weight:Font.DemiBold;font.pixelSize:Style.font.body}
   WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!people.candidates.length&&!!root.writeResult.busy;active:visible}
   ListView{id:peopleList;Layout.fillWidth:true;Layout.fillHeight:true;implicitHeight:Math.min(Style.space(340),contentHeight);clip:true;model:people.candidates
    T3Wheel{flickable:peopleList}
    delegate:RowLayout{required property var modelData;width:peopleList.width;height:Style.space(30);spacing:Style.space(8)
     T3Switch{checked:people.mode==="labels"?!!modelData.isApplied:!!modelData.isRequested;enabled:!root.writeResult.busy;Accessible.name:modelData.login||modelData.name;onToggled:{if(people.mode==="labels")root.write({kind:"labels",labels:[modelData.name],applied:checked});else root.write({kind:"reviewers",reviewers:[{id:modelData.id,kind:modelData.kind}],requested:checked})}}
     Rectangle{visible:people.mode==="labels";implicitWidth:Style.space(10);implicitHeight:implicitWidth;radius:width/2;color:modelData.color?("#"+String(modelData.color).replace(/^#/,"")):root.dim}
     Text{Layout.fillWidth:true;text:people.mode==="labels"?modelData.name:(modelData.login+(modelData.name?" · "+modelData.name:""));color:root.fg;elide:Text.ElideRight;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
    }
   }
  }
 }
}
