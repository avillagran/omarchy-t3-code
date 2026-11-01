import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int step:0
 property var fixture:null
 readonly property var layouts:["classic","focus","projects"]
 function check(ok,msg){if(!ok){console.error("SIDEBAR_METADATA_QML_FAIL: "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 function row(){var list=panel.sidebarView;for(var i=0;i<list.count;i++){var r=list.itemAtIndex(i);if(r&&r.thread&&r.thread.id==="one")return r}return null}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return
   fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",environmentName:"Long remote host name",projectId:"p",projectKey:"fixture/p",project:"A very long project name that must elide",title:"Task",status:"completed"}],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"A very long project name that must elide",workspaceRoot:"/w/p"}],environments:[]};b.state=fixture
   b.rememberView({threadRowSize:"compact",sidebarCompact:false,interfaceLayout:"classic",threadGroups:{"fixture/one":"A long custom group name"}});panel.open();phase=1
  }else if(phase===1){b.rememberView({interfaceLayout:layouts[Math.floor(step/2)]});panel.resizeSidebar(step%2?600:260);phase=2
  }else if(phase===2){var r=row();if(!r)return;var p=keys.findChild(r,"threadCompactProject"),g=keys.findChild(r,"threadCompactGroup"),host=keys.findChild(r,"threadFocusHost");check(p&&g&&p.visible&&g.visible,"project and group remain visible");check(p.width>0&&g.width>0,"both labels retain usable width: "+JSON.stringify({step:step,row:r.width,project:p.width,group:g.width,host:host.width}));check(p.mapToItem(r,p.width,0).x<=g.mapToItem(r,0,0).x,"project and group do not overlap");if(host.visible){check(g.mapToItem(r,g.width,0).x<=host.mapToItem(r,0,0).x,"group does not overlap host");check(host.mapToItem(r,host.width,0).x<=r.width,"host stays within row")}
   if(++step<6)phase=1;else{panel.close();console.log("SIDEBAR_METADATA_QML_PASS");Qt.quit()}
  }
 }}
}
