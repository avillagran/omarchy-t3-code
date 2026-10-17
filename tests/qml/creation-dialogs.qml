import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int mode:0;property var fixture:null
 function inside(dialog){var layout=keys.findChild(panel,"t3PanelLayout"),pos=dialog.contentItem.mapToItem(layout,0,0);console.log("CREATION_GEOMETRY",pos.x,pos.y,dialog.width,dialog.height,layout.width,layout.height);return pos.x>=0&&pos.y>=0&&pos.x+dialog.contentItem.width<=layout.width&&pos.y+dialog.contentItem.height<=layout.height}
 function check(ok,msg){if(!ok){console.error("CREATION_DIALOGS_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[],projects:[{key:"fixture/p",id:"p",title:"Repo",environmentId:"fixture"}],environments:[{id:"fixture",name:"Fixture",state:"ready"}]};b.state=fixture;panel.open();phase=1
  }else if(phase===1){keys.mouseClick(keys.findChild(panel,"newProjectButton"));phase=2
  }else if(phase===2){var wizard=keys.findChild(panel,"t3AddProjectDialog");check(wizard.visible&&wizard.width>100&&wizard.height>50,"project wizard visible from actual sidebar button");check(inside(wizard),"project wizard must be inside visible panel");wizard.close();keys.mouseClick(keys.findChild(panel,"newChatButton"));phase=3
  }else if(phase===3){var dialog=keys.findChild(panel,"t3NewChatDialog");check(dialog.visible&&dialog.width>100&&dialog.height>50,"chat dialog visible from actual sidebar button");check(inside(dialog),"chat dialog must be inside visible panel");dialog.close();b.requestNewThread({action:"new-thread",environment:"fixture",projectId:"p",title:"Worktree",modelSelection:{instanceId:"codex",model:"fixture"}});b.action({op:"settings",requestId:b.workspaceOperation.requestId,result:{mode:null,startFromOrigin:true}});var workspace=keys.findChild(panel,"t3WorkspaceConfirmation");check(workspace.visible&&inside(workspace),"workspace confirmation inside current window mode");workspace.close();if(mode<3){panel.cycleWindowMode();mode++;phase=4}else{panel.close();console.log("CREATION_DIALOGS_QML_PASS: actual sidebar clicks stay in panel/floating/tiled and return to panel");Qt.quit()}
  }else if(phase===4){if(b.windowLayoutOperation.busy)return;phase=1
 }}}
}
