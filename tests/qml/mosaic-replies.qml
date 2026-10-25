import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/QuickReplies.js" as Replies
ShellRoot {
 id:root
 property int phase:0
 property int hoverTicks:0
 function check(ok,msg){if(!ok){console.error("T3_MOSAIC_REPLIES_QML_FAIL: "+msg);Qt.exit(1)}}
 function named(item,name){if(item.objectName===name)return item;var kids=item.children || [];for(var i=0;i<kids.length;i++){var found=named(kids[i],name);if(found)return found}return null}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer {interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={environments:[{id:"a",name:"Alpha"},{id:"b",name:"Beta"}],projects:[],threads:[
    {key:"a/one",id:"one",environmentId:"a",title:"One",projectKey:"a/p",working:true},
    {key:"b/two",id:"two",environmentId:"b",title:"Two",projectKey:"b/p",pending:true,reason:"waiting"},
    {key:"a/three",id:"three",environmentId:"a",title:"Three",projectKey:"a/p",settled:true},
    {key:"b/four",id:"four",environmentId:"b",title:"Four",projectKey:"b/p",snoozed:true}]}
   panel.open();keys.findChild(panel,"t3HostFilter").activated(1)
   check(panel.activeRows.length===1&&panel.settledRows.length===1&&panel.snoozedRows.length===0,"host scopes all shelves")
   panel.hostFilter="b";panel.statusFilter="waiting"
   check(panel.sidebarRows.length===1&&panel.sidebarRows[0].key==="b/two","host combines with waiting")
   panel.projectFilter="a/p"
   check(panel.sidebarRows.length===0,"project filter intersects host and waiting")
   panel.projectFilter="b/p";panel.statusFilter="pending"
   check(panel.sidebarRows.length===1&&panel.sidebarRows[0].key==="b/two","project and host combine with pending")
   panel.hostFilter="a";panel.projectFilter="a/p";panel.statusFilter="working"
   check(panel.sidebarRows.length===1&&panel.sidebarRows[0].key==="a/one","project and host combine with working")
   panel.hostFilter="";panel.projectFilter="";panel.statusFilter="all"
   var extra=[];for(var n=5;n<=8;n++)extra.push({key:"a/"+n,id:String(n),environmentId:"a",title:"Thread "+n,projectKey:"a/p"})
   b.state=Object.assign({},b.state,{threads:b.state.threads.concat(extra)})
   b.rememberView({mosaicOpen:true,mosaicCount:8});phase=1
  }else if(phase===1){
   if(b.mosaicTargets.length!==8)return
   var target=b.mosaicTargets[0],key=target.key,serial=target.serial
   b.mosaicTargets.forEach(function(t,i){b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:t.serial,result:{messages:[{role:"assistant",text:"Live tile "+i,id:"m"+i}]}}}))})
   check(Object.keys(b.mosaicData).length===8,"eight independent streams have content")
   b.mosaicTargets.forEach(function(t,i){check(b.mosaicData[t.key].messages[0].text==="Live tile "+i,"stream isolation for tile "+i)})
   check(b.mosaicData[key].messages[0].text==="Live tile 0","tile receives its own stream")
   b.receive(JSON.stringify({type:"mosaic-messages",data:{serial:-1,result:{messages:[]}}}))
   check(b.mosaicData[key].messages.length===1,"stale stream ignored")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3-mosaic.png")})
   panel.hostFilter="b";phase=2
  }else if(phase===2){
   if(b.mosaicTargets.length!==2)return
   check(!b.mosaicData["a/one"]&&Object.keys(b.mosaicData).length===2,"removed transcripts released and retained hosts preserved")
   b.rememberView({mosaicCount:1});phase=3
  }else if(phase===3){
   if(b.mosaicTargets.length!==1)return
   b.rememberView({mosaicOpen:false});phase=4
  }else if(phase===4){
   if(b.mosaicTargets.length)return
   check(Replies.language("implementa los cambios",[],"en","auto")==="es","draft overrides UI language")
   check(Replies.language("",[{role:"user",text:"please run the tests"}],"es","auto")==="en","last user message supplies language")
   check(Replies.language("please continue",[],"en","fr")==="fr","explicit language wins")
   check(Object.keys(Replies.defaults).length===20&&Replies.detect("結果を説明して")==="ja","all interface languages have replies; script detection works")
   check(Replies.replies("es",{es:[]}).length===0,"empty custom list respected")
   check(Replies.parse("  Continúa.\n\nValida.  ").join("|")==="Continúa.|Valida.","custom list parsed")
   b.selectThread("a","one");phase=5
  }else if(phase===5){
   if(b.draft.loading)return
   b.edit("Antes después",false);b.draftCursors={"a/one":{start:5,end:5}}
   b.rememberView({quickReplyLanguage:"es"})
   var button=keys.findChild(panel,"t3QuickRepliesButton")
   check(button.enabled,"quick reply button is interactive")
   keys.mouseMove(button,button.width/2,button.height/2)
   phase=50
  }else if(phase===50){
   var button=keys.findChild(panel,"t3QuickRepliesButton"),popup=button.menu
   if(!popup.visible&&++root.hoverTicks<6)return
   check(button.hovered&&popup.visible,"hover opens quick replies without clicking")
   var reply=keys.findChild(popup.contentItem,"t3QuickReply_0");check(!!reply,"quick reply menu creates rows");if(!reply)return
   keys.mouseClick(reply,reply.width/2,reply.height/2)
   check(b.draft.text==="Antes Continúa. después","reply preserves draft and inserts at cursor")
   check(!b.draft.busy&&!b.draft.attempt,"reply does not submit")
   check(!!named(panel.composerView,"t3QuickRepliesButton"),"quick reply control loads")
   panel.openPreferencesPage("general");phase=6
  }else if(phase===6){
   var section=named(panel.preferencesView,"preferencesQuickReplies")
   check(!!section,"preferences section loads")
   var edit=named(panel.preferencesView,"t3QuickRepliesEditor")
   check(!!edit,"custom replies editor instantiated")
   edit.text="Una respuesta";edit.parent.editLanguage="en"
   check(edit.text.indexOf("Continue.")>=0,"changing editor language reloads that list")
   edit.text="Custom reply\nValidate again"
   keys.findChild(panel.preferencesView,"t3QuickRepliesSave").clicked()
   check(b.viewState.quickReplies.en.join("|")==="Custom reply|Validate again","preferences save custom reply list")
   panel.close();console.log("T3_MOSAIC_REPLIES_QML_PASS: host/project/status intersections, eight isolated live tiles, cleanup, hover and click, languages, draft insertion and preferences");Qt.quit()
  }
 }}
}
