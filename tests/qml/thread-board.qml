import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property real boardWidth:1680
 property string ideaKey:""
 property string converted:""
 property var fixture:null
 function check(ok,msg){if(!ok){console.error("THREAD_BOARD_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 FloatingWindow {
  id:window
  implicitWidth:1680;implicitHeight:960;visible:true
  Loader{id:loader;anchors.left:parent.left;anchors.top:parent.top;anchors.bottom:parent.bottom;width:root.boardWidth;active:false;sourceComponent:Test.T3ThreadBoard{bridge:Test.T3Backend;threads:root.fixture?root.fixture.threads:[];onIdeaRequested:function(text){root.converted=text}}}
 }
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer {interval:240;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,board=loader.item
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={environments:[{id:"fixture",name:"NODE"}],projects:[],threads:[]}
   var states=["idle","running","running","completed"]
   for(var i=0;i<16;i++)fixture.threads.push({id:"task"+i,key:"fixture/task"+i,environmentId:"fixture",environmentName:"NODE",project:"Proyecto "+(i%3),projectId:"p",projectKey:"fixture/p",title:["Preparar el tablero y sus columnas","Integrar las tarjetas de conversaciones en vivo","Revisar la propuesta antes de continuar","Pruebas de persistencia y recuperación"][i%4],status:states[i%4],working:i%4===1||i%4===2,pending:i%4===2,reason:i%4===2?"request":"",updatedAt:100-i})
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],capabilities:{}}}
   b.rememberView({interfaceLayout:"focus",boardIdeas:[],boardIdeaText:"Una idea anterior\n- [ ] conservarla",boardLive:false,hostStyles:{fixture:{color:"#8fc46a"}}});loader.active=true;phase=1
  }else if(phase===1){
   check(b.viewState.boardIdeas.length===1&&b.viewState.boardIdeas[0].text.indexOf("Una idea anterior")===0,"migrates legacy note without losing text")
   ideaKey=board.addIdea("Crear una herramienta para organizar ideas\n- [ ] Definir el primer paso")
   board.editingIdea="";check(b.viewState.boardIdeas.length===2,"multiple independent ideas")
   var viewport=keys.findChild(board,"t3BoardViewport");check(viewport.contentWidth>=viewport.width,"columns stay navigable")
   check(keys.findChild(board,"t3BoardThread_task0").height<130,"compact card has no empty live panel or large action")
   phase=2;board.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-compact.png")})
  }else if(phase===2){
   board.updateIdea(ideaKey,"Idea refinada\n- [x] Definir el primer paso\n- [ ] Crear un prototipo")
   keys.findChild(board,"t3BoardConvertIdea_"+ideaKey).clicked()
   check(converted.indexOf("Idea refinada")===0&&b.viewState.boardIdeas.length===2,"conversion hands off full text and keeps original")
   keys.findChild(board,"t3BoardLiveMode").clicked();check(keys.findChild(board,"t3BoardThread_task0").live&&keys.findChild(board,"t3BoardThread_task3").live,"global live expands idle and done too");phase=3
  }else if(phase===3){
   if(!b.mosaicTargets.some(function(t){return t.key==="fixture/task1"}))return
   b.mosaicTargets.forEach(function(t){b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t.serial,result:{timeline:[{id:"user",role:"user",text:"Revisa el diseño y conserva los borradores."},{id:"answer",role:"assistant",text:"Las columnas ya separan los estados. Estoy verificando las tarjetas y el guardado de ideas para que puedas continuar sin perder el contexto."}]}}}))})
   phase=4
  }else if(phase===4){
   check(board.liveThreads.length>0&&board.liveThreads.length<=24,"all visible columns receive live updates, including idle and done")
   check(keys.findChild(board,"t3BoardMessages_task1").count===2,"live card renders actual conversation")
   board.toggleLive(fixture.threads[0]);board.writeTo(fixture.threads[0]);phase=5
  }else if(phase===5){
   b.patchDraft("fixture/task0",{loading:true});b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/task0",draft:{text:"Borrador existente"},error:null}}));phase=6
  }else if(phase===6){
   var reply=keys.findChild(board,"t3BoardReply_task0")
   check(reply.enabled&&reply.text==="Borrador existente","inline composer loads existing thread draft")
   check(board.editingKey==="fixture/task0","writing stays in board")
   reply.forceActiveFocus();reply.text="Borrador editado en el tablero";check(b.draft.text===reply.text,"inline changes use durable thread draft")
   phase=7;board.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-live.png")})
  }else if(phase===7){root.boardWidth=640;phase=8
  }else if(phase===8){
   check(board.liveMode&&keys.findChild(board,"t3BoardThread_task3").live,"offscreen done cards remain expanded")
   check(!b.mosaicTargets.some(function(t){return t.key==="fixture/task3"}),"offscreen streams are released; width="+board.width+" x="+keys.findChild(board,"t3BoardViewport").contentX)
   var view=keys.findChild(board,"t3BoardViewport");view.contentX=view.contentWidth-view.width;phase=9
  }else if(phase===9){
   check(b.mosaicTargets.some(function(t){return t.key==="fixture/task3"}),"horizontal navigation subscribes newly visible done cards")
   loader.active=false;phase=10
  }else if(phase===10){check(b.mosaicTargets.length===0,"closing releases subscriptions");loader.active=true;phase=11
  }else if(phase===11){check(b.viewState.boardIdeas.length===2,"ideas survive board recreation");check(b.draft.text==="Borrador editado en el tablero","board recreation preserves draft");console.log("THREAD_BOARD_QML_PASS: compact cards, idea migration/edit/conversion, bounded live messages, inline durable draft and cleanup");Qt.quit()}
 }}
}
