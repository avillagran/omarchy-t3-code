import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("COPY_ACTIONS_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],environments:[]};b.selectThread("fixture","one");panel.open();b.messagesAboutToChange();b.transcript=[{id:"text",role:"assistant",text:"**Raw Markdown**\n<literal>",createdAt:1},{id:"image",role:"user",text:"An image",attachments:[{id:"picture",type:"image",name:"picture.png",mimeType:"image/png"}],createdAt:2}];b.messagesUpdated();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;var row=c.itemAtIndex(0),copy=row?keys.findChild(row,"copyChatMessage"):null
   check(copy&&copy.copyText==="**Raw Markdown**\n<literal>","copy uses source text not HTML");copy.clicked();check(copy.copyState==="busy"&&!copy.enabled,"copy awaits confirmation");b.copyFinished(copy.requestId,"");check(copy.copyState==="copied"&&copy.name==="check","successful copy shows check");phase=2
  }else if(phase===2){var row=c.itemAtIndex(1),thumb=keys.findChild(row,"t3MessageThumbnail"),copy=keys.findChild(row,"copyThumbnailImage")
   check(!!copy,"thumbnail offers copy");thumb.parent.forceActiveFocus();check(copy.visible,"keyboard focus exposes image copy");keys.mouseClick(thumb.parent,20,20,Qt.RightButton);var menuAction=thumb.parent.contextMenu.itemAt(0);check(menuAction&&menuAction.width>100,"image context action has usable width");menuAction.triggered();thumb.parent.contextMenu.close();check(copy.copyState==="busy","context action starts image copy");b.copyFinished(copy.requestId,"clipboard_unavailable");check(copy.copyState==="error"&&copy.name==="circle-alert","failure remains visible");keys.mouseClick(thumb.parent,20,20,Qt.LeftButton);phase=3
  }else if(phase===3){var full=keys.findChild(panel,"copyFullImage");check(full&&full.attachment.id==="picture","enlarged viewer copies same attachment");full.clicked();b.copyFinished(full.requestId,"");check(full.copyState==="copied","viewer copy feedback");panel.close();console.log("COPY_ACTIONS_QML_PASS: source text, completion/error feedback, thumbnail context/keyboard and full image copy");Qt.quit()}
 }}
}
