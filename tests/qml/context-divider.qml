import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool captured:false
 function check(ok,msg){if(!ok){console.error("CONTEXT_DIVIDER_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/thread",id:"thread",environmentId:"fixture",title:"Fixture"}],environments:[]};b.selectThread("fixture","thread");panel.open();b.messagesAboutToChange();b.transcript=[{id:"b",role:"context",context:{kind:"handoff",active:false,from:[{instanceId:"codex",model:"gpt"}],to:{instanceId:"kimi",model:"k3"}},createdAt:2},{id:"a",role:"assistant",text:"Report",rendered:'<p><a href="https://example.com/report">Report link</a></p>',createdAt:3}];b.messagesUpdated();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;var row=c.itemAtIndex(1),text=row?keys.findChild(row,"chatMessageText"):null,status=keys.findChild(panel,"connectionStatusIcon")
   check(text&&text.readOnly&&text.selectByMouse,"links retain selectable message text");check(status&&status.width<30&&status.Accessible.name.length>0,"compact accessible connection status")
   if(text){keys.mouseMove(text,12,text.height/2);check(text.hoveredLink==="https://example.com/report","link hover recognized");check(keys.findChild(text,"chatLinkCursor").cursorShape===Qt.PointingHandCursor,"link uses pointing hand cursor")}
   c.itemAtIndex(0).grabToImage(function(r){root.captured=r.saveToFile("/tmp/t3-context-divider.png")});phase=2
  }else if(phase===2){if(!captured)return;console.log("CONTEXT_DIVIDER_QML_PASS: lifecycle row, selectable hovered link and compact accessible status");panel.close();Qt.quit()
  }
 }}
}
