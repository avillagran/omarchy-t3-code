import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
// A chat opened with a prompt to review: a new thread in the chosen project,
// selected, with the prompt in its draft and nothing sent; or sent at once.
ShellRoot {
 id:root;property int phase:0;property int waits:0;property var fixture:null;property string creation:"";property string submitted:""
 function check(ok,msg){if(!ok){console.error("PROMPTED_CHAT_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}function onMessageSubmitted(key){root.submitted=key}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return
   fixture={threads:[{key:"fixture/one",id:"one",projectId:"p",projectKey:"fixture/p",environmentId:"fixture",title:"Source",modelSelection:{instanceId:"codex",model:"fixture"},runtimeMode:"approval-required",branch:"main"}],projects:[{key:"fixture/p",id:"p",environmentId:"fixture",title:"Repo"}],environments:[]}
   b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return
   panel.preferencesOpen=true
   check(b.startPromptedChat("fixture","p","Set up the device"),"starts");check(!panel.preferencesOpen&&panel.opened,"preferences give way to the chat")
   var m=b.managementRequest;check(m&&m.action==="new-thread"&&m.projectId==="p"&&!m.text&&!m.launchWithMessage,"a blank new thread, nothing sent: "+JSON.stringify(m));root.creation=m.requestId
   fixture=Object.assign({},fixture,{threads:fixture.threads.concat([{key:"fixture/two",id:"two",projectId:"p",projectKey:"fixture/p",environmentId:"fixture",title:"New chat",modelSelection:{instanceId:"codex",model:"fixture"},runtimeMode:"approval-required"}])});b.state=fixture
   b.action({op:"manage",environment:"fixture",requestId:root.creation,result:{action:"new-thread",threadId:"two"}});phase=2
  }else if(phase===2){
   if((b.selectedKey!=="fixture/two"||b.draft.loading)&&++waits<40)return
   check(b.selectedKey==="fixture/two","the new chat is selected");
   check(b.draft.text==="Set up the device"&&!b.draft.attempt&&!b.managementRequest,"its draft holds the prompt, unsent: "+JSON.stringify({text:b.draft.text,attempt:b.draft.attempt}));
   check(b.promptPrefill===null,"the prompt is applied once");
   check(b.drafts["fixture/one"]===undefined||b.drafts["fixture/one"].text!=="Set up the device","the source chat keeps its own draft");
   check(!b.startPromptedChat("fixture","p","  "),"an empty prompt opens nothing");
   check(b.startPromptedChat("fixture","p","Pair the device",true),"starts one sent at once");root.creation=b.managementRequest.requestId
   fixture=Object.assign({},fixture,{threads:fixture.threads.concat([{key:"fixture/three",id:"three",projectId:"p",projectKey:"fixture/p",environmentId:"fixture",title:"New chat",modelSelection:{instanceId:"codex",model:"fixture"},runtimeMode:"approval-required"}])});b.state=fixture
   b.action({op:"manage",environment:"fixture",requestId:root.creation,result:{action:"new-thread",threadId:"three"}});waits=0;phase=3
  }else if(phase===3){
   if(root.submitted!=="fixture/three"&&++waits<40)return
   check(b.selectedKey==="fixture/three"&&root.submitted==="fixture/three"&&b.draft.text==="Pair the device","sent in the new chat: "+root.submitted);
   check(b.drafts["fixture/two"].text==="Set up the device"&&!b.drafts["fixture/two"].attempt,"the earlier chat stays unsent");
   console.log("PROMPTED_CHAT_QML_PASS: blank new thread, preferences give way, prompt in the new draft only, unsent, applied once; sent at once on request");Qt.quit()
  }
 }}
}
