import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/DeviceSettings.js" as Devices
import "UnderTest/ui/js/NativeDeviceSetup.js" as Native
ShellRoot{
 id:root;property int phase:0;property bool captured:false
 function check(v,m){if(!v){console.error("INTEGRATIONS_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 QtObject{id:b
  property var clientSettings:({data:{},busy:false,error:""})
  function requestClientSettings(p){}
  property var browserProfileView:({busy:false,error:"",data:null})
  property var browserProfileOperation:({state:"",results:[]})
  function requestBrowserProfiles(t,f){}
  function dismissBrowserProfiles(){}
  function browserProfileAction(a,p){}
  function acknowledgeBrowserProfile(){}
  property bool panelVisible:true
  property bool deviceWorkerReady:true
  property var deviceView:({signature:"",targets:[],sessions:{},error:""})
  property var deviceOperations:({})
  property var deviceCommands:({})
  property var deviceLiveStates:({})
  property string deviceWatchRequest:""
  property var sent:[];property int serial:0
  function uuid(){return "fixture-"+(++serial)}
  function send(c){sent=sent.concat([JSON.parse(JSON.stringify(c))])}
  function requestDevices(t,f){Devices.select(b,t,f)}
  function dismissDevices(){Devices.dismiss(b)}
  function deviceAllowed(e,a){return Devices.allowed(b,e,a)}
  function deviceAction(a,i,e){return Devices.dispatch(b,a,i,e)}
  function acknowledgeDevice(e){Devices.acknowledge(b,e)}
  function refreshDevice(e){Devices.read(b,e)}
 }
 property var targets:[{environment:"one",projectId:null},{environment:"two",projectId:null}]
 function fixture(env,can){return {environment:env,originKey:"origin-"+env,viewId:b.uuid(),permissions:{configure:can,operate:can},settings:{enableDeviceSupport:false,enableAgentDeviceAccess:false,deviceHosts:[]},state:{hostStatuses:{},revision:0,hostStatus:"disabled",hosts:[{id:"local",kind:"local",label:"Local",platforms:[{platform:"android",available:false,reason:"SDK missing"}],tools:{hub:{requiredVersion:"1",installedVersions:[],runningVersion:null},agent:{requiredVersion:"2",installedVersions:[],runningVersion:null}}}],devices:[],sessions:[],supportsToolInspection:true,supportsToolUpdate:true,supportsHostRetry:true}}}
 function reply(e,can){var s=b.deviceView.sessions[e];Devices.receipt(b,{op:"devices-read",environment:e,requestId:s.requestId,result:fixture(e,can)})}
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:1000;implicitHeight:900;Test.T3Integrations{id:page;anchors.fill:parent;anchors.margins:24;bridge:b;targets:root.targets;settingsWritable:true;settings:({fields:[{key:"enableAgentBrowserAccess",kind:"bool",label:"Agent browser access",value:true},{key:"enableAgentDeviceAccess",kind:"bool",label:"Agent device access",value:true,projectScoped:true,source:"project",scopeWritable:true}]})}}
 Timer{interval:120;repeat:true;running:true;onTriggered:{
  if(phase===0){check(b.sent.filter(function(c){return c.op==="devices-read"}).length===2,"mount reads exact captured environments");check(!b.sent.some(function(c){return c.op==="devices-action"}),"observation cannot start tools");reply("one",true);reply("two",false);phase=1}
  else if(phase===1){check(page.scopeAllowed("hub"),"native representative permission permits per-environment partial results");var one=b.deviceView.sessions.one.data,watch=b.deviceWatchRequest;Devices.receiveState(b,{environment:"one",requestId:"stale",originKey:one.originKey,state:Object.assign({},one.state,{revision:9,hostStatus:"installing"})});check(b.deviceView.sessions.one.data.state.revision===0,"stale watcher ignored: "+JSON.stringify(b.deviceView.sessions.one.data.state));Devices.receiveState(b,{environment:"one",requestId:watch,originKey:one.originKey,state:Object.assign({},one.state,{revision:2,hostStatus:"ready"})});check(b.deviceView.sessions.one.data.state.revision===2,"native progress applied by nonce/origin");Devices.receiveState(b,{environment:"one",requestId:watch,originKey:one.originKey,state:one.state});check(b.deviceView.sessions.one.data.state.revision===2,"older revision ignored");check(!Native.nativeDeviceSetup.platformSetupStatus(one.state,"android").ready,"exact original missing-SDK status");b.deviceAction("hub",{enabled:true});var cmds=Object.keys(b.deviceCommands);check(cmds.length===1&&b.deviceOperations.two.state==="failed","native partial-target dispatch captured");var command=b.deviceCommands[cmds[0]].command;Devices.receipt(b,{op:"devices-action",environment:"wrong",requestId:command.requestId,result:{state:"complete"}});check(!!b.deviceCommands[command.requestId],"wrong-environment result rejected");Devices.receipt(b,{op:"devices-action",environment:"one",requestId:command.requestId,result:{state:"uncertain",error:"device_outcome_unknown"}});check(b.deviceOperations.one.state==="uncertain"&&!b.deviceAllowed("one","hub"),"uncertainty blocks accidental replay");reply("one",true);check(!b.deviceAllowed("one","hub"),"passive read does not erase unknown outcome");b.acknowledgeDevice("one");reply("one",true);check(b.deviceAllowed("one","hub"),"explicit acknowledgement plus fresh view required");root.targets=[{environment:"one",projectId:"project"}];phase=2}
  else if(phase===2){reply("one",true);check(page.projectScope&&!page.scopeAllowed("hub")&&page.scopeAllowed("inspect"),"project cannot configure hub but native version maintenance remains");check(page.agentField().projectScoped,"project agent access uses shared scoped field");var old=b.deviceWatchRequest;root.targets=[{environment:"one",projectId:null}];Devices.receiveState(b,{environment:"one",requestId:old,disconnected:true});check(!b.deviceView.sessions.one.error,"scope transition ignores old disconnect");reply("one",true);b.deviceAction("agent",{enabled:true});check(Object.keys(b.deviceCommands).length===1,"agent configure captured");Devices.lost(b);check(b.deviceOperations.one.state==="uncertain"&&!Object.keys(b.deviceCommands).length,"worker loss never resends setup");page.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-integrations-fixture.png")});phase=3}
  else if(phase===3){if(!captured)return;b.acknowledgeDevice("one");reply("one",true);page.editHost(null);phase=4}
  else if(phase===4){var editor=keys.findChild(page,"deviceHostEditor"),popup=keys.findChild(page,"deviceHostEditorPopup");check(editor&&popup.opened,"shared native-style host editor");editor.target="remote-alias";check(!editor.valid&&editor.parse(true).label==="remote-alias","probe accepts target fallback while Save requires name");editor.label="Mac mini";editor.port="70000";check(!editor.valid,"native port bounds");editor.port="2222";editor.identity="  ~/.ssh/device_key  ";check(editor.draft.identityFile==="~/.ssh/device_key"&&editor.draft.port===2222,"SSH options normalized as server-local config");editor.check();var id=Object.keys(b.deviceCommands)[0],c=b.deviceCommands[id].command;check(c.action==="test-host"&&c.host.target==="remote-alias"&&c.host.port===2222,"native per-environment test payload");Devices.receipt(b,{op:"devices-action",environment:"one",requestId:id,result:{state:"complete",host:{kind:"local"},data:fixture("one",true)}});reply("one",true);check(b.deviceOperations.one.host.kind==="local","same-machine result preserved");editor.save();var id=Object.keys(b.deviceCommands)[0],c=b.deviceCommands[id].command;check(c.action==="host-upsert"&&c.original.id===c.host.id,"host save uses exact captured original");Devices.receipt(b,{op:"devices-action",environment:"one",requestId:id,error:"permission_denied"});reply("one",true);check(popup.opened&&editor.label==="Mac mini","failed host save retains complete editor draft");editor.save();var id=Object.keys(b.deviceCommands)[0];Devices.receipt(b,{op:"devices-action",environment:"one",requestId:id,result:{state:"complete",data:fixture("one",true)}});reply("one",true);check(!popup.opened,"host editor closes only after every selected environment succeeds");phase=5}
  else if(phase===5){var count=b.sent.length;b.dismissDevices();Devices.receiveState(b,{environment:"one",requestId:"old",state:{revision:99}});check(!Object.keys(b.deviceView.sessions).length&&b.sent.length===count+1&&b.sent[b.sent.length-1].environments.length===0,"leaving page cancels subscription without mutating tools");console.log("INTEGRATIONS_QML_PASS: native helper, scoped device screen, exact targets/roles, captured receipts, progress nonces/revisions, uncertain outcomes and cancellation");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("INTEGRATIONS_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
