import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property bool ready:false
 property var fixture:({environments:[],projects:[],threads:[]})
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.ready)Test.T3Backend.state=root.fixture}}
 Timer{interval:100;running:!root.ready;repeat:true;onTriggered:{
  var b=Test.T3Backend;if(!b.viewStateLoaded)return
  fixture={environments:[],projects:[],threads:Array.from({length:Number(Quickshell.env("OT3_BENCH_ROWS")||24)},function(_,i){return {key:"fixture/"+i,id:String(i),environmentId:"fixture",title:"Thread "+i,project:"Example project",branch:"main",updatedAt:1000+i}})}
  b.rememberView({animationsEnabled:false,windowMode:"floating",windowWidth:1200,windowHeight:900,sidebarCompact:false});b.state=fixture;ready=true;panel.open();measure.start()
 }}
 Timer{id:measure;interval:3500;onTriggered:{console.log("SIDEBAR_MEMORY_READY");finish.start()}}
 Timer{id:finish;interval:800;onTriggered:Qt.quit()}
}
