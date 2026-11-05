import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/ModelSync.js" as ModelSync
import "UnderTest/ui/js/JsonDelta.js" as JsonDelta
ShellRoot {
 function check(ok,msg){if(!ok){console.error("SNAPSHOT_DELTA_QML_FAIL "+msg);Qt.exit(1)}}
 ListModel{id:model;dynamicRoles:true}
 Timer{interval:250;running:true;onTriggered:{var b=Test.T3Backend
  // Whole value, then a patch that changes one thread and keeps the others.
  var threads=[{key:"a/1",id:"1",environmentId:"a",title:"A",working:true},{key:"a/2",id:"2",environmentId:"a",title:"B"},{key:"b/1",id:"1",environmentId:"b",title:"C"}]
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:1,data:{threads:threads,environments:[{id:"a",state:"ready"}],working:1,stale:false}}))
  check(b.state.threads.length===3&&b.state.threads[2].title==="C","whole frame becomes the state")
  var kept=b.state.threads[1],other=b.state.threads[2],envs=b.state.environments
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:2,base:1,patch:{o:{threads:{k:"key",a:[{i:"a/1",p:{o:{title:{$:"A2"}},r:["working"]}},"a/2","b/1",{$:{key:"c/9",id:"9",environmentId:"c",title:"New"}}]},working:{$:0}}}}))
  check(b.state.threads.length===4&&b.state.threads[0].title==="A2"&&b.state.threads[0].working===undefined,"patched thread changes")
  check(b.state.threads[1]===kept&&b.state.threads[2]===other,"unchanged threads keep their identity")
  check(b.state.environments===envs&&b.state.working===0&&b.state.stale===false,"unchanged members kept, changed replaced")
  check(b.state.threads[3].title==="New","new thread added")
  // Reordering and removal.
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:3,base:2,patch:{o:{threads:{k:"key",a:["b/1","a/2"]}}}}))
  check(b.state.threads.length===2&&b.state.threads[0]===other&&b.state.threads[1]===kept,"reorder and removal keep identity")
  // A gap is not applied; the stream is asked once for the whole value.
  var before=b.state
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:9,base:8,patch:{o:{working:{$:5}}}}))
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:10,base:9,patch:{o:{working:{$:6}}}}))
  check(b.state===before&&b.streamResyncs["snapshot/7"]===true,"gap ignored and resync requested")
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:11,data:{threads:[],environments:[],working:0,stale:false}}))
  check(b.state.threads.length===0&&!b.streamResyncs["snapshot/7"],"whole frame after a gap resumes the stream")
  // A patch for an element the base does not hold is a gap too.
  b.receive(JSON.stringify({type:"snapshot",stream:7,seq:12,base:11,patch:{o:{threads:{k:"key",a:["zz/1"]}}}}))
  check(b.state.threads.length===0&&b.streamResyncs["snapshot/7"]===true,"unknown element requests resync")
  // Plain frames (no stream) still work.
  b.receive(JSON.stringify({type:"snapshot",data:{threads:[{key:"p/1",id:"1",environmentId:"p",title:"Plain"}],environments:[],working:0,stale:false}}))
  check(b.state.threads[0].title==="Plain","frames without a stream are whole values")
  // Empty patch: same content, new top-level object so bindings still refresh.
  var whole={rows:[{id:1}]},same=JsonDelta.apply(whole,{})
  check(same!==whole&&same.rows===whole.rows,"empty patch copies only the top level")
  // Model sync: unchanged rows are not reassigned; changed ones are.
  var changes=0;model.dataChanged.connect(function(){changes++})
  var r1={key:"x",title:"one"},r2={key:"y",title:"two"},st={keys:[],prints:{}}
  ModelSync.sync(model,[r1,r2],st,function(r){return r.key},function(r){return {row:r}})
  check(model.count===2&&model.get(1).row.title==="two","rows inserted")
  changes=0;ModelSync.sync(model,[r1,r2],st,function(r){return r.key},function(r){return {row:r}})
  check(changes===0,"same rows are not reassigned")
  ModelSync.sync(model,[r1,{key:"y",title:"two"}],st,function(r){return r.key},function(r){return {row:r}})
  check(changes===0,"equal content is not reassigned")
  ModelSync.sync(model,[{key:"y",title:"2"},r1],st,function(r){return r.key},function(r){return {row:r}})
  check(model.count===2&&model.get(0).row.title==="2"&&model.get(1).row.key==="x"&&changes>0,"moved and changed rows applied")
  ModelSync.sync(model,[r1],st,function(r){return r.key},function(r){return {row:r}})
  check(model.count===1&&st.keys.join()==="x","removed rows dropped")
  model.clear();ModelSync.sync(model,[r1,r2],st,function(r){return r.key},function(r){return {row:r}})
  check(model.count===2&&st.keys.join()==="x,y","external clear rebuilds")
  console.log("SNAPSHOT_DELTA_QML_PASS: whole and patch frames, identity reuse, gaps and resync, plain frames, model sync");Qt.quit()
 }}
}
