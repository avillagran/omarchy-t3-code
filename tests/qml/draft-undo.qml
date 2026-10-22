import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property string withImage:""
 function check(ok,msg){if(!ok){console.error("T3_DRAFT_UNDO_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,e=panel.composerView.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two"}],projects:[],environments:[]};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("safe prompt");e.forceActiveFocus();e.cursorPosition=e.length;b.resetUndo(b.selectedKey);b.clipboardRequests={paste:{key:b.selectedKey,start:e.cursorPosition,end:e.cursorPosition}};b.patchDraft(b.selectedKey,{busy:true});b.action({op:"clipboard",requestId:"paste",result:{attachments:[{id:"image-fixture",type:"image",name:"shot.png",mimeType:"image/png",sizeBytes:100}]}});phase=2
  }else if(phase===2){withImage=b.draft.text;check(b.draft.attachments.length===1&&e.cursorPosition===e.length,"image inserted at end");check(keys.findChild(panel.composerView,"t3PreviewDraftAttachment_0").visible&&!!keys.findChild(panel.composerView,"t3RemoveDraftImage_0"),"pasted image shows as a thumbnail with its remove button");e.forceActiveFocus();keys.keyClick(Qt.Key_Z,Qt.ControlModifier);phase=3
  }else if(phase===3){check(b.draft.text==="safe prompt"&&b.draft.attachments.length===0,"CtrlZ atomically undoes image and reference "+JSON.stringify({text:b.draft.text,files:b.draft.attachments,undo:b.draftUndo[b.selectedKey],focus:e.activeFocus}));check(e.cursorPosition===e.length,"undo restores cursor");keys.keyClick(Qt.Key_Z,Qt.MetaModifier|Qt.ShiftModifier);phase=4
  }else if(phase===4){check(b.draft.text===withImage&&b.draft.attachments[0].attachment.id==="image-fixture","SuperShiftZ reuses same image descriptor");e.insert(e.length,"x");e.insert(e.length,"y");phase=5
  }else if(phase===5){check(b.draft.text.endsWith("xy"),"typing after image tracked");keys.keyClick(Qt.Key_Z,Qt.MetaModifier);phase=6
  }else if(phase===6){check(b.draft.text===withImage&&b.draft.attachments.length===1,"one undo coalesces adjacent typing and keeps image");keys.keyClick(Qt.Key_Z,Qt.MetaModifier);phase=7
  }else if(phase===7){check(b.draft.text==="safe prompt"&&b.draft.attachments.length===0,"SuperZ removes image");keys.keyClick(Qt.Key_Y,Qt.ControlModifier);phase=8
  }else if(phase===8){check(b.draft.text===withImage&&b.draft.attachments.length===1,"CtrlY restores image");b.removeAttachment(0);phase=9
  }else if(phase===9){check(b.draft.attachments.length===0,"explicit remove");keys.keyClick(Qt.Key_Z,Qt.ControlModifier);phase=10
  }else if(phase===10){check(b.draft.text===withImage&&b.draft.attachments.length===1,"undo remove restores reference and image");b.patchDraft("fixture/two",{text:"other draft",loading:false,attachments:[]});b.selectThread("fixture","two");phase=11
  }else if(phase===11){check(e.plainText==="other draft"&&e.cursorPosition===e.length,"cached chat opens with cursor at end");e.cursorPosition=0;b.selectThread("fixture","one");phase=12
  }else if(phase===12){check(b.draft.text===withImage&&e.cursorPosition===e.length,"return to chat moves cursor to end");b.patchDraft("fixture/loading",{loading:true,text:""});b.selectThread("fixture","loading");phase=13
  }else if(phase===13){b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/loading",draft:{text:"loaded asynchronously",attachments:[]}}}));phase=14
  }else if(phase===14){check(e.cursorPosition===e.length&&e.plainText==="loaded asynchronously","async draft opens cursor at end "+JSON.stringify({text:e.plainText,cursor:e.cursorPosition,pending:panel.composerView.pendingCursorKey,loading:b.draft.loading}));panel.close();console.log("T3_DRAFT_UNDO_QML_PASS: Ctrl/Super undo/redo image/text/removal, same descriptor, per-chat history and cached/async end cursor");Qt.quit()}
 }}
}
