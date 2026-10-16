import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int cycles:0;property int waitFrames:0;property var fixture:null
 readonly property string image:"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="10" fill="white"/></svg>')
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Loader{id:loaded;active:false;sourceComponent:Component{Test.T3Panel{bridge:Test.T3Backend}} onLoaded:item.open()}
 Timer{interval:100;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;var rows=[];for(var i=0;i<35;i++)rows.push({key:"fixture/t"+i,id:"t"+i,environmentId:"fixture",project:"Project",projectKey:"fixture/p",title:"Example thread "+i,provider:"provider",model:"model"});b.state={threads:rows,projects:[],environments:[]};fixture=b.state;b.catalogs={fixture:{models:[{instanceId:"provider",model:"model",driverKind:"acpRegistry",registryIcon:root.image}],providers:[],capabilities:{}}};b.selectThread("fixture","t0");phase=1
  }else if(phase===1){if(b.draft.loading)return;if(!cycles)b.edit("Preserved through menu reopening");loaded.active=true;phase=2
  }else if(phase===2){loaded.item.close();phase=3
  }else if(phase===3){loaded.item.open();phase=4
  }else if(phase===4){if(b.draft.text!=="Preserved through menu reopening"){console.error("PANEL_REOPEN_QML_FAIL draft lost");Qt.exit(1)}loaded.item.close();waitFrames=0;phase=5
  }else if(phase===5){if(++waitFrames<3)return;loaded.active=false;if(++cycles>=10){console.log("PANEL_REOPEN_QML_PASS: 10 menu-style load/open/close/reopen/unload cycles with registry icons and preserved draft");Qt.quit()}phase=1}
 }}
}
