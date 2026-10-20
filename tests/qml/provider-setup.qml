import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PROVIDER_SETUP_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{
  id:fixture
  property var providerInstances:({environment:"local",writable:true,busy:false,error:"",drivers:[],instances:[{id:"claude",driver:"claudeAgent",label:"Claude",enabled:true,installed:false,canInstall:true,canAuthenticate:true,authStatus:"unauthenticated"},{id:"codex",driver:"codex",label:"Codex",enabled:true,installed:true,canAuthenticate:true,canLogout:true,authStatus:"authenticated"}]})
  property var providerSetup:({})
  property var calls:[]
  function requestProviderInstances(){}
  function providerSetupAction(environment,id,values){calls=calls.concat([{id:id,values:values}]);var all=Object.assign({},providerSetup);all[id]={busy:false,kind:values.action==="install"?"install":"auth",state:values.action==="install"?{phase:"downloading",downloadedBytes:50,totalBytes:100,operationId:"op"}:{phase:"waiting",flowId:"f",interaction:{type:"deviceCode",id:"x",url:"https://example.com/device",userCode:"ABCD-1234"}}};providerSetup=all}
 }
 TestCase{id:keys;when:false}
 Window{visible:true;width:900;height:700;color:"black"
  Test.T3ProviderInstances{id:view;anchors.fill:parent;bridge:fixture;environment:"local"}
 }
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  if(phase===0){var install=keys.findChild(view,"providerInstall_claude");check(install&&install.visible,"install offered when missing");check(!keys.findChild(view,"providerSignIn_claude").visible,"no sign-in before install");check(keys.findChild(view,"providerSignOut_codex").visible,"sign out for signed-in providers");install.clicked();check(fixture.calls[0].values.action==="install","install starts");phase=1
  }else if(phase===1){check(view.setupOf("claude").state.phase==="downloading","install progress is tracked");fixture.providerSetupAction("local","codex",{action:"login"});phase=2
  }else if(phase===2){check(view.setupOf("codex").state.interaction.userCode==="ABCD-1234","device code shown");console.log("PROVIDER_SETUP_QML_PASS");Qt.quit()}
 }}
}
