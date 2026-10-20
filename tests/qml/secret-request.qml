import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SECRET_REQUEST_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"secret/s1",role:"secret",text:"",createdAt:1,secret:{turnItemId:"s1",threadId:"one",label:"GitHub token",reason:"To open the PR",status:"pending",local:true}},{id:"secret/s2",role:"secret",text:"",createdAt:2,secret:{turnItemId:"s2",threadId:"one",label:"API key",reason:"",status:"saved",local:true}}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return;var card=keys.findChild(panel.conversationView.itemAtIndex(0),"t3SecretRequest");check(card&&card.visible&&card.pending,"pending request shows a form");var input=keys.findChild(card,"t3SecretInput");check(input.echoMode===TextInput.Password,"value is masked");check(!keys.findChild(card,"t3SecretSave").enabled,"save needs a value");input.text="  ghp_secret  ";check(keys.findChild(card,"t3SecretSave").enabled,"save enabled");keys.findChild(card,"t3SecretSave").clicked();check(input.text==="","field is cleared after sending");check(b.secretAnswers.s1.busy,"answer in flight");check(JSON.stringify(b.secretAnswers).indexOf("ghp_secret")<0,"the value is not kept in QML state")
   var done=keys.findChild(panel.conversationView.itemAtIndex(1),"t3SecretRequest");check(done&&!done.pending,"answered requests show the outcome only")
   b.action({op:"secret-answer",requestId:b.secretAnswers.s1.requestId,error:"rpc_rejected"});check(!b.secretAnswers.s1.busy&&b.secretAnswers.s1.error==="rpc_rejected","errors are shown");panel.close();console.log("SECRET_REQUEST_QML_PASS: masked form, save/decline, outcome rows, value not retained");Qt.quit()}
 }}
}
