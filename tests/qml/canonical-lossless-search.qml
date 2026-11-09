import QtQuick
import Quickshell
import O3.CanonicalProcessTest 1.0
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeJson.js" as NativeJson
ShellRoot {
 id:root
 property var backend:Test.T3Backend
 property var actualWorker:null
 property bool attached:false
 property bool finishing:false
 property bool allVisitsDone:false
 property bool releaseAck:false
 property bool sidebarRequested:false
 property bool selected:false
 property bool resultSeen:false
 property bool causeSeen:false
 property bool sharedChecked:false
 property bool paletteReleased:false
 property int index:0
 property int visits:0
 property int packetCount:0
 property int searchPacketCount:0
 property int matchingSearchPacketCount:0
 property int snapshotPacketCount:0
 property int lastPending:-1
 property int lastMatches:-1
 property int lastRawRows:-1
 property var queries:["a".repeat(199)+"\ud83d","ab\udc00","ab\ud83d\ude00","ab\ud800"]
 ProcessInspector{id:inspector}
 Test.T3Panel{id:panel;bridge:root.backend}
 function check(ok,why){if(!ok)throw Error(why)}
 function diagnostics(){
  var q=queries[index]||"",search=backend.threadSearch||{},sidebar=backend.sidebarSearch||{}
  var row=(backend.state.threads||[]).find(function(t){return t.key==="good/older"}),key="",decoded=null,keyDecoded=false
  try{key=JSON.parse(search.mountSignature||"[]")[0]||"";decoded=JSON.parse(key);keyDecoded=true}catch(_){}
  return {qualification:"after-failure-non-atomic",index:index,visits:visits,attached:attached,
   actualWorkerRunning:!!(actualWorker&&actualWorker.running),packets:packetCount,searchPackets:searchPacketCount,
   matchingSearchPackets:matchingSearchPacketCount,snapshotPackets:snapshotPacketCount,
   resultSeen:resultSeen,causeSeen:causeSeen,queryEqual:search.query===q,settledEqual:search.settledQuery===q,
   queryLength:(search.query||"").length,settledLength:(search.settledQuery||"").length,busy:!!search.busy,
   errorPresent:!!search.error,planReady:!!search.planReady,canSearch:search.plan?!!search.plan.canSearch:null,
   searchMessages:search.plan?!!search.plan.searchMessages:null,lastPending:lastPending,lastMatches:lastMatches,lastRawRows:lastRawRows,
   rowPresent:!!row,titleString:!!(row&&typeof row.title==="string"),
   titleUnit8Matches:!!(row&&typeof row.title==="string"&&row.title.charCodeAt(8)===q.charCodeAt(q.length-1)),
   titleUnit2Matches:!!(row&&typeof row.title==="string"&&row.title.charCodeAt(2)===0xd800),
   trustedRowScope:!!(row&&row.environmentId==="good"),rawShellPresent:!!(row&&row.searchShell),
   trustedShellScope:!!(row&&row.searchShell&&row.searchShell.environmentId==="good"),
   branchUnitMatches:!!(row&&row.searchShell&&typeof row.searchShell.branch==="string"&&row.searchShell.branch.charCodeAt(1)===0xdc00),
   mountKeyDecoded:keyDecoded,mountQueryEqual:!!(decoded&&decoded[1]===q),
   mountPairLiteral:key.indexOf("\ud83d\ude00")>=0,mountIsolatedEscaped:key.indexOf("\\ud83d")>=0,
   selected:selected,sharedChecked:sharedChecked,sidebarRequested:sidebarRequested,paletteReleased:paletteReleased,
   releaseAck:releaseAck,sidebarBusy:!!sidebar.busy,sidebarSettledEqual:sidebar.settledQuery===q,
   allVisitsDone:allVisitsDone,finishing:finishing,viewStateLoaded:!!backend.viewStateLoaded,panelVisible:!!backend.panelVisible}
 }
 function fail(error){
  try{console.error("CANONICAL_LOSSLESS_DIAGNOSTICS "+JSON.stringify(diagnostics()))}catch(_){console.error("CANONICAL_LOSSLESS_DIAGNOSTICS_UNAVAILABLE")}
  console.error("CANONICAL_LOSSLESS_QT_FAIL "+String(error));Qt.exit(1)
 }
 function observe(line){try{
  var packet=JSON.parse(line),d=packet.data||{}
  packetCount++;if(packet.type==="snapshot")snapshotPacketCount++
  check(packet.type!=="native-search-error","actual native worker close error")
  if(packet.type!=="action")return
  if(d.op==="thread-search"&&d.result&&typeof d.result.resultJson==="string"){
   searchPacketCount++
   var r=JSON.parse(d.result.resultJson)
   if(r.query!==queries[index])return
   matchingSearchPacketCount++;lastPending=typeof r.pending==="number"?r.pending:-1;lastMatches=(r.matches||[]).length;lastRawRows=(r.rows||[]).length
   if(r.matches.length){check(r.matches[0].environmentId==="good","trusted RPC scope");check(r.matches[0].snippet.length===240&&r.matches[0].snippet.charCodeAt(239)===0xd83d,"canonical snippet240");resultSeen=true}
   if(r.errors&&r.errors.bad){check(r.errors.bad.cause.query===queries[index],"canonical raw failure query");causeSeen=true}
  }
  if(d.op==="thread-search-mount"&&d.requestId==="canonical-release-barrier"){check(!d.error,"real palette release barrier");releaseAck=true}
  if(d.op==="thread-visit"){
   check(!d.error&&d.result&&typeof d.result.resultJson==="string","actual visit result")
   var picked=JSON.parse(d.result.resultJson).picked
   if(index===0)check(picked==="a".repeat(32),"original picked32")
   if(index===1)check(picked.charCodeAt(2)===0xdc00,"canonical picked low-surrogate visit")
   if(index===2)check(picked==="ab\ud83d\ude00","canonical valid pair visit")
   if(index===3)check(picked==="ab","original trailing-high removal")
   visits++;backend.clearSelection();backend.setSidebarSearchActive(false);backend.searchThreads("")
   index++;selected=false;resultSeen=false;causeSeen=false
   if(index===queries.length){allVisitsDone=true;backend.refresh()}
   else backend.searchThreads(queries[index])
  }
 }catch(error){fail(error)}}
 Timer{interval:30;repeat:true;running:true;onTriggered:{try{
  if(!attached){
   // The private native observer uses QObject ancestry because the default
   // QML children list has no count callback. Product handlers stay intact.
   inspector.find(backend,backend.binary)
   var child=inspector.observed
   if(child){
     actualWorker=child;child.stdout.read.connect(root.observe)
     child.exited.connect(function(code,status){try{check(finishing&&visits===4&&code===0&&status===0,"canonical main normal exit");console.log("CANONICAL_LOSSLESS_MAIN_QT_PASS actualBackend actualMain clip200 snippet240 rawFailure picked32 lowVisit scopedRows sharedOwners normalMainExit");Qt.exit(0)}catch(error){fail(error)}})
     attached=true
   }
   if(!attached)return
  }
  if(finishing||paletteReleased)return
  if(allVisitsDone){if((backend.state.threads||[]).some(function(t){return t.key==="good/older"}))return;finishing=true;backend.send({op:"quit"});return}
  if(!backend.viewStateLoaded||!(backend.state.environments||[]).some(function(e){return e.id==="good"&&e.state==="ready"}))return
  if(!backend.panelVisible)panel.open()
  if(backend.threadSearch.query!==queries[index]){backend.searchThreads(queries[index]);return}
  var search=backend.threadSearch
  if(search.busy||search.settledQuery!==queries[index]||!resultSeen||!causeSeen||selected)return
  var row=(backend.state.threads||[]).find(function(t){return t.key==="good/older"})
  if(!row||row.title.charCodeAt(8)!==queries[index].charCodeAt(queries[index].length-1))return
  check(row.environmentId==="good"&&row.title.charCodeAt(2)===0xd800,"canonical raw normalized row")
  check(row.searchShell.environmentId==="good"&&row.searchShell.branch.charCodeAt(1)===0xdc00,"canonical raw shell scope")
  var signature=JSON.parse(search.mountSignature),key=JSON.parse(signature[0]);check(key[1]===queries[index],"actual mount key UTF16")
  if(index===2)check(signature[0].indexOf("\ud83d\ude00")>=0,"modern valid pair key stays literal")
  if(index===0)check(signature[0].indexOf("\\ud83d")>=0,"modern isolated key escape")
  if(index===0&&!sharedChecked){if(!sidebarRequested){backend.setSidebarSearchActive(true);backend.searchSidebar(queries[index]);sidebarRequested=true;return}if(backend.sidebarSearch.busy||backend.sidebarSearch.settledQuery!==queries[index])return;sharedChecked=true;backend.searchThreads("");paletteReleased=true;backend.send({op:"thread-search-mount",requestId:"canonical-release-barrier",payloadJson:NativeJson.stringify({consumer:"palette",key:"",rows:[]})});return}
  selected=true;backend.selectThread("good","older",false,queries[index])
 }catch(error){fail(error)}}}
 // The ownership phase intentionally releases only the palette, then verifies
 // that the real main still projects the row held by the sidebar before pick.
 Timer{interval:40;repeat:true;running:root.paletteReleased&&!root.selected;onTriggered:{try{
  if(!releaseAck)return
  check(backend.sidebarSearch.settledQuery===queries[0],"sidebar retained query")
  var row=(backend.state.threads||[]).find(function(t){return t.key==="good/older"});if(!row)return
  check(backend.threadSearch.query===""&&row.title.charCodeAt(2)===0xd800,"unrelated real owner survives palette release")
  paletteReleased=false;selected=true;backend.selectThread("good","older",false,queries[0])
 }catch(error){fail(error)}}}
 Timer{interval:90000;running:true;onTriggered:fail("canonical deadline")}
}
