import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 property var fields:[
  {key:"branchNamePrefix",label:"Branch name prefix",section:"source-control",kind:"text",value:"t3",maximumLength:256,multiline:false},
  {key:"sourceControlWritingStyle.customInstructions",label:"Writing instructions",section:"source-control",kind:"text",value:"Native instructions",maximumLength:16384,multiline:true},
  {key:"pullRequestMergeMethod",label:"Pull request merge method",section:"source-control",kind:"choice",value:"merge",choices:[{id:null,name:"Last used"},{id:"merge",name:"Merge"}]},
  {key:"sidebarAutoSettleAfterDays",label:"Auto-settle after inactivity (days)",section:"general",kind:"days",value:null,minimum:1,maximum:90}]
 function check(ok,msg){if(!ok){console.error("SOURCE_CONTROL_PREFERENCES_QML_FAIL "+msg);Qt.exit(1)}}
 // Retire the synthetic RPC ID after its injected reply; the isolated real worker
 // can report unknown_environment later and must not overwrite this fixture.
 function reply(b,error){b.action({op:"settings",requestId:b.settingsResult.requestId,result:{fields:fields},error:error || ""});b.settingsResult=Object.assign({},b.settingsResult,{requestId:"fixture-replied-"+b.settingsResult.requestId})}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,loader=keys.findChild(panel,"t3PreferencesView"),v=loader?loader.item:null
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"local/one",id:"one",environmentId:"local",title:"One"}],environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Node",state:"ready"}]};b.selectThread("local","one");panel.open();panel.preferencesOpen=true;phase=1
  }else if(phase===1){b.settingsResult={busy:false,error:"",environment:"local",fields:root.fields};v.page="source-control";phase=2
  }else if(phase===2){var e=keys.findChild(v,"preferenceText_branchNamePrefix");check(e&&e.visible,"source control text input");e.forceActiveFocus();e.selectAll();keys.keyClick(Qt.Key_X);phase=3
  }else if(phase===3){check(v.edits.branchNamePrefix==="x","typing stages preference");check(!b.settingsResult.busy,"typing never sends automatically");var select=keys.findChild(v,"preferenceChoice_pullRequestMergeMethod");select.activated(0);check(b.settingsResult.busy,"nullable choice sends native update");reply(b);phase=4
  }else if(phase===4){var e=keys.findChild(v,"preferenceText_branchNamePrefix");check(e.text==="x","unsaved text survives native snapshot");var save=keys.findChild(v,"savePreference_branchNamePrefix");check(save.enabled,"edited field can save");save.clicked();check(b.settingsResult.busy,"save dispatches");reply(b,"unreachable");phase=5
  }else if(phase===5){check(v.edits.branchNamePrefix==="x"&&!v.writable,"failed save preserves edit and exposes error");v.load();reply(b);v.page="general";phase=6
  }else if(phase===6){var spin=keys.findChild(v,"preferenceDays_sidebarAutoSettleAfterDays"),toggle=keys.findChild(v,"preferenceEnabled_sidebarAutoSettleAfterDays");check(spin&&!spin.enabled&&!toggle.checked,"null inactivity means disabled");toggle.checked=true;toggle.toggled();check(b.settingsResult.busy,"enable auto settle dispatches");reply(b);v.page="source-control";phase=7
  }else if(phase===7){var memo=keys.findChild(v,"preferenceInstructions_sourceControlWritingStyle.customInstructions");check(memo.visible,"multiline instructions visible");memo.forceActiveFocus();memo.selectAll();keys.keyClick(Qt.Key_A);keys.keyClick(Qt.Key_Return);keys.keyClick(Qt.Key_B);phase=8
  }else if(phase===8){check(v.edits["sourceControlWritingStyle.customInstructions"]==="a\nb","multiline instructions staged as plain text");keys.findChild(v,"savePreference_branchNamePrefix").clicked();root.fields=root.fields.map(function(f){return f.key==="branchNamePrefix"?Object.assign({},f,{value:"x"}):f});reply(b);phase=9
  }else if(phase===9){check(v.edits.branchNamePrefix===undefined&&!keys.findChild(v,"savePreference_branchNamePrefix").enabled,"successful save resets only acknowledged edit");check(v.edits["sourceControlWritingStyle.customInstructions"]==="a\nb","unrelated instructions stay staged");v.environment="remote";check(Object.keys(v.edits).length===0,"environment switch cannot reuse local edits");reply(b);v.closed();panel.close();console.log("SOURCE_CONTROL_PREFERENCES_QML_PASS: native choices, explicit save, draft preservation, failure feedback, nullable inactivity and environment isolation");Qt.quit()}
 }}
}
