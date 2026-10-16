import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PREFERENCES_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:250;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,loader=keys.findChild(panel,"t3PreferencesView"),view=loader?loader.item:null
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"local/one",id:"one",environmentId:"local",title:"One"}],environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Node",state:"ready"}]};b.selectThread("local","one");panel.open();keys.findChild(panel,"openPreferences").clicked();phase=1
  }else if(phase===1){check(view&&view.visible,"preferences open inside chat");check(!panel.sidebarView.visible,"settings replace chat sidebar instead of duplicating navigation");b.settingsResult={busy:false,error:"",environment:"local",fields:[{key:"enableProviderUpdateChecks",label:"Check provider updates",kind:"bool",value:true}]};phase=2
  }else if(phase===2){var field=keys.findChild(view,"preference_enableProviderUpdateChecks");check(field&&field.enabled&&field.checked,"native field rendered");field.checked=false;field.toggled();check(b.settingsResult.busy&&b.settingsResult.environment==="local","setting update targets selected server");b.action({op:"settings",requestId:b.settingsResult.requestId,result:{fields:[{key:"enableProviderUpdateChecks",label:"Check provider updates",kind:"bool",value:false}]}});check(!b.settingsResult.busy,"native action response applied");keys.findChild(view,"preferencesTab_appearance").clicked();phase=3
  }else if(phase===3){var usage=keys.findChild(view,"preferenceUsage");check(usage.visible,"appearance tab visible");usage.checked=true;usage.toggled();check(!panel.usageCollapsed,"usage preference updates panel state");keys.findChild(view,"preferencesTab_connections").clicked();view.environment="remote";check(b.settingsResult.busy&&b.settingsResult.fields.length===0,"environment switch clears stale fields");b.action({op:"settings",requestId:b.settingsResult.requestId,result:{fields:[]}});phase=4
  }else if(phase===4){check(view.writable&&view.environment==="remote","remote settings response accepted");view.closed();check(!panel.preferencesOpen,"back returns to chat");check(b.selectedThread==="one","selected chat preserved");phase=5
  }else if(phase===5){if(panel.restoringConversation)return;check(panel.sidebarView.visible&&panel.conversationView.visible,"back restores chat and original sidebar");panel.close();console.log("PREFERENCES_QML_PASS: embedded navigation, native settings update, appearance state, environment isolation and return to chat");Qt.quit()}
 }}
}
