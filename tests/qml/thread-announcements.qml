import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("THREAD_ANNOUNCEMENTS_QML_FAIL "+msg);Qt.exit(1)}}
 function thread(working,status,run){return {id:"one",key:"fixture/one",environmentId:"fixture",title:"One",project:"Alpha",projectId:"p",updatedAt:1,working:working,status:status,latestRunId:run}}
 function set(t){fixture={threads:[t],projects:[{id:"p",key:"fixture/p",title:"Alpha",environmentId:"fixture"}],environments:[]};Test.T3Backend.state=fixture}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,a=keys.findChild(panel,"t3ActivityAnnouncer")
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});set(thread(false,"completed","r1"));b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;check(a&&a.key==="fixture/one","announcer follows the open thread");check(a.last==="","opening a thread is a silent baseline: "+a.last);set(thread(true,"running","r2"));phase=2
  }else if(phase===2){check(a.last==="Working","a new turn is announced: "+a.last);b.approvals=[{requestId:"q1",options:[]}];phase=3
  }else if(phase===3){check(a.last==="Needs approval","a pending approval is announced: "+a.last);b.approvals=[];set(thread(false,"completed","r2"));phase=4
  }else if(phase===4){check(a.last==="Done","the finished turn is announced: "+a.last);a.last="";set(thread(false,"completed","r2"));phase=5
  }else if(phase===5){check(a.last==="","an unchanged state is not repeated: "+a.last);panel.close();console.log("THREAD_ANNOUNCEMENTS_QML_PASS: silent baseline, working, approval, done, no repeats");Qt.quit()}
 }}
}
