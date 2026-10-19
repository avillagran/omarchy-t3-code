import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property var launched:null;property int waited:0
 function check(ok,msg){if(!ok){console.error("ROW_MOTION_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function threads(doneIds,settledIds){var r=[{id:"w",title:"Working thread",working:true,startedAt:Date.now()-30000,runtimeMode:"full-access",modelSelection:{instanceId:"codex",model:"fixture"}},{id:"a",title:"Ready thread",status:"completed",runtimeMode:"auto-accept-edits",modelSelection:{instanceId:"codex",model:"fixture"}},{id:"s",title:"Settled thread",status:"completed",settled:true}]
  return r.map(function(t,i){var row=Object.assign({key:"fixture/"+t.id,environmentId:"fixture",projectKey:"fixture/p",projectId:"p",project:"Project",updatedAt:1e12-i*60000,activeAnchorAt:1e12-i*60000},t);if(doneIds.indexOf(t.id)>=0){row.working=false;row.status="completed";row.pending=true;row.reason="review"}if(settledIds.indexOf(t.id)>=0)row.settled=true;return row})}
 function row(list,id){var view=keys.findChild(panel,list);for(var i=0;i<view.count;i++){var it=view.itemAtIndex(i);if(it&&it.thread.id===id)return it}return null}
 function flights(){var layer=keys.findChild(panel,"sidebarFlightLayer");return layer.children.length}
 function push(state){fixture={threads:state,projects:[{key:"fixture/p",id:"p",title:"Project",environmentId:"fixture"}],environments:[]};Test.T3Backend.snapshotAboutToChange();Test.T3Backend.state=fixture}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}function onNewThreadWorkspaceRequested(request){root.launched=request}}
 Timer{interval:150;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:true,workingExpanded:true,settledExpanded:true,threadMoveEffect:"arc",threadMoveSpeed:"slow"});fixture={threads:threads([],[]),projects:[{key:"fixture/p",id:"p",title:"Project",environmentId:"fixture"}],environments:[]};b.state=fixture;panel.open();phase=1
  }else if(phase===1){if(panel.restoringSidebar)return;check(!!row("workingThreadList","w"),"working row starts in the Working shelf");check(!!row("threadList","a"),"ready row in the inbox");check(flights()===0,"no motion for plain state");
   push(threads(["w"],[]));phase=2
  }else if(phase===2){check(panel.rowMotion,"a snapshot that moves a thread animates");check(flights()===1,"the thread flies between sections, got "+flights());
   var target=row("threadList","w");check(!!target&&target.flight,"arriving row waits for the flight");phase=3;waited=0
  }else if(phase===3){if(++waited<9)return;check(flights()===0,"flight lands and cleans up");var target=row("threadList","w");check(!!target&&target.opacity===1&&!target.flight,"landed row is whole");check(!row("workingThreadList","w"),"left the Working shelf");
   push(threads(["w"],["a"]));phase=4;waited=0
  }else if(phase===4){if(++waited<9)return;check(!!row("settledThreadList","a"),"settled row arrives in Settled");check(flights()===0,"settle flight cleaned up");var settled=row("settledThreadList","a");check(settled.opacity===1&&settled.drift===0,"settled row whole");
   b.rememberView({threadMoveEffect:"off"});phase=5
  }else if(phase===5){push(threads(["w"],[]));check(!panel.rowMotion&&flights()===0,"off keeps the lists still");phase=6
  }else if(phase===6){check(!!row("threadList","a"),"unsettled without motion");
   b.selectThread("fixture","w");phase=7
  }else if(phase===7){check(b.currentRuntimeMode()==="full-access","open chat reports its permissions");keys.mouseClick(keys.findChild(panel,"newChatButton"));phase=8
  }else if(phase===8){var dialog=keys.findChild(panel,"t3NewChatDialog");check(dialog.visible,"new chat dialog opens");keys.mouseClick(keys.findChild(dialog,"newChatCreateThread"));
   var command=b.workspaceOperation.request.command;check(command.runtimeMode==="full-access"&&command.inheritRuntimeMode===true,"new thread inherits the open chat's permissions "+JSON.stringify(command));
   b.action({op:"settings",requestId:b.workspaceOperation.requestId,result:{mode:null,defaultRuntimeMode:"approval-required"}});phase=9
  }else if(phase===9){check(!!root.launched&&root.launched.command.runtimeMode==="full-access","project default does not override inherited permissions");
   console.log("ROW_MOTION_QML_PASS: section flights, fades, off switch and inherited new-thread permissions");panel.close();Qt.callLater(Qt.quit)
  }
 }}
}
