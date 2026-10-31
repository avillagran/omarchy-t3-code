import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("SHARED_DRAFTS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 FloatingWindow{id:window;visible:false;implicitWidth:1000;implicitHeight:750
  Test.T3Preferences{id:preferences;anchors.fill:parent;bridge:Test.T3Backend;page:"connections";query:"Shared drafts"}
 }
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;check(b.viewState.sharedDraftsEnabled!==true,"sharing off by default");b.state={threads:[{key:"fixture/a",id:"a",environmentId:"fixture",projectId:"p",title:"A"}],projects:[],environments:[]};b.selectThread("fixture","a");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Keep this local text");b.flushDrafts();b.viewState=Object.assign({},b.viewState,{sharedDraftsEnabled:true,sharedDraftsAuthority:"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"});b.sharedDraftStatus={"fixture/a":{conflict:true,remote:{payload:{text:"Other device"}}}};phase=2
  }else if(phase===2){var c=panel.composerView;check(keys.findChild(c,"sharedDraftKeepLocal").visible,"local resolution visible");check(keys.findChild(c,"sharedDraftUseRemote").visible,"remote resolution visible");check(b.draft.text==="Keep this local text","conflict preserves active editor");
    c.draftsOpen=true;b.projectDrafts={scope:b.projectDraftScope(),entries:[{id:"legacy-stash",prompt:"Local stash collision",attachmentCount:0,locked:true,conflict:true,sharedKey:"stash/fixture/legacy-stash",sharedScope:{}}],busy:false,error:""};
    var drawer=keys.findChild(c,"t3ProjectDrafts");check(drawer.entries.length===1&&drawer.entries[0].conflict,"project migration conflict retains its local branch");b.viewState=Object.assign({},b.viewState,{sharedDraftsEnabled:false});panel.close();window.visible=true;phase=3
  }else if(phase===3){var enable=keys.findChild(preferences,"sharedDraftEnable"),authority=keys.findChild(preferences,"sharedDraftAuthority");check(enable&&!enable.checked,"preferences sharing disabled");var remote=keys.findChild(preferences,"remoteDocumentsEnable");check(remote&&!remote.checked,"remote documents default off");remote.checked=true;remote.toggled();check(b.viewState.remoteDocumentsEnabled===true,"explicit remote consent");var epoch=b.viewState.remoteDocumentsConsent;check(!!epoch,"consent epoch created");remote.checked=false;remote.toggled();check(b.viewState.remoteDocumentsEnabled===false,"remote consent revoked");check(b.viewState.remoteDocumentsConsent!==epoch,"revocation rotates epoch");check(authority&&authority.enabled,"authority can be chosen while disabled");check(authority.text===b.viewState.sharedDraftsAuthority,"explicit authority displayed");b.viewState=Object.assign({},b.viewState,{sharedDraftsEnabled:true});phase=4
  }else if(phase===4){check(!keys.findChild(preferences,"sharedDraftAuthority").enabled,"authority cannot change mid-sharing");b.viewState=Object.assign({},b.viewState,{sharedDraftsEnabled:false});console.log("SHARED_DRAFTS_QML_PASS: composer conflict controls, editor retained, explicit authority and default-off preferences");Qt.quit()}
 }}
}
