import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test

// Actual settings/picker components with a metadata transport peer. No account,
// credential store, sign-in, or native removal runs in this fixture.
ShellRoot {
 id:root
 property int phase:0
 function check(ok,message){if(!ok){console.error("PROVIDER_AUTH_REMOVE_QML_FAIL "+message);Qt.exit(1);throw new Error(message)}}
 function click(item){check(item&&item.visible,"actual control visible");keys.mouseClick(item,item.width/2,item.height/2,Qt.LeftButton)}
 function method(id,removable){return {id:id,name:"<b>Connection</b>",description:null,type:"agent",canRemove:removable}}
 function state(methods){return {phase:"idle",methods:methods,credentialOwner:"t3"}}
 function settle(methods,error){fixture.providerSetup={codex:{environment:"node",busy:false,kind:"auth",action:"remove-auth-method",error:error||"",state:state(methods)}}}
 QtObject {
  id:fixture
  property var providerInstances:({environment:"node",writable:true,busy:false,error:"",drivers:[],instances:[{id:"codex",driver:"codex",label:"Codex",enabled:true,installed:true,config:{setupMode:"managed"},authStatus:"authenticated",canAuthenticate:true,canLogout:true,isDefault:true}]})
  property var providerSetup:({codex:{environment:"node",busy:false,kind:"auth",state:root.state([root.method("chatgpt-profile:a",true),root.method("chatgpt-profile:b",false)])}})
  property var calls:[]
  property int reads:0
  property var state:({environments:[{id:"node",name:"NODE"}]})
  property var providerUpdates:[]
  function requestProviderInstances(environment,mode){if(mode==="read")reads++}
  function providerSetupAction(environment,id,values){
   calls=calls.concat([{environment:environment,id:id,values:values}])
   if(values.action!=="auth-state"){
    var all=Object.assign({},providerSetup);all[id]=Object.assign({},all[id],{environment:environment,busy:true,error:"",action:values.action});providerSetup=all
   }
  }
 }
 TestCase{id:keys;when:false}
 Window {
  id:window
  visible:true;width:900;height:700;color:"black"
  Test.T3ProviderInstances{id:view;anchors.fill:parent;bridge:fixture;environment:"node"}
 }
 Timer {
  interval:180;repeat:true;running:true
  onTriggered:{
   var picker=keys.findChild(view,"chatGptAccountPicker")
   if(phase===0){
    check(picker,"actual picker exists")
    view.pickAuth(view.instances[0]);phase=1
   }else if(phase===1){
    check(picker.opened&&picker.selectedMethodId==="chatgpt-profile:a","first native profile selected")
    var a=keys.findChild(picker.contentItem,"chatGptProfile_chatgpt-profile:a")
    check(a&&a.contentItem.text==="<b>Connection</b>"&&a.contentItem.textFormat===Text.PlainText,"remote label is plain text")
    var remove=keys.findChild(picker.contentItem,"chatGptRemoveConnection")
    check(remove.visible&&remove.enabled,"only actual removable native method offered")
    click(keys.findChild(picker.contentItem,"chatGptProfile_chatgpt-profile:b"))
    check(!remove.visible,"saved complete connection not removable")
    click(keys.findChild(picker.contentItem,"chatGptDifferentAccount"));picker.close();picker.open()
    check(picker.selectedMethodId==="chatgpt-change-account","selection survives closing")
    click(a);click(remove);phase=2
   }else if(phase===2){
    var call=fixture.calls[fixture.calls.length-1]
    check(call.environment==="node"&&call.id==="codex"&&call.values.action==="remove-auth-method"&&call.values.methodId==="chatgpt-profile:a","removal keeps exact owner and method")
    check(picker.opened&&!keys.findChild(picker.contentItem,"chatGptRemoveConnection").enabled,"removal keeps picker open and waits")
    settle([method("chatgpt-profile:a",true),method("chatgpt-profile:b",false)],"<b>Private fixture refusal</b>");phase=3
   }else if(phase===3){
    var error=keys.findChild(picker.contentItem,"chatGptAccountError")
    check(error.visible&&error.text.indexOf("<b>Private fixture refusal</b>")>=0&&error.textFormat===Text.PlainText,"inline failure is plain text")
    click(keys.findChild(picker.contentItem,"chatGptRemoveConnection"))
    settle([method("chatgpt-profile:b",false)]);phase=4
   }else if(phase===4){
    check(picker.opened&&picker.selectedMethodId==="chatgpt-profile:b"&&!keys.findChild(picker.contentItem,"chatGptRemoveConnection").visible,"successful metadata removes selection without closing or signing in")
    check(fixture.calls.filter(function(call){return call.values.action!=="auth-state"&&call.values.action!=="remove-auth-method"}).length===0,"no implicit login/logout/cancel")
    fixture.providerInstances=Object.assign({},fixture.providerInstances,{writable:false});phase=5
   }else if(phase===5){
    var before=fixture.calls.length
    check(!picker.canManage&&!keys.findChild(picker.contentItem,"chatGptReconnect").enabled,"fresh permission loss disables reconnect")
    click(keys.findChild(picker.contentItem,"chatGptReconnect"))
    check(fixture.calls.length===before,"actual disabled reconnect emits no action")
    fixture.providerInstances=Object.assign({},fixture.providerInstances,{writable:true})
    settle([{id:"chatgpt-profile:absent",name:"Missing flag",description:null,type:"agent"},method("chatgpt-profile:false",false),method("chatgpt-profile:string","true")]);phase=6
   }else if(phase===6){
    check(picker.profiles.length===2&&!keys.findChild(picker.contentItem,"chatGptRemoveConnection").visible,"missing/false flag cannot remove and string flag is rejected")
    click(keys.findChild(picker.contentItem,"chatGptProfile_chatgpt-profile:false"))
    check(!keys.findChild(picker.contentItem,"chatGptRemoveConnection").visible,"explicit false cannot remove")
    settle([method("chatgpt-profile:a",true)])
    var all=Object.assign({},fixture.providerSetup)
    all.codex=Object.assign({},all.codex,{state:{phase:"verifying",methods:[method("chatgpt-profile:a",true)],credentialOwner:"t3"}})
    fixture.providerSetup=all;phase=7
   }else if(phase===7){
    var before=fixture.calls.length
    check(picker.busy&&!keys.findChild(picker.contentItem,"chatGptReconnect").enabled&&!keys.findChild(picker.contentItem,"chatGptRemoveConnection").enabled,"native active flow disables both mutations")
    click(keys.findChild(picker.contentItem,"chatGptReconnect"));click(keys.findChild(picker.contentItem,"chatGptRemoveConnection"))
    check(fixture.calls.length===before,"actual busy controls emit no action")
    settle([method("chatgpt-profile:"+"😀".repeat(50),true)]);phase=8
   }else if(phase===8){
    check(picker.selectedMethodId==="chatgpt-profile:"+"😀".repeat(50),"UTF16-valid profile beyond128UTF8bytes selected exactly")
    click(keys.findChild(picker.contentItem,"chatGptReconnect"));phase=9
   }else if(phase===9){
    var call=fixture.calls[fixture.calls.length-1]
    check(call.environment==="node"&&call.id==="codex"&&call.values.action==="login"&&call.values.methodId==="chatgpt-profile:"+"😀".repeat(50),"reconnect forwards exact issued profile instead of provider default")
    settle([method("chatgpt-profile:a",true)])
    view.pickAuth(view.instances[0]);phase=10
   }else if(phase===10){
    check(picker.opened,"same-owner picker reopens")
    fixture.providerInstances=Object.assign({},fixture.providerInstances,{instances:[{id:"codex_new",driver:"codex",label:"Replacement",enabled:true,installed:true,config:{setupMode:"managed"}}]});phase=11
   }else if(phase===11){
    check(!picker.opened&&picker.selection==="","instance replacement closes and clears old issued choices")
    view.environment="other";phase=12
   }else if(phase===12){
    check(!picker.opened&&picker.selection===""&&picker.profiles.length===0,"environment replacement clears stale scope")
    var before=fixture.calls.length
    // Delayed metadata still belongs to NODE, not the selected environment.
    settle([method("chatgpt-profile:late",true)]);phase=13
   }else if(phase===13){
    check(!picker.opened&&!picker.canManage&&picker.profiles.length===0,"late metadata cannot resurrect the previous owner")
    console.log("PROVIDER_AUTH_REMOVE_QML_PASS");Qt.quit()
   }
  }
 }
}
