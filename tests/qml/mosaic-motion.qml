import QtQuick
import Quickshell
import QtTest
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 property var watch:({})
 property int grow:1
 function check(ok,msg){if(!ok){console.error("T3_MOSAIC_MOTION_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 function mosaic(){return keys.findChild(panel,"t3ThreadMosaic")}
 function board(){return keys.findChild(panel,"t3MosaicBoard")}
 function tile(i){var b=board();var n=0;for(var k=0;k<b.children.length;k++){var c=b.children[k];if(c.present!==undefined){if(n===i)return c;n++}}return null}
 function feed(extra){
  var b=Test.T3Backend
  b.mosaicTargets.forEach(function(t,i){
   var rows=[{id:"u",role:"user",text:"Look at thread "+i},{id:"a",role:"assistant",text:"Checked thread "+i+"."}]
   if(extra&&i===0)rows.push({id:"new",role:"assistant",text:"A fresh answer arrives."})
   b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t.serial,result:{timeline:rows,activity:[]}}}))
   b.receive(JSON.stringify({type:"mosaic-state",data:{serial:t.serial,connected:true}}))
  })
 }
 function shot(name){keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3s/motion-"+name+".png")})}
 Timer {interval:120;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={environments:[{id:"a",name:"omarchy"}],projects:[{key:"a/p",name:"omarchy-t3-code"}],threads:[0,1,2,3,4,5,6,7,8,9].map(function(i){return {key:"a/t"+i,id:"t"+i,environmentId:"a",projectKey:"a/p",project:"omarchy-t3-code",projectMonogram:"OT",title:"Thread "+i,working:i%3===0,updatedAt:1000+i}})}
   panel.open();b.rememberView({mosaicOpen:true,mosaicCount:4});phase=1
  } else if(phase===1){
   if(b.mosaicTargets.length!==4)return
   feed(false);frames=0;phase=2
  } else if(phase===2){
   if(++frames<10)return
   check(tile(0).present&&tile(3).present&&!tile(4).present,"four slots present")
   check(!tile(5).visible,"slots past the count are hidden")
   root.watch={y4:tile(4).y,h0:tile(0).height}
   b.rememberView({mosaicCount:6});frames=0;phase=3
  } else if(phase===3){
   frames++
   if(frames===2){
    var t4=tile(4),slot=board().cell(4)
    check(t4.visible&&t4.y>slot.y+1,"slot 5 rises into place instead of jumping (y "+t4.y+" → "+slot.y+")")
    check(Math.abs(tile(0).height-root.watch.h0)>0.5&&Math.abs(tile(0).height-board().cell(0).height)>0.5,"existing tiles resize gradually ("+root.watch.h0+" → "+tile(0).height+" → "+board().cell(0).height+")")
    shot("mid")
   }
   if(frames<14)return
   ;[0,1,2,3,4,5].forEach(function(i){var c=board().cell(i);check(Math.abs(tile(i).y-c.y)<0.5&&Math.abs(tile(i).width-c.width)<0.5,"slot "+i+" settles in its cell")})
   shot("six");frames=0;phase=4
  } else if(phase===4){
   if(b.mosaicTargets.length!==6)return
   feed(true);frames=0;phase=5
  } else if(phase===5){
   if(++frames<2)return
   var list=tile(0).messageList
   check(list.count===3,"the new row is added in place, got "+list.count)
   root.grow=1;frames=0;phase=50
  } else if(phase===50){
   // A long answer streams in: the tile keeps the newest text in view.
   var t0=b.mosaicTargets.find(function(t){return t.key===tile(0).thread.key})
   var text="";for(var n=0;n<root.grow*6;n++)text+="Paragraph "+n+" of a long streamed answer that keeps arriving while the agent writes.\n\n"
   b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t0.serial,result:{timeline:[{id:"u",role:"user",text:"Write a lot"},{id:"a",role:"assistant",text:text,streaming:true}],activity:[]}}}))
   if(++frames<3)return
   var list=tile(0).messageList
   check(list.contentHeight>list.height*1.5,"the answer overflows the tile")
   check(list.originY+list.contentHeight-list.height-list.contentY<Style.space(24),"the tile follows streamed text (step "+root.grow+")")
   frames=0;if(++root.grow>4){phase=51;keys.mouseWheel(list,list.width/2,list.height/2,0,480)}
  } else if(phase===51){
   var list=tile(0).messageList
   if(++frames<5)return
   check(!list.follow,"scrolling up lets go of the tail")
   var y=list.itemAtIndex(1).y-list.contentY,t0=b.mosaicTargets.find(function(t){return t.key===tile(0).thread.key})
   var more="";for(var n=0;n<40;n++)more+="Paragraph "+n+" of a long streamed answer that keeps arriving while the agent writes.\n\n"
   b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t0.serial,result:{timeline:[{id:"u",role:"user",text:"Write a lot"},{id:"a",role:"assistant",text:more,streaming:true}],activity:[]}}}))
   root.watch={y:y};frames=0;phase=52
  } else if(phase===52){
   var list=tile(0).messageList
   if(++frames<3)return
   check(!list.follow&&list.originY+list.contentHeight-list.height-list.contentY>list.height,"new text does not yank a reader who scrolled up to the end")
   // Reordering the list (a thread updating) keeps every tile where it is.
   root.watch={keys:[0,1,2,3,4,5].map(function(i){return tile(i).thread.key})}
   var th=b.state.threads.slice(),head=th.splice(0,6);head.reverse();th=head.concat(th)
   b.state=Object.assign({},b.state,{threads:th})
   frames=0;phase=53
  } else if(phase===53){
   if(++frames<3)return
   ;[0,1,2,3,4,5].forEach(function(i){check(tile(i).thread.key===root.watch.keys[i],"slot "+i+" keeps its thread after a reorder")})
   b.rememberView({mosaicCount:2});frames=0;phase=6
  } else if(phase===6){
   if(++frames<10)return
   var m=mosaic()
   check(m.pages===5,"ten threads in pages of two, got "+m.pages)
   m.turn(1);check(m.page===1&&m.flipDir===1,"turning a page flips forward")
   frames=0;phase=7
  } else if(phase===7){
   if(++frames===2)shot("flip")
   if(frames<12)return
   var m2=mosaic(),want=m2.threads.slice(2,4).map(function(t){return t.key});check(want.indexOf(tile(0).thread.key)>=0&&want.indexOf(tile(1).thread.key)>=0,"the second page shows the next threads")
   mosaic().dismiss();frames=0;phase=8
  } else if(phase===8){
   if(b.viewState.mosaicOpen!==false){if(++frames>30)check(false,"dismiss closes the mosaic");return}
   panel.close();console.log("T3_MOSAIC_MOTION_QML_PASS: views glide, new slots rise, rows arrive in place, tiles follow streamed text until the reader scrolls up, slots survive reorders, pages flip and closing plays out");Qt.quit()
  }
 }}
}
