import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/BrowserImport.js" as Flow
ShellRoot{
 id:root
 property var popup:null
 property int phase:0
 property string stableProfile:""
 property bool captured:false
 property var source:({id:"firefox",name:"Firefox",profiles:[{directory:"a",name:"Personal",cookieCount:5},{directory:"b",name:"Work",cookieCount:2}]})
 function check(v,m){if(!v){console.error("BROWSER_IMPORT_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 function ready(){var v=fake.browserImportView;Flow.receipt(fake,{op:"browser-import-read",requestId:v.requestId,result:{viewId:"view-"+fake.counter,environment:"primary",environmentName:"Primary",sources:[root.source],profiles:[{id:"default",name:"Default"},{id:"work",name:"Work"}],canCreateProfile:true}})}
 QtObject{id:fake
  property bool deviceWorkerReady:true
  property var state:({primaryEnvironmentId:"primary",environments:[{id:"primary",name:"Primary",state:"ready"}]})
  property var clientSettings:({busy:false,error:"",data:{revision:"fixture",browserProfiles:[]}})
  property var browserProfileView:({busy:false,error:"",data:{available:true}})
  function requestBrowserProfiles(targets,force){}
  function dismissBrowserProfiles(){}
  function requestClientSettings(patch){}
  function requestBrowserImportSources(){Flow.loadSources(fake)}
  function openBrowserImport(source){return Flow.open(fake,source)}
  property var browserProfileOperation:({state:""})
  property bool browserProfileNeedsRefresh:false
  property var browserImportCatalog:({busy:false,error:"",requestId:"",sources:[]})
  property var browserImportView:({environment:"",requestId:"",busy:false,error:"",data:null})
  property var browserImportOperation:({state:"",error:""})
  property var browserImportWizard:({open:false,step:{step:"configure"}})
  property var commands:[]
  property int counter:0
  function uuid(){return "00000000-0000-4000-8000-"+String(++counter).padStart(12,"0")}
  function send(v){commands=commands.concat([v])}
  function chooseBrowserImportSource(directory){Flow.chooseSource(fake,directory)}
  function chooseBrowserImportTarget(kind,id){Flow.chooseTarget(fake,kind,id)}
  function refreshBrowserImport(retry){return Flow.refresh(fake,retry)}
  function runBrowserImport(){return Flow.dispatch(fake)}
  function closeBrowserImport(ack){return Flow.close(fake,ack)}
 }
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:900;implicitHeight:800;Rectangle{id:surface;anchors.fill:parent;color:"#101214";Test.T3BrowserProfiles{id:page;anchors.left:parent.left;anchors.right:parent.right;bridge:fake;targets:[{environment:"secondary",projectId:"project"}]}}}
 Timer{interval:100;running:true;repeat:true;onTriggered:{
  if(phase===0){root.popup=keys.findChild(page,"browserImportWizard");Flow.loadSources(fake);var req=fake.commands[fake.commands.length-1];Flow.receipt(fake,{op:"browser-import-sources",requestId:req.requestId,result:[root.source,{id:"chrome",name:"Chrome",profiles:[],unavailable:"notInstalled"},{id:"safari",name:"Safari",profiles:[],unavailable:"unsupportedPlatform"}]});check(page.importSources.length===1,"native menu hides unavailable platform/uninstalled sources");keys.findChild(page,"browserProfileAdd").clicked();phase=10}
  else if(phase===10){var menu=keys.findChild(page,"browserProfileAddMenu"),row=keys.findChild(page,"browserImportMenu_firefox");if(!menu.opened||!row)return;check(fake.commands[fake.commands.length-1].op==="browser-import-sources","Add profile refetches source metadata on open");check(row.enabled,"connected primary enables source row");row.triggered();menu.close();check(fake.browserImportWizard.open&&fake.browserImportWizard.environment==="primary","native source menu captures primary independently of secondary project scope");root.stableProfile=fake.browserImportWizard.newProfileId;ready();phase=1}
  else if(phase===1){if(!popup.opened)return;check(popup.phase==="configure"&&popup.targets.length===3,"original from/into layout and targets load");check(Math.abs(keys.findChild(popup.contentItem,"browserImportSource_0").width-keys.findChild(popup.contentItem,"browserImportTarget_new").width)<3,"original equal-width From/Into columns");keys.mouseClick(keys.findChild(popup.contentItem,"browserImportSource_1"));check(fake.browserImportWizard.directory==="b","real source tile chooses source profile");keys.mouseClick(keys.findChild(popup.contentItem,"browserImportTarget_work"));check(fake.browserImportWizard.target.profileId==="work","real existing target tile selects existing profile");keys.mouseClick(keys.findChild(popup.contentItem,"browserImportTarget_new"));check(fake.browserImportWizard.target.kind==="new","new target remains selectable");popup.contentItem.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-browser-import-fixture.png")});phase=2}
  else if(phase===2){if(!captured)return;keys.findChild(popup.contentItem,"browserImportContinue").clicked();check(fake.browserImportOperation.state==="pending"&&popup.pending,"import click enters pending before dispatch");var req=fake.commands[fake.commands.length-1];check(req.op==="browser-import-action"&&req.confirmed&&req.environment==="primary"&&req.sourceProfileDirectory==="b"&&req.target.profileId===stableProfile,"exact captured destination/source and explicit import");check(!req.cookie&&!req.value,"UI command never carries cookie values");check(!Flow.close(fake,false)&&!Flow.dispatch(fake),"pending cannot close or replay");keys.keyClick(Qt.Key_Escape);keys.mouseClick(surface,10,10);check(popup.opened&&fake.browserImportWizard.open,"native modal ignores Escape and outside clicks during import");Flow.receipt(fake,{op:req.op,requestId:"stale",result:{}});check(fake.browserImportOperation.state==="pending","stale receipt ignored");Flow.receipt(fake,{op:req.op,requestId:req.requestId,result:{kind:"blocked",state:"failed",environment:"primary",profileId:stableProfile,reason:"readFailed",imported:0,skipped:0,skippedDomains:[]}});check(popup.phase==="blocked","known failure uses original blocked step");phase=3}
  else if(phase===3){keys.findChild(popup.contentItem,"browserImportContinue").clicked();check(popup.phase==="checking"&&fake.browserImportView.busy,"retry refreshes one-use native view");ready();check(fake.browserImportOperation.state==="pending"&&fake.browserImportWizard.newProfileId===stableProfile,"retry preserves original new target id and dispatches only after refreshed view");var req=fake.commands[fake.commands.length-1];Flow.receipt(fake,{op:req.op,requestId:req.requestId,result:{kind:"imported",state:"complete",environment:"primary",profileId:stableProfile,imported:2,skipped:1,skippedDomains:["example.test"],targetName:"Firefox"}});check(popup.phase==="done"&&fake.browserProfileNeedsRefresh,"completion displays counts and refreshes client metadata");phase=4}
  else if(phase===4){keys.findChild(popup.contentItem,"browserImportContinue").clicked();check(!fake.browserImportWizard.open,"done closes dialog");
   Flow.open(fake,root.source);ready();Flow.chooseSource(fake,"b");Flow.dispatch(fake);var failed=fake.browserImportOperation;Flow.receipt(fake,{op:"browser-import-action",requestId:failed.requestId,result:{kind:"blocked",state:"failed",environment:"primary",profileId:failed.profileId,reason:"readFailed",imported:0,skipped:0,skippedDomains:[]}});Flow.refresh(fake,true);var fresh=fake.browserImportView;Flow.receipt(fake,{op:"browser-import-read",requestId:fresh.requestId,result:{viewId:"changed-source",environment:"primary",sources:[{id:"firefox",name:"Firefox",profiles:[root.source.profiles[0]]}],profiles:[{id:"default",name:"Default"}],canCreateProfile:true}});check(fake.browserImportWizard.step.reason==="unknownSourceProfile"&&fake.commands[fake.commands.length-1].op==="browser-import-read","retry cannot silently import a different source profile if the selected directory disappears");Flow.close(fake,false);
   Flow.open(fake,root.source);ready();popup.width=340;keys.wait(30);check(keys.findChild(popup.contentItem,"browserImportChoices").columns===1,"narrow dialog stacks original From/Into choices");Flow.dispatch(fake);Flow.lost(fake);check(fake.browserImportOperation.state==="uncertain"&&popup.phase==="blocked","worker loss preserves unknown outcome");check(!Flow.open(fake,root.source),"unknown outcome blocks new imports");check(Flow.close(fake,true)&&fake.browserImportOperation.state==="","explicit dismiss acknowledges unknown result");fake.state={primaryEnvironmentId:null,environments:[{id:"primary",state:"ready"}]};check(!Flow.open(fake,root.source),"missing primary never silently uses selected scope");console.log("BROWSER_IMPORT_QML_PASS: native tiles/modal, original transitions, captured source/primary target, stable retry, no UI cookie values, completion and unknown/no-replay handling");Qt.quit()}
 }}
 Timer{interval:7000;running:true;onTriggered:{console.error("BROWSER_IMPORT_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
