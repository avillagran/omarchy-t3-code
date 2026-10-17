import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeKeybindings.js" as Native
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property string readId:""
 function check(ok,msg){if(!ok){console.error("KEYBINDINGS_RUNTIME_QML_FAIL "+msg);Qt.exit(1);throw new Error(msg)}}
 function rules(input){return Native.nativeKeys.compileResolvedKeybindingsConfig(input)}
 function config(env,input){return {busy:false,error:"",data:{environment:env,originKey:"origin-"+env,viewId:"view-"+env,revision:"first",writable:true,keybindings:rules(input)}}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,r=panel.nativeShortcutController
  if(phase===0){if(!b.viewStateLoaded)return;fixture={primaryEnvironmentId:"fixture",environments:[{id:"fixture",state:"ready"},{id:"other",state:"ready"}],projects:[{id:"p",key:"other/p",environmentId:"other",title:"Repo"}],threads:[{id:"one",key:"other/one",environmentId:"other",projectId:"p",projectKey:"other/p",title:"First"},{id:"two",key:"other/two",environmentId:"other",projectId:"p",projectKey:"other/p",title:"Second"}]};b.state=fixture;b.selectThread("other","one");panel.open();b.setPanelVisible(true);phase=1}
  else if(phase===1){if(b.draft.loading)return;b.patchDraft(b.selectedKey,{text:"Preserved local draft",attachments:[],loading:false,busy:false,attempt:null});b.keybindingConfigs={fixture:config("fixture",[{command:"sidebar.toggle",key:"mod+alt+b"},{command:"thread.next",key:"mod+alt+n"}]),other:config("other",[{command:"sidebar.toggle",key:"mod+alt+y"},{command:"script.check.run",key:"mod+alt+j"}])};phase=2}
  else if(phase===2){
   check(b.primaryKeybindingEnvironment==="fixture","snapshot identifies native primary server");check(r.claims.some(function(c){return c.command==="sidebar.toggle"&&c.shortcut.key==="b"&&c.source==="primary"}),"application keys come from primary");check(!r.claims.some(function(c){return c.shortcut.key==="y"}),"active server cannot override app keys");check(r.claims.some(function(c){return c.command==="script.check.run"&&c.source==="active"}),"script keys come from active server");
   var collapsed=panel.sidebarCollapsed;r.triggered("sidebar.toggle");check(panel.sidebarCollapsed!==collapsed,"native route dispatches existing sidebar action");r.triggered("thread.next");check(b.selectedThread==="two"&&b.drafts["other/one"].text==="Preserved local draft","thread shortcut preserves original draft");
   panel.preferencesOpen=true;collapsed=panel.sidebarCollapsed;r.triggered("sidebar.toggle");check(panel.sidebarCollapsed===collapsed&&!r.active,"preferences suppress chat actions");panel.preferencesOpen=false;b.keybindingCaptureActive=true;r.triggered("sidebar.toggle");check(panel.sidebarCollapsed===collapsed&&!r.active,"capture suppresses chat actions");
   var nonce=b.keybindingWatchRequest;check(b.keybindingWatchEnvironments.join(",")==="fixture,other","only visible primary/active environments watched "+JSON.stringify({ids:b.keybindingWatchEnvironments,scope:b.keybindingScope,selected:b.selectedEnvironment,visible:b.panelVisible}));b.invalidateKeybindings({environment:"fixture",requestId:"stale"});b.invalidateKeybindings({environment:"unrelated",requestId:nonce});check(!Object.keys(b.keybindingInvalidations).length,"late and unrelated stream events ignored");b.invalidateKeybindings({environment:"fixture",requestId:nonce});check(b.keybindingInvalidations.fixture,"current native update is invalidation only");phase=3
  }
  else if(phase===3){check(b.keybindingInvalidations.fixture,"update deferred while recording");b.keybindingCaptureActive=false;phase=4}
  else if(phase===4){check(!b.keybindingInvalidations.fixture,"recording end drains native invalidation");panel.close();b.setPanelVisible(false);check(!b.keybindingWatchEnvironments.length,"hidden panel has no subscriptions");b.invalidateKeybindings({environment:"fixture",requestId:b.keybindingWatchRequest});check(!Object.keys(b.keybindingInvalidations).length,"late hidden event cannot read or install config");console.log("KEYBINDINGS_RUNTIME_QML_PASS: native primary versus active rules, existing action dispatch/draft preservation, preferences/capture guards, event nonce/scope, capture deferral and hidden cancellation");Qt.quit()}
 }}
 Timer{interval:9000;running:true;onTriggered:{console.error("KEYBINDINGS_RUNTIME_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
