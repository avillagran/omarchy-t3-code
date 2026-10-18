import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("THREAD_AURA_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function threads(doneId){var r=[{id:"w",title:"Working thread",working:true,startedAt:Date.now()-30000},{id:"q",title:"Needs input",pending:true,reason:"request",status:"completed"},{id:"i",title:"Idle thread",status:"completed"}];
  return r.map(function(t,i){var row=Object.assign({key:"fixture/"+t.id,environmentId:"fixture",project:"Project",updatedAt:1e12-i*60000,activeAnchorAt:1e12-i*60000},t);if(t.id===doneId){row.working=false;row.status="completed";row.pending=true;row.reason="review"}return row})}
 function row(id){var list=keys.findChild(panel,"threadList");for(var i=0;i<list.count;i++){var it=list.itemAtIndex(i);if(it&&it.thread.id===id)return it}return null}
 function aura(id){var r=row(id);return r?keys.findChild(r,"threadAura"):null}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:400;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:false,starfieldMode:"full"});b.state={threads:threads(""),projects:[],environments:[]};fixture=b.state;panel.open();phase=1
  }else if(phase===1){var sky=keys.findChild(panel,"t3Starfield");check(sky.animated,"sky animates in full mode");
   check(aura("w")&&aura("w").visible&&aura("w").working>0,"working row orbits "+JSON.stringify({row:!!row("w"),aura:!!aura("w"),vis:aura("w")?aura("w").visible:null,w:aura("w")?aura("w").working:null,moving:row("w")?row("w").skyMoving:null,ids:panel.activeRows.map(function(t){return t.id})}));check(aura("q")&&aura("q").visible&&aura("q").attention>0,"input row glows");check(aura("i")&&!aura("i").visible,"idle row stays plain");
   keys.findChild(panel,"threadList").grabToImage(function(r){r.saveToFile("/tmp/thread-aura-1.png")});phase=2
  }else if(phase===2){b.state={threads:threads("w"),projects:[],environments:[]};fixture=b.state;phase=3
  }else if(phase===3){var a=aura("w");check(a&&a.bursting&&a.visible,"state change plays a shooting star "+(a?a.burst:"none"));keys.findChild(panel,"threadList").grabToImage(function(r){r.saveToFile("/tmp/thread-aura-2.png")});phase=4
  }else if(phase===4){phase=5}else if(phase===5){phase=6}else if(phase===6){var a=aura("w");check(a&&!a.bursting&&!a.visible,"burst ends and the row settles")
   b.rememberView({threadWorkingEffect:"pulse",threadInputEffect:"off",threadChangeEffect:"flash",threadEffectIntensity:"vivid",starfieldSpeed:"fast",starfieldMeteors:"none"});b.state={threads:threads(""),projects:[],environments:[]};fixture=b.state;phase=7
  }else if(phase===7){phase=8}else if(phase===8){var sky=keys.findChild(panel,"t3Starfield");check(sky.speedScale===1.8&&sky.meteorScale===0,"sky follows speed and meteor preferences");
   var w=aura("w");check(w&&w.visible&&w.workingStyle===1&&w.intensity===1.6,"working row pulses at vivid intensity");check(aura("q")&&!aura("q").visible,"input effect can be turned off");check(row("w").changeStyle===1,"state change uses the flash");
   b.rememberView({starfieldMode:"off",threadInputEffect:"glints"});phase=9
  }else if(phase===9){phase=10}else if(phase===10){check(aura("w")&&!aura("w").visible,"a still sky stops the working effect");var q=aura("q");check(q&&q.visible&&q.attentionStyle===2,"input stays highlighted, still, without motion");
   console.log("THREAD_AURA_QML_PASS: working comet, input glow, state-change shooting star, animation preferences");panel.close();Qt.callLater(Qt.quit)}
 }}
}
