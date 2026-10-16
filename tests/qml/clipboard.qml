import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_CLIPBOARD_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.composerView,e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("left selected right");e.forceActiveFocus();e.select(5,13);keys.keyClick(Qt.Key_V,Qt.ControlModifier);phase=2
  }else if(phase===2){if(b.draft.busy)return;check(b.draft.text==="left pasted text right","CtrlV inserts clipboard fixture at selection "+b.draft.text);check(!b.draft.attempt,"paste does not send");b.clipboardRequests={"fixture-paste":{key:b.selectedKey,start:5,end:16}};b.patchDraft(b.selectedKey,{busy:true});b.action({op:"clipboard",requestId:"fixture-paste",result:{attachments:[{id:"image_1",type:"image",name:"shot [1].png",mimeType:"image/png",sizeBytes:128}]}});phase=3
  }else if(phase===3){check(b.draft.text==="left ![shot 1 .png](t3-context://v1/image/image_1)  right","image reference retains inline position");check(b.draft.attachments.length===1&&!b.draft.busy,"uploaded descriptor attached and editor unlocked");b.removeAttachment(0);phase=4
  }else if(phase===4){check(b.draft.attachments.length===0&&!b.draft.text.includes("t3-context"),"remove clears inline reference and attachment");b.clipboardRequests={"failed":{key:b.selectedKey,start:0,end:4}};b.action({op:"clipboard",requestId:"failed",error:"clipboard_unavailable"});check(b.draft.text.startsWith("left"),"failed paste preserves selected text");panel.close();console.log("T3_CLIPBOARD_QML_PASS: CtrlV, text selection, image reference, descriptor, removal and failure preservation without real clipboard");Qt.quit()}
 }}
}
