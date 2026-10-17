import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property string selected:"";property var drawer:null
 function check(ok,msg){if(!ok){console.error("PROJECT_DRAFTS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.composerView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/a",id:"a",environmentId:"fixture",projectId:"p",title:"A"},{key:"fixture/b",id:"b",environmentId:"fixture",projectId:"p",title:"B"},{key:"fixture/c",id:"c",environmentId:"fixture",projectId:"other",title:"C"}],projects:[],environments:[]};b.selectThread("fixture","a");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Current prompt");b.patchDraft("fixture/b",{text:"From another thread",attachments:[],loading:false});b.patchDraft("fixture/c",{text:"Different project",attachments:[],loading:false});b.flushDrafts();selected=b.selectedKey;phase=2
  }else if(phase===2){keys.mouseClick(keys.findChild(c,"t3ExpandEditor"),14,14,Qt.RightButton);phase=3
  }else if(phase===3){if(b.projectDrafts.busy)return;check(!panel.composerExpanded&&c.projectDraftsVisible,"right click opens drawer in chat without expanding");drawer=keys.findChild(c,"t3ProjectDrafts");check(drawer.entries.length===2,"lists all project thread drafts, excludes other project: "+JSON.stringify(b.projectDrafts));var list=keys.findChild(drawer,"t3ProjectDraftList");check(list.activeFocus,"right-click selector receives keyboard focus");list.currentIndex=0;keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Return);phase=4
  }else if(phase===4){if(b.projectDrafts.busy)return;check(!b.projectDrafts.error,"restore succeeded "+b.projectDrafts.error);check(b.selectedKey===selected&&b.draft.text==="Current prompt\n\nFrom another thread","restore stays in current thread and keeps existing prompt");check(b.drafts["fixture/b"].text===""&&b.drafts["fixture/c"].text==="Different project","source consumed, unrelated project retained");check(!c.projectDraftsVisible,"compact drawer closes after restore");panel.expandComposer();phase=5
  }else if(phase===5){if(b.projectDrafts.busy)return;check(panel.composerExpanded&&c.projectDraftsVisible,"expanded editor opens project drafts automatically");check(keys.findChild(c,"t3ProjectDrafts")===drawer,"editor and chat use the same component instance");keys.mouseClick(keys.findChild(drawer,"t3StashPrompt"));phase=6
  }else if(phase===6){if(b.projectDrafts.busy)return;check(!b.projectDrafts.error&&b.draft.text==="","stash persists before clearing composer");check(drawer.entries.length===1&&!drawer.entries[0].sourceKey,"saved prompt belongs to project independently of any thread");drawer.pick(0);phase=7
  }else if(phase===7){if(b.projectDrafts.busy)return;check(b.draft.text==="Current prompt\n\nFrom another thread"&&!b.draft.attempt,"restore never submits prompt");check(panel.composerExpanded,"restoring in editor does not leave fullscreen");panel.expandComposer();b.selectThread("fixture","c");phase=8
  }else if(phase===8){keys.mouseClick(keys.findChild(c,"t3ExpandEditor"),14,14,Qt.RightButton);phase=9
  }else if(phase===9){if(b.projectDrafts.busy)return;check(drawer.entries.length===1&&drawer.entries[0].prompt==="Different project","changing project never leaks earlier entries");panel.close();console.log("PROJECT_DRAFTS_QML_PASS: shared fullscreen/chat drawer, actual right click, cross-thread restore, safe stash, no submission and project isolation");Qt.quit()}
 }}
}
