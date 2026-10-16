import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_PERMISSIONS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,p=keys.findChild(panel.composerView,"t3PermissionPicker")
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One",runtimeMode:"approval-required",modelSelection:{instanceId:"personal",model:"model"}},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two",runtimeMode:"auto"}],projects:[],environments:[]};b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[{instanceId:"personal",supportedRuntimeModes:["approval-required","full-access"]}],models:[],favorites:[]}};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Keep this prompt");check(p&&p.enabled&&p.mode==="approval-required","native inherited mode");keys.mouseClick(p,15,15);phase=2
  }else if(phase===2){check(p.popup.visible&&p.modes.length===2,"provider filters unsupported modes");keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){check(!p.popup.visible&&b.draft.runtimeMode==="full-access"&&b.selectedInfo.runtimeMode==="approval-required","keyboard selects deferred permission");check(b.draft.text==="Keep this prompt","selection preserves prompt");b.chooseRuntimeMode("auto");check(b.draft.runtimeMode==="full-access","unsupported mode rejected");b.patchDraft(b.selectedKey,{runtimeMode:"auto"});check(p.mode==="approval-required"&&b.draft.runtimeMode==="auto","legacy unsupported selection displays safe fallback without mutation");b.patchDraft(b.selectedKey,{runtimeMode:"full-access"});b.patchDraft("fixture/two",{text:"Other prompt",loading:false,attachments:[]});b.selectThread("fixture","two");phase=4
  }else if(phase===4){check(p.mode==="auto","another thread inherits its own mode");b.selectThread("fixture","one");phase=5
  }else if(phase===5){check(p.mode==="full-access"&&b.draft.text==="Keep this prompt","return restores permission and prompt");b.catalogs={fixture:{canWrite:true,providers:[{instanceId:"personal",supportedRuntimeModes:[]}],models:[],favorites:[]}};phase=6
  }else if(phase===6){check(p.modes.length===4,"older provider catalog exposes four native choices");p.choose("auto-accept-edits");check(b.draft.runtimeMode==="auto-accept-edits","new mode selected");b.submit();check(b.draft.attempt.runtimeMode==="auto-accept-edits"&&!p.enabled,"permission travels with locked send intent");var request=b.draft.attempt.requestId;fixture=Object.assign({},fixture,{threads:fixture.threads.map(function(t){return t.id==="one"?Object.assign({},t,{runtimeMode:"auto-accept-edits"}):t})});b.action({op:"send",environment:"fixture",thread:"one",requestId:request,result:{}});check(!b.draft.runtimeMode&&b.selectedInfo.runtimeMode==="auto-accept-edits","successful send reconciles inherited mode and clears override");panel.close();console.log("T3_PERMISSIONS_QML_PASS: filtered native modes, keyboard selection, per-thread draft, deferred send and successful reconciliation");Qt.quit()}
 }}
}
