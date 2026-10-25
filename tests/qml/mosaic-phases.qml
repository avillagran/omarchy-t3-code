import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 function check(ok,msg){if(!ok){console.error("T3_MOSAIC_PHASES_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 // One thread per phase; each tile must name what its agent is doing.
 readonly property var fixtures:[
  {id:"think",title:"Refactor the sidebar model",working:true,activity:{type:"reasoning",category:"think",active:true},expect:"think"},
  {id:"read",title:"Audit the live transport",working:true,activity:{category:"read",active:true,title:"src/live.rs"},expect:"read"},
  {id:"edit",title:"Port upstream palette ranking",working:true,activity:{category:"edit",active:true,title:"ui/js/Palette.js"},expect:"edit"},
  {id:"run",title:"Release 0.2.23",working:true,activity:{category:"execute",active:true,command:"cargo test --release"},expect:"execute"},
  {id:"write",title:"Explain the reconnect backoff",working:true,streaming:true,expect:"respond"},
  {id:"ask",title:"Choose the base branch",pending:true,reason:"waiting",expect:"attention"},
  {id:"done",title:"Fix dictation long clips",sidebarStatus:"ready",pending:true,reason:"review",expect:"done"},
  {id:"fail",title:"Android AAR build",sidebarStatus:"failed",expect:"failed"}]
 Timer {interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={environments:[{id:"a",name:"omarchy"},{id:"b",name:"node"}],projects:[{key:"a/p",name:"omarchy-t3-code"},{key:"b/q",name:"lib-t3-rust"}],threads:root.fixtures.map(function(f,i){
    return Object.assign({key:(i%2?"b/":"a/")+f.id,id:f.id,environmentId:i%2?"b":"a",projectKey:i%2?"b/q":"a/p",project:i%2?"lib-t3-rust":"omarchy-t3-code",projectMonogram:i%2?"LR":"OT"},f)})}
   panel.open();b.rememberView({mosaicOpen:true,mosaicCount:8});phase=1
  } else if(phase===1){
   if(b.mosaicTargets.length!==8)return
   b.mosaicTargets.forEach(function(t){
    var f=root.fixtures.find(function(x){return t.key.split("/")[1]===x.id})
    var rows=[{id:"u",role:"user",text:"Please look at "+f.title.toLowerCase()+" and tell me what you find."},
     {id:"w",role:"activity",activity:{category:"read",title:"README.md",active:false}},
     {id:"a",role:"assistant",text:"I checked the module. The reader keeps one snapshot per thread and resumes from its last sequence, so reconnects never replay a half-applied state.",streaming:!!f.streaming}]
    if(f.activity)rows.push({id:"x",role:"activity",activity:f.activity})
    b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t.serial,result:{timeline:rows,activity:f.activity?[f.activity]:[]}}}))
    b.receive(JSON.stringify({type:"mosaic-state",data:{serial:t.serial,connected:true}}))
   })
   phase=2
  } else if(phase===2){
   var mosaic=keys.findChild(panel,"t3ThreadMosaic")
   root.fixtures.forEach(function(f,i){var t=b.state.threads.find(function(x){return x.id===f.id});check(mosaic.phaseOf(t,b.mosaicData[t.key])===f.expect,f.id+" reads as "+f.expect+", got "+mosaic.phaseOf(t,b.mosaicData[t.key]))})
   check(mosaic.labelOf("respond",b.state.threads[4],{})!=="","responding has a label")
   ;["think","read","edit","execute","respond","agent","work"].forEach(function(ph){check(Qt.colorEqual(mosaic.toneOf(ph),Test.T3Palette.working),ph+" wears the working blue, got "+mosaic.toneOf(ph))})
   if(++frames<4)return
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3-mosaic-phases.png");phase=3})
  } else if(phase===3){
   panel.close();console.log("T3_MOSAIC_PHASES_QML_PASS: thinking, reading, editing, commands, responding, input, done and failed each read distinctly");Qt.quit()
  }
 }}
}
