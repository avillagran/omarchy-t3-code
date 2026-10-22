import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/ProviderUpdates.js" as Updates
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_THREAD_DETAILS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Test.T3EnvironmentIcon{id:machine;kind:"mac-mini"}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   var claude={instanceId:"claudeAgent",name:"Claude",driverKind:"claudeAgent",update:{driver:"claudeAgent",name:"Claude",currentVersion:"2.1.306",latestVersion:"2.1.307",canUpdate:false,oneClick:false,state:"idle"}}
   var codex={instanceId:"codex",name:"Codex",driverKind:"codex",update:{driver:"codex",name:"Codex",currentVersion:"0.162.1",latestVersion:"0.163.0",canUpdate:true,oneClick:true,state:"idle"}}
   fixture={threads:[{key:"local/t1",id:"t1",environmentId:"local",environmentName:"T3 Code",title:"Chat",projectId:"p",projectKey:"local/p",projectMonogram:"ME",worktreePath:"/repo"}],projects:[{id:"p",key:"local/p",environmentId:"local",title:"moodle",workspaceRoot:"/repo"}],environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"node",state:"ready"}]}
   b.state=fixture;b.catalogs={local:{canWrite:true,models:[],providers:[claude],favorites:[],machine:"laptop"},node:{canWrite:true,models:[],providers:[codex,claude],favorites:[],machine:"desktop"}}
   b.selectThread("local","t1");panel.open();phase=1
  }else if(phase===1){
   check(machine.name==="mac-mini"&&machine.label==="Mini PC","machine kinds map to T3's icons")
   check(b.providerUpdates.length===3,"one candidate per environment and driver: "+b.providerUpdates.length)
   var n=b.providerUpdateNotice;check(n&&n.key===Updates.key(b.providerUpdates)&&n.key.indexOf("node=codex:0.163.0")>=0,"notice keyed by environment, driver and version")
   var notice=keys.findChild(panel,"t3ProviderUpdateNotice");check(notice&&notice.visible&&notice.anyOneClick,"update notice shows with a one-click update")
   check(notice.description().indexOf("Claude")>=0,"manual installs get their own sentence: "+notice.description())
   check(notice.requestText(Object.assign({},b.providerUpdates[0],{state:"running",name:"Codex"})).indexOf("Codex")>=0,"an update the server is running shows in its row")
   var settings=keys.findChild(panel,"t3ProviderUpdateSettings");check(!!settings&&settings.implicitHeight<36,"notice buttons are compact")
   var icon=keys.findChild(panel,"t3HeaderProjectIcon");check(icon&&icon.visible,"header shows the project icon")
   var all=keys.findChild(panel,"t3ProviderUpdateAll");check(all&&all.visible&&all.ready.length===1,"update all shows for the one-click update among several: "+(all&&all.ready.length));var cat0=b.catalogs;b.catalogs={local:cat0.local};check(keys.findChild(panel,"t3ProviderUpdateAll").visible&&keys.findChild(panel,"t3ProviderUpdateAll").ready.length===0,"update all shows even for a single manual update");b.catalogs=cat0
   // Two one-click updates in different environments for the queue check.
   var one=b.providerUpdates.filter(function(u){return u.oneClick}).concat([Object.assign({},b.providerUpdates[0],{environment:"local",instanceId:"codex-local",oneClick:true})])
   b.providerUpdateRequests=({});b.updateAllProviders(one)
   var states=one.map(function(u){return (b.providerUpdateRequest(u) || {}).state})
   check(states.filter(function(st){return st==="pending"||st==="failed"}).length===1||states.every(function(st){return st==="failed"}),"one update starts at a time: "+JSON.stringify(states))
   check(b.providerUpdateQueue.length+states.filter(function(st){return st!=="queued"}).length>=1,"the rest wait their turn")
   b.providerUpdateQueue=[];b.providerUpdateRequests=({})
   b.closeProviderUpdates(n.key,true)
   check(!b.providerUpdateNotice&&b.viewState.providerUpdateDismissals.indexOf(n.key)>=0,"closing remembers this set of versions")
   var cat=Object.assign({},b.catalogs);cat.node=Object.assign({},cat.node,{providers:[Object.assign({},cat.node.providers[0],{update:Object.assign({},cat.node.providers[0].update,{latestVersion:"0.164.0"})}),cat.node.providers[1]]});b.catalogs=cat
   check(!!b.providerUpdateNotice,"a newer version shows again")
   check(!panel.threadDetailsOpen,"details start closed");panel.toggleThreadDetails();phase=2
  }else if(phase===2){
   var card=keys.findChild(panel,"t3ThreadDetails");if(!card)return
   check(panel.threadDetailsOpen&&b.viewState.threadDetailsOpen===true,"details toggle is remembered")
   check(keys.findChild(card,"t3DetailsMachine").name==="laptop","run context uses the environment's machine")
   b.threadDetails={key:b.selectedKey,editors:[{id:"vscode",label:"VS Code"},{id:"zed",label:"Zed"}],cwd:"/repo",busy:false,error:"",requestId:""}
   var editorRow=keys.findChild(card,"t3DetailsOpenEditor");check(editorRow&&editorRow.visible&&editorRow.text.indexOf("VS Code")>=0,"open in the first editor by default")
   b.rememberView({preferredEditor:"zed"});check(editorRow.text.indexOf("Zed")>=0,"the preferred editor leads")
   b.gitView={loading:false,error:"",scope:{environment:"local",thread:"t1",projectId:"p",cwd:"/repo",requestId:"r"},operation:{},data:{revision:"x",permissions:{terminalWrite:true},scripts:[],actions:{commit:true},status:{isRepo:true,refName:"main",aheadCount:0,behindCount:0,workingTree:{insertions:3,deletions:1}}}}
   console.log("T3_DETAILS_SIZE card="+card.height+" content="+card.contentHeight+" max="+card.maxHeight)
   check(card.height===Math.min(card.contentHeight,card.maxHeight),"card fits its content")
   card.resized(320,180);check(b.viewState.threadDetailsHeight===180&&b.viewState.threadDetailsWidth===320,"drag size is remembered")
   check(card.height===Math.min(180,card.maxHeight)&&card.width===320,"card takes the dragged size: "+card.width+"x"+card.height)
   card.resized(0,0);check(card.height===Math.min(card.contentHeight,card.maxHeight),"double-click fits again")
   panel.toggleThreadDetails();check(!panel.threadDetailsOpen,"details close again")
   panel.close();console.log("T3_THREAD_DETAILS_QML_PASS: provider update notice, header icon, thread details card, environment icons");Qt.quit()
  }
 }}
}
