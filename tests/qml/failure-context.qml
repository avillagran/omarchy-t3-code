import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_FAILURE_CONTEXT_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 FloatingWindow{visible:true;implicitWidth:640;implicitHeight:600
  Column{width:600
   Test.T3RunError{id:card;width:parent.width;bridge:Test.T3Backend;failure:({})}
   Test.T3ActivityBlock{id:row;bridge:Test.T3Backend;activity:({})}
  }}
 Timer{interval:150;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   b.selectedEnvironment="node";b.selectedThread="t1";b.selectedKey="node/t1"
   var ctx={};ctx["node/t1/run-2"]={busy:false,error:"",tools:[{itemId:"i1",type:"dynamic_tool",status:"failed",failed:true,label:"t3-code.preview_open",toolName:"t3-code.preview_open",input:"url https://demorv.relasco.cl",detail:"Preview automation open failed: AppArmor blocks the sandbox."}]};b.failureContexts=ctx
   var ref={sourceThreadId:"t1",sourceItemId:"i1",type:"dynamic_tool",runId:"run-2",providerTurnId:null,revision:"2026-10-21T22:58:58.464Z"}
   var tf={};tf["node/i1/"+ref.revision]={busy:false,error:"",failed:true,input:"url https://demorv.relasco.cl",detail:"Preview automation open failed: AppArmor blocks the sandbox."};b.toolFailures=tf
   card.failure={message:"Provider turn failed.",status:"failed",class:"provider_error",runId:"run-2"}
   row.activity={type:"dynamic_tool",category:"tool",title:"t3-code.preview_open",failed:true,active:false,status:"failed",inspector:ref}
   phase=1
  }else if(phase===1){
   var causes=keys.findChild(card,"t3RunErrorCauses");check(causes&&causes.visible,"run error lists what failed")
   var detail=keys.findChild(card,"t3RunErrorCauseDetail");check(detail&&detail.text.indexOf("AppArmor")>=0,"the tool's own words are shown")
   check(card.implicitHeight>keys.findChild(card,"t3RunErrorTitle").height*3,"card grows to fit the causes")
   check(keys.findChild(row,"t3ActivityFailed").visible,"failed tool row is marked")
   check(b.activityText(row.activity).indexOf(Test.T3I18n.tr("Tool failed"))===0,"failed tool label: "+b.activityText(row.activity))
   var rd=keys.findChild(row,"t3ActivityFailureDetail");check(rd&&rd.visible&&rd.text.indexOf("AppArmor")>=0,"failed tool row shows its error")
   check(b.activityText({category:"read",count:3,entries:[{failed:true},{failed:false},{failed:true}],active:false}).indexOf("2")>0,"groups count their failures")
   console.log("T3_FAILURE_CONTEXT_QML_PASS: run error causes, failed tool rows");Qt.quit()
  }
 }}
}
