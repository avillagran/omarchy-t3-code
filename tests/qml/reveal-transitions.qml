import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property int step:0;property var fixture:null;property real fullWidth:0
 readonly property var styles:["nebula","warp","fold","spring"]
 property var quotas:[{provider:"codex",label:"Codex",weekly:{availablePercent:75,windowSeconds:604800},onPace:true,expectedAvailable:50}]
 function check(ok,msg){if(!ok){console.error("REVEAL_TRANSITIONS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function threads(){var r=[];for(var i=0;i<8;i++)r.push({key:"fixture/t"+i,id:"t"+i,environmentId:"fixture",project:"Project",title:"Thread "+i,status:"completed",updatedAt:1e12-i*60000,activeAnchorAt:1e12-i*60000});return r}
 function reveal(){return keys.findChild(panel,"sidebarReveal")}
 function grab(name){keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/reveal-"+name+".png")})}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Connections{target:Test.T3Backend;function onProvidersChanged(){if(root.phase>0&&Test.T3Backend.providers!==root.quotas)Test.T3Backend.providers=root.quotas}}
 Timer{interval:60;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.providers=root.quotas;b.rememberView({sidebarCompact:false,transitionStyle:"nebula",transitionSpeed:"slow"});b.sidebarCollapsed=false;b.state={threads:threads(),projects:[],environments:[]};fixture=b.state;panel.open();phase=1
  }else if(phase===1){var r=reveal();check(r&&r.visible&&r.extent===1&&!r.moving,"sidebar starts shown");fullWidth=r.width;check(fullWidth>100,"sidebar has width");phase=2
  }else if(phase===2){var s=styles[step];b.rememberView({transitionStyle:s,transitionSpeed:"slow"});check(Test.T3Motion.style===s&&Test.T3Motion.pace===1.6,"motion follows preferences");panel.toggleSidebar();phase=3
  }else if(phase===3){var r=reveal(),s=styles[step];if(r.progress>0.55)return;check(r.moving&&r.visible&&r.width>0&&r.width<fullWidth,s+" hides gradually "+r.width);grab(s+"-hide");phase=4
  }else if(phase===4){phase=5
  }else if(phase===5){var r=reveal();if(r.moving)return;check(!r.visible&&!r.moving,styles[step]+" ends hidden");panel.toggleSidebar();phase=6
  }else if(phase===6){var r=reveal(),s=styles[step];if(r.progress<0.45)return;check(r.moving&&r.visible&&r.width<fullWidth,s+" shows gradually");grab(s+"-show");phase=7
  }else if(phase===7){phase=8
  }else if(phase===8){var r=reveal();if(r.moving)return;check(r.visible&&!r.moving&&Math.abs(r.width-fullWidth)<1,styles[step]+" ends shown");step++;phase=step<styles.length?2:9
  }else if(phase===9){b.rememberView({transitionStyle:"warp",transitionSpeed:"slow"});panel.usageCollapsed=false;phase=10
  }else if(phase===10){var u=keys.findChild(panel,"t3HeaderUsageReveal");check(u.moving&&u.visible,"usage reveals with the transition");grab("usage");phase=11
  }else if(phase===11){phase=12
  }else if(phase===12){b.rememberView({transitionStyle:"instant"});panel.toggleSidebar();var r=reveal();check(!r.visible&&!r.moving,"instant hides at once");panel.toggleSidebar();check(r.visible&&r.extent===1,"instant shows at once")
   console.log("REVEAL_TRANSITIONS_QML_PASS: nebula, hyperspace, fold and spring hide/show, usage reveal, instant");panel.close();Qt.callLater(Qt.quit)}
 }}
}
