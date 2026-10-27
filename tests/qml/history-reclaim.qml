import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property real deadline:0
 function check(ok,msg){if(!ok){console.error("HISTORY_RECLAIM_QML_FAIL "+msg);Qt.exit(1);throw new Error(msg)}}
 Timer{interval:100;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   check(!b.historyCollectionPending,"no periodic collection at startup")
   b.transcript=[{id:"small",role:"user",text:"Small history"}];b.threadWillChange()
   check(!b.historyCollectionPending,"small history does not request collection")
   b.selectedEnvironment="fixture";b.selectedThread="one";b.selectedKey="fixture/one"
   b.patchDraft(b.selectedKey,{text:"Keep unsent accented draft: acción",attachments:[],loading:false,busy:false,attempt:null})
   b.transcript=[{id:"large",role:"assistant",text:"Long response ".repeat(24000)}]
   b.clearSelection()
   check(b.historyCollectionPending&&!b.transcript.length,"discarding large history schedules one collection")
   check(b.drafts["fixture/one"].text==="Keep unsent accented draft: acción","discard preserves draft")
   deadline=Date.now()+1200;phase=1
  }else if(phase===1&&Date.now()>=deadline){
   check(b.historyCollectionPending,"collection waits for quiet navigation")
   b.messagesUpdated();deadline=Date.now()+1200;phase=2
  }else if(phase===2&&Date.now()>=deadline){
   check(b.historyCollectionPending,"message update defers pending collection")
   b.transcript=[{id:"new",role:"user",text:"New small history"}];b.threadWillChange()
   deadline=Date.now()+1200;phase=3
  }else if(phase===3&&Date.now()>=deadline){
   check(b.historyCollectionPending,"subsequent small navigation also defers pending collection")
   deadline=Date.now()+1200;phase=4
  }else if(phase===4&&Date.now()>=deadline){
   check(!b.historyCollectionPending,"collection fires once without restarting itself")
   check(b.transcript[0].text==="New small history"&&b.drafts["fixture/one"].text==="Keep unsent accented draft: acción","current history and unsent draft remain intact")
   console.log("HISTORY_RECLAIM_QML_PASS: large-release threshold, quiet delay, message/navigation deferral, one shot and draft preservation");Qt.quit()
  }
 }}
}
