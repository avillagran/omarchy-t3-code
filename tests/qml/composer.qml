import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
  id: root
  function check(ok, msg) { if (!ok) { console.error("T3_WRITE_QML_FAIL: " + msg); Qt.exit(1) } }
  function named(item, name) { if (item.objectName === name) return item; var found = null; var kids = item.children || []; for (var i=0;i<kids.length;i++) { found=named(kids[i],name); if(found)return found } return null }
  Test.T3Composer { id: composer; width: 760; bridge: Test.T3Backend; showOptions: true }
  Timer {
    interval: 700; running: true
    onTriggered: {
      var b=Test.T3Backend
      b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One",activeRunId:"fixture-run",modelSelection:{instanceId:"codex",model:"first",options:[{id:"reasoningEffort",value:"medium"}]}},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two",modelSelection:{instanceId:"codex",model:"second",options:[]}}]}
      b.catalogs={fixture:{models:[{instanceId:"codex",model:"first",name:"First",descriptors:[{id:"reasoningEffort",label:"Reasoning",type:"select",currentValue:"low",options:[{id:"low",label:"Low"},{id:"medium",label:"Medium"}]}]},{instanceId:"codex",model:"second",name:"Second",descriptors:[]}]}}
      b.selectThread("fixture","one"); b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/one",draft:{},error:null}})); b.edit("draft one\nline two")
      Qt.callLater(function() {
        root.check(root.named(composer,"t3ComposerEditor").text === "draft one\nline two", "editor draft binding")
        root.check(composer.modelIndex===0,"current model")
        root.check(root.named(composer,"t3Option_reasoningEffort").currentIndex===1,"current reasoning option")
        b.chooseOption("reasoningEffort","low")
        root.check(b.draft.modelSelection.options[0].value==="low","option writes draft")
        b.chooseModel(1)
        root.check(b.draft.modelSelection.model==="second","model writes draft")
        b.selectThread("fixture","two");b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/two",draft:{},error:null}}));b.edit("draft two")
        b.selectThread("fixture","one")
        root.check(b.draft.text === "draft one\nline two", "per-thread draft restoration")
        b.submit(); var attempt=b.draft.attempt
        root.check(!!attempt && b.draft.busy,"submission locks draft");root.check(attempt.delivery==="queue","active execution dispatches to queue")
        b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,error:"rpc_outcome_unknown"})
        root.check(!b.draft.busy && b.draft.attempt.requestId===attempt.requestId,"uncertain preserves retry identity")
        b.edit("changed"); root.check(b.draft.text === attempt.text,"uncertain draft immutable")
        b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,result:{sequence:1}})
        root.check(b.draft.text==="" && !b.draft.attempt,"acknowledgment clears draft")
        b.selectThread("fixture","two")
        root.check(b.draft.text==="draft two","other draft unaffected")
        console.log("T3_WRITE_QML_PASS: draft bindings, models/options, isolation and uncertain retry")
        Qt.quit()
      })
    }
  }
}
