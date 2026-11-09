import QtQuick
import Quickshell
import Quickshell.Io
import "../../ui/js/ThreadSearch.js" as ThreadSearch
import "../../ui/js/NativeJson.js" as NativeJson

ShellRoot {
  id: root
  property var threadSearch:({query:"",settledQuery:"",busy:false,matches:[],requestId:""})
  property var sidebarSearch:({query:"",settledQuery:"",busy:false,matches:[],requestId:""})
  property var state:({threads:[]})
  property var callbacks:({})
  property int serial:0
  property int index:0
  property bool selecting:false
  property bool completed:false
  property var queryCases:["a".repeat(199)+"\ud83d", "ab\udc00", "ab\ud83d\ude00", "ab\ud800"]
  function uuid(){return "qt-lossless-"+(++serial)}
  function check(value,reason){if(!value)throw Error(reason)}
  function send(packet){process.write(NativeJson.stringify(packet)+"\n")}
  function refresh(){send({op:"fixture-state"})}
  function restartThreadSearchDebounce(channel){Qt.callLater(function(){ThreadSearch.debounced(root,channel)})}
  function nativeSearch(input,callback){
    var id=uuid(),pending=Object.assign({},callbacks);pending[id]=callback;callbacks=pending
    var rows=(state.threads||[]).filter(function(row){return !!row.searchShell}).map(function(row){return {key:row.key,thread:row.searchShell}})
    send({op:"native-search",action:input.action,requestId:id,payloadJson:NativeJson.stringify(Object.assign({},input,{rows:rows}))})
  }
  function codecProbe(){
    var high=String.fromCharCode(0xd800),low=String.fromCharCode(0xdc00),pair=String.fromCharCode(0xd83d,0xde00)
    var keyed={};keyed["key"+high]="value"+low
    var value={nested:[high,low,pair,"a\u0000b",low+high],keyed:keyed,missing:undefined}
    var encoded=NativeJson.stringify(value),decoded=JSON.parse(encoded)
    check(decoded.nested[0].charCodeAt(0)===0xd800&&decoded.nested[1].charCodeAt(0)===0xdc00,"Qt value units")
    check(decoded.nested[2]===pair&&decoded.nested[3].charCodeAt(1)===0,"Qt pair/control units")
    check(decoded.nested[4].charCodeAt(0)===0xdc00&&decoded.nested[4].charCodeAt(1)===0xd800,"Qt reversed isolated units")
    var key=Object.keys(decoded.keyed)[0];check(key.charCodeAt(3)===0xd800&&decoded.keyed[key].charCodeAt(5)===0xdc00,"Qt key units")
    check(encoded.indexOf("\\ud800")>=0&&encoded.indexOf("\\udc00")>=0,"Qt well-formed wire escape")
    var standard=NativeJson.stringify({drop:undefined,list:[undefined,NaN,Infinity,-0],number:1.25,pair:pair,bmp:"日本"})
    check(standard==='{"list":[null,null,null,0],"number":1.25,"pair":'+NativeJson.quote(pair)+',"bmp":"日本"}',"native packet JSON semantics")
    check(NativeJson.quote(pair)==='"'+pair+'"'&&NativeJson.quote("日本")==='"日本"',"stable key pair/BMP identity")
    var nativeHigh=JSON.stringify(high),nativeParsed=JSON.parse(nativeHigh),nativeLow=JSON.parse(JSON.stringify(low))
    var nativeKeyed=JSON.parse(JSON.stringify(keyed)),nativeKey=Object.keys(nativeKeyed)[0]
    console.log("LOSSLESS_QT_NATIVE_STRINGIFY "+NativeJson.stringify({escaped:nativeHigh===NativeJson.quote(high),preserved:nativeParsed.charCodeAt(0)===0xd800,repaired:nativeParsed.charCodeAt(0)===0xfffd,lowPreserved:nativeLow.charCodeAt(0)===0xdc00,keyPreserved:nativeKey.charCodeAt(3)===0xd800,keyValuePreserved:nativeKeyed[nativeKey].charCodeAt(5)===0xdc00}))
    // Assert the exact packet that crosses QProcess, including escaped keys.
    send({op:"fixture-codec",payloadJson:encoded})
  }
  function begin(){
    selecting=false
    ThreadSearch.queue(root,"palette",queryCases[index],{environmentIds:["good","bad"],anyOlderSettled:true})
  }
  function choose(){
    if(selecting||threadSearch.busy||threadSearch.settledQuery!==queryCases[index]||!(state.threads||[]).length)return
    var item=state.threads[0]
    // A prior visible row can remain until the ordered mount refresh arrives.
    if(item.title.charCodeAt(8)!==queryCases[index].charCodeAt(queryCases[index].length-1))return
    check(item.id==="older"&&item.environmentId==="good"&&item.key==="good/older","row scope")
    check(item.title.charCodeAt(2)===0xd800,"native older title repaired")
    check(item.searchShell.environmentId==="good"&&item.searchShell.branch.charCodeAt(1)===0xdc00,"native shell repaired")
    selecting=true
    nativeSearch({action:"open",key:item.key,query:queryCases[index],now:1700000000000+index},function(result,error){
      check(!error&&result&&typeof result.searchQuery==="string","native pickedQuery missing")
      if(index===0)check(result.searchQuery==="a".repeat(32),"original pickedQuery32 policy")
      if(index===1)check(result.searchQuery.charCodeAt(2)===0xdc00,"picked low surrogate repaired")
      if(index===3)check(result.searchQuery==="ab","original trailing-high removal")
      send(ThreadSearch.visitPacket("good","older",1700000000000+index,true,result.searchQuery,uuid()))
    })
  }
  function receive(line){
    try{
      var packet=JSON.parse(line),data=packet.data
      if(packet.type==="ready"){codecProbe();return}
      if(packet.type==="codec-ready"){begin();return}
      if(packet.type==="snapshot"){state=ThreadSearch.unpackSnapshot(data);choose();return}
      if(packet.type==="complete"){check(packet.workerRetired&&packet.searches===4&&packet.visits===4,"owned completion");completed=true;return}
      check(packet.type==="action","unknown fixture event")
      if(data.op==="native-search-result"){
        var pending=Object.assign({},callbacks),callback=pending[data.requestId];delete pending[data.requestId];callbacks=pending
        check(typeof callback==="function","native result scope")
        callback(JSON.parse(data.result.resultJson),data.error||"");return
      }
      if(data.op==="thread-search"){
        var result=JSON.parse(data.result.resultJson)
        check(result.query===queryCases[index],"query repaired before actual RPC")
        if(result.matches.length){check(result.matches[0].environmentId==="good","match scope");check(result.matches[0].snippet.length===240&&result.matches[0].snippet.charCodeAt(239)===0xd83d,"snippet240 repaired")}
        if(result.errors.bad)check(result.errors.bad.cause.query===queryCases[index],"raw rejection repaired")
        ThreadSearch.accept(root,data);choose();return
      }
      if(data.op==="thread-visit"){
        var result=JSON.parse(data.result.resultJson)
        if(index===1)check(result.picked.charCodeAt(2)===0xdc00,"visit pickedQuery repaired")
        index++
        if(index===queryCases.length){send({op:"thread-search-mount",payloadJson:NativeJson.stringify({consumer:"palette",key:"",rows:[]})});send({op:"fixture-close"})}
        else begin()
        return
      }
      throw Error("unknown fixture action")
    }catch(error){console.error("LOSSLESS_QT_FAILURE "+String(error));Qt.exit(1)}
  }
  Process {
    id:process
    command:[Quickshell.env("O3_LOSSLESS_TEST_DRIVER"),"--qt-pipe"]
    running:true
    stdinEnabled:true
    stdout:SplitParser{onRead:function(line){root.receive(line)}}
    onExited:function(code,status){if(code!==0||!root.completed){console.error("LOSSLESS_QT_CHILD_FAILURE");Qt.exit(1)}else{console.log("LOSSLESS_QT_SEARCH_VISIT_PASS clip200 snippet240 nativePickedQuery32 lowVisit rawCause scope rows normalWorkerExit");Qt.exit(0)}}
  }
  Timer{interval:90000;running:true;onTriggered:{console.error("LOSSLESS_QT_DEADLINE");Qt.exit(1)}}
}
