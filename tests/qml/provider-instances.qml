import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PROVIDER_INSTANCES_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:backend
  property string selectedEnvironment:"local"
  property var state:({environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Remote",state:"ready"}]})
  property var viewState:({})
  property var catalogs:({local:{providers:[],models:[],favorites:[]},remote:{providers:[],models:[],favorites:[]}})
  property var settingsResult:({busy:false,error:"",environment:"local",fields:[]})
  property var providerInstances:({busy:false,error:"",environment:"",instances:[],drivers:[],writable:false})
  property var requests:[]
  function requestSettings(){}
  function refreshCatalogIn(){}
  function requestProviderInstances(environment,action,values){
   requests=requests.concat([{environment:environment,action:action,values:values || {}}])
   if(action==="read")providerInstances={busy:false,error:"",environment:environment,writable:environment==="local",instances:[{id:"claudeAgent",driver:"claudeAgent",label:"Claude",isDefault:true,enabled:true,config:{},environment:[]}],drivers:[
    {id:"codex",label:"Codex",fields:[{key:"binaryPath",label:"Binary path"},{key:"homePath",label:"CODEX_HOME path"}]},
    {id:"claudeAgent",label:"Claude",fields:[{key:"binaryPath",label:"Binary path"},{key:"homePath",label:"CLAUDE_CONFIG_DIR path"}]},
    {id:"opencode",label:"OpenCode",fields:[{key:"binaryPath",label:"Binary path"},{key:"serverUrl",label:"Server URL"}]},
    {id:"acpRegistry",label:"ACP Registry",fields:[{key:"source",label:"ACP source",control:"select",options:[{id:"registry",name:"Registry"},{id:"local",name:"Local"}]},{key:"commandPath",label:"Executable override"}]}
   ]}
   else providerInstances=Object.assign({},providerInstances,{busy:false,error:""})
  }
 }
 FloatingWindow{id:win;visible:true;implicitWidth:1050;implicitHeight:790
  Test.T3Preferences{id:prefs;width:win.width;height:win.height;bridge:backend;page:"providers"}
 }
 TestCase{id:finder;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){check(backend.requests.some(function(r){return r.action==="read"&&r.environment==="local"}),"reads selected host");var widget=finder.findChild(prefs,"providerInstances");check(widget!==null,"generic provider editor mounted");check(widget.drivers.length===4,"driver catalog reaches native UI");widget.add();phase=1}
  else if(phase===1){var widget=finder.findChild(prefs,"providerInstances");check(widget.step===0,"driver selection step");widget.chooseDriver("opencode");check(widget.driver==="opencode","non-Claude driver chosen");widget.step=1;var label=finder.findChild(prefs,"providerLabel");label.text="OpenCode Work";widget.step=2;widget.setConfigValue("serverUrl","http://127.0.0.1:4096");widget.submit();phase=2}
  else if(phase===2){var creates=backend.requests.filter(function(r){return r.action==="create"});check(creates.length===1,"one generic create");check(creates[0].environment==="local"&&creates[0].values.driver==="opencode"&&creates[0].values.config.serverUrl==="http://127.0.0.1:4096","origin, driver and typed config captured");prefs.selectEnvironment("remote");phase=3}
  else if(phase===3){var widget=finder.findChild(prefs,"providerInstances");check(backend.requests.some(function(r){return r.environment==="remote"&&r.action==="read"}),"remote read");check(!widget.writable,"remote read-only permission");var count=backend.requests.length;widget.add();widget.submit();check(backend.requests.length===count,"read-only can't create");console.log("PROVIDER_INSTANCES_QML_PASS: generic driver wizard, non-Claude creation, per-host isolation and read-only gate");Qt.quit()}
 }}
}
