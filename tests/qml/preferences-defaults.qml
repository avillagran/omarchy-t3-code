import QtQuick
import QtQuick.Layouts
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,message){if(!ok){console.error("PREFERENCES_DEFAULTS_QML_FAIL: "+message);Qt.exit(1)}}
 property var fields:[
  {key:"defaultModelSelection",label:"Model",kind:"model",value:null,resetValue:null,description:"Default model for new threads. Projects can override it."},
  {key:"defaultRuntimeMode",label:"Permissions",kind:"choice",value:"full-access",resetValue:"full-access",choices:[{id:"approval-required",name:"Supervised"},{id:"full-access",name:"Full access"}]},
  {key:"defaultThreadEnvMode",label:"Workspace",kind:"choice",value:null,resetValue:null,choices:[{id:null,name:"Automatic"},{id:"local",name:"Current checkout"},{id:"worktree",name:"New worktree"}]}
 ]
 QtObject{id:b
  property string selectedEnvironment:"local"
  property var viewState:({})
  property var state:({environments:[{id:"local",name:"Local",state:"ready"},{id:"node",name:"Node",state:"ready"}]})
  property var catalogs:({local:{favorites:[],favoritesReady:true,providers:[{instanceId:"work",name:"Work",ready:true}],models:[{instanceId:"work",model:"one",displayName:"One",available:true},{instanceId:"work",model:"two",displayName:"Two",available:true,descriptors:[{id:"effort",label:"Reasoning",type:"select",options:[{id:"high",label:"High"}]}]}]}})
  property var settingsResult:({busy:false,error:"",environment:"local",fields:root.fields})
  property var sent:({});property int serial:0
  function serverVersion(environment){return ""}
  function requestSettings(environment,key,value){sent={environment:environment,key:key,value:value};settingsResult={environment:environment,requestId:String(++serial),busy:true,error:"",fields:settingsResult.environment===environment?settingsResult.fields:[]}}
  function reply(){var cmd=sent;root.fields=root.fields.map(function(f){return f.key===cmd.key?Object.assign({},f,{value:cmd.value}):f});settingsResult={environment:cmd.environment,requestId:settingsResult.requestId,busy:false,error:"",fields:root.fields}}
  function refreshCatalogIn(environment){}
  function rememberView(value){}
  function toggleModelFavoriteIn(environment,instance,model){}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:820;Test.T3Preferences{id:view;anchors.fill:parent;bridge:b}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  if(phase===0){b.reply();phase=1
  }else if(phase===1){var picker=keys.findChild(view,"preferenceModel_defaultModelSelection");check(picker&&picker.enabled,"native default model picker is reachable");check(keys.findChild(view,"preferenceDescription_defaultModelSelection").visible,"native row description rendered");picker.popup.open();phase=2
  }else if(phase===2){keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Return);check(b.sent.key==="defaultModelSelection"&&b.sent.value.instanceId==="work"&&b.sent.value.model==="two","keyboard model choice saves the exact provider instance without editing a chat draft");check(b.settingsResult.busy,"preference waits for native acknowledgement");b.reply();phase=3
  }else if(phase===3){keys.findChild(view,"preferenceModelOptions_defaultModelSelection").clicked();phase=4
  }else if(phase===4){var field=keys.findChild(view,"preferenceField_defaultModelSelection");check(field.optionsPopup.opened,"options popup is open");check(field.selectedModel.model==="two","options belong to chosen model");var option=keys.findChild(field.optionsPopup.contentItem,"preferenceOption_defaultModelSelection_effort");if(!option){check(false,"model options popup contains the current model descriptor");return}option.activated(0);check(b.sent.value.model==="two"&&b.sent.value.options[0].id==="effort"&&b.sent.value.options[0].value==="high","same shared option control preserves model identity");b.reply();phase=5
  }else if(phase===5){keys.findChild(view,"preferenceChoice_defaultRuntimeMode").activated(0);check(b.sent.key==="defaultRuntimeMode"&&b.sent.value==="approval-required","permissions use native typed value");b.reply();phase=6
  }else if(phase===6){var reset=keys.findChild(view,"resetPreference_defaultRuntimeMode");check(reset.visible&&reset.enabled,"native default reset is reachable");reset.clicked();check(b.sent.value==="full-access","reset uses native built-in default");b.reply();phase=7
  }else if(phase===7){keys.findChild(view,"preferenceChoice_defaultThreadEnvMode").activated(2);check(b.sent.key==="defaultThreadEnvMode"&&b.sent.value==="worktree","workspace preference uses native mode");b.reply();phase=8
  }else if(phase===8){view.environment="node";check(b.sent.environment==="node"&&b.settingsResult.fields.length===0,"scope switch clears prior fields");view.query="Default model";check(view.groups.length===0,"stale scope fields are not searched");console.log("PREFERENCES_DEFAULTS_QML_PASS: native default model/options, typed permissions/workspace, reset and environment isolation");Qt.quit()}
 }}
}
