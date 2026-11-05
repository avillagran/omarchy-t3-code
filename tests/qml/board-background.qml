import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 property real beforeClock:0
 function check(ok,msg){if(!ok){console.error("BOARD_BACKGROUND_QML_FAIL: "+msg);Qt.exit(1)}}
 function board(){var loader=keys.findChild(panel,"t3FocusBoardLoader");return loader?loader.item:null}
 function background(){var v=board();return v?keys.findChild(v,"t3BoardStarfield"):null}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:280;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,v=board(),bg=background(),sky=panel.starfield
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={working:1,threads:[{key:"fixture/work",id:"work",environmentId:"fixture",environmentName:"NODE",title:"Hilo trabajando",working:true,sidebarStatus:"working",projectKey:"fixture/p",projectId:"p",project:"O3"}],projects:[],environments:[{id:"fixture",name:"NODE",state:"ready"}]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[],models:[],capabilities:{}}}
   b.rememberView({interfaceLayout:"focus",animationsEnabled:true,starfieldMode:"full",starfieldSpeed:"fast",starfieldMeteors:"many",mosaicOpen:false});panel.open();panel.boardOpen=true;phase=1
  }else if(phase===1){
   if(!bg)return
   check(bg.visible&&bg.leader===sky,"board renders a follower of the actual panel sky");check(v.color.a===1,"conversation stays covered by opaque backing");check(bg.width===v.width&&bg.height===v.height,"background covers entire board");check(sky.mode==="full"&&bg.effectiveMode===sky.effectiveMode,"board uses the same motion mode");check(!sky.software&&bg.animated,"GPU board background animates when enabled");check(bg.brightness===sky.brightness&&bg.nebula===sky.nebula&&bg.meteorRate===sky.meteorRate,"same brightness, nebula and meteor settings");
   var p=sky.mapFromItem(v,0,0);check(Math.abs(bg.origin.x-p.x)<1&&Math.abs(bg.origin.y-p.y)<1,"sky aligns to panel coordinates");beforeClock=bg.clock;bg.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-sky-a.png")});phase=2
  }else if(phase===2){
   check(bg.clock>beforeClock&&Math.abs(bg.clock-sky.clock)<.0001&&Math.abs(bg.drift-sky.drift)<.0001,"background advances on the existing shared clock");check(sky.speedScale===1.8&&sky.meteorScale===2.2&&bg.meteors>0,"speed and meteor preferences apply to board");bg.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-sky-b.png")});phase=3
  }else if(phase===3){b.rememberView({starfieldMode:"light",starfieldMeteors:"none"});phase=4
  }else if(phase===4){
   check(bg.effectiveMode==="light"&&bg.animated&&bg.meteors===0,"light mode and disabled meteors are respected");b.rememberView({starfieldMode:"off"});phase=5
  }else if(phase===5){check(!bg.animated&&bg.effectiveMode==="off","background setting disables motion");beforeClock=bg.clock;phase=6
  }else if(phase===6){check(bg.clock===beforeClock,"off setting leaves clock still");b.rememberView({starfieldMode:"full",animationsEnabled:false});phase=7
  }else if(phase===7){check(!Test.T3Motion.enabled&&!bg.animated&&bg.effectiveMode==="off","global animation setting disables board too");beforeClock=bg.clock;phase=8
  }else if(phase===8){check(bg.clock===beforeClock,"global off remains still");b.rememberView({animationsEnabled:true,starfieldSpeed:"slow"});phase=9
  }else if(phase===9){check(bg.animated&&sky.speedScale===.5,"re-enabling animation and speed applies live");beforeClock=bg.clock;phase=10
  }else if(phase===10){check(bg.clock>beforeClock,"animation resumes without reopening board");panel.boardOpen=false;phase=11
  }else if(phase===11){check(!board(),"closing board disposes follower");panel.close();console.log("BOARD_BACKGROUND_QML_PASS: shared sky clock, actual motion, full/light/off/global settings, origin, disposal and opaque backing");Qt.quit()}
 }}
}
