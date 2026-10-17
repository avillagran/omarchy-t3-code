import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int modeCalls:0
 property int actionCalls:0
 property string lastMode:""
 property bool lastNewThread:false
 property var view:({loading:false,error:"",plans:[{id:"plan-a",title:"Native plan",status:"active",markdown:"# Native plan\nStep 1",displayMarkdown:"Step 1",rendered:"<p>Step 1</p>"}],ready:{id:"plan-a",title:"Native plan",status:"active",markdown:"# Native plan\nStep 1",displayMarkdown:"Step 1",rendered:"<p>Step 1</p>"}})
 function check(value,message){if(!value){console.error("PLAN_WORKFLOW_QML_FAIL "+message);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3PlanWorkflow {
  id:workflow;width:560;height:440;view:root.view;modeEnabled:true;interactionMode:"plan";canOperate:true;draftText:"Please change step 1"
  onModeRequested:function(mode){root.modeCalls++;root.lastMode=mode}
  onFollowUpRequested:function(plan,draft,newThread){root.actionCalls++;root.lastNewThread=newThread;root.check(plan.id==="plan-a","selected plan identity")}
 }
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  if(phase===0){check(workflow.actionable,"ready status");check(!!keys.findChild(workflow,"t3PlanMarkdown"),"markdown card exists");workflow.requestFeedback();check(actionCalls===1&&!lastNewThread,"feedback signal");workflow.requestImplementation(true);check(actionCalls===2&&lastNewThread,"new thread signal");workflow.modeRequested("default");check(lastMode==="default","mode selection");workflow.busy=true;phase=1}
  else if(phase===1){check(!workflow.actionable,"busy guards action");workflow.requestImplementation(false);check(actionCalls===2,"no busy submission");workflow.busy=false;root.view={loading:false,error:"",plans:root.view.plans,ready:null};phase=2}
  else if(phase===2){check(!workflow.actionable,"no inferred plan");workflow.planId="plan-a";check(workflow.plan && workflow.plan.id==="plan-a" && !workflow.actionable,"historical card read-only");root.view={loading:false,error:"connection_lost",plans:[],ready:null};workflow.planId="";phase=3}
  else if(phase===3){check(!workflow.actionable,"error state");console.log("PLAN_WORKFLOW_QML_PASS: projection, signals, busy, historical, error");Qt.quit()}
 }}
}
