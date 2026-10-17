import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 function check(ok,msg){if(!ok){console.error("PROJECT_PREFERENCES_QML_FAIL "+msg);Qt.exit(1)}}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,loader=keys.findChild(panel,"t3PreferencesView"),view=loader?loader.item:null
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"local/one",environmentId:"local",title:"One"}],projects:[{id:"p",key:"remote/p",environmentId:"remote",title:"Remote project",workspaceRoot:"/remote/repo"}],environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Remote",state:"ready"}]};fixture=b.state;b.selectThread("local","one");panel.open();panel.openProjectSettings("remote/p");phase=1}
  else if(phase===1){check(view&&view.page==="projects","project action opens native-style settings page");check(view.environment==="remote"&&view.checkoutKey==="remote/p","remote checkout remains explicit");check(view.scope.targets.length===1&&view.scope.targets[0].environment==="remote"&&view.scope.targets[0].projectId==="p","cross-chat action never inherits unrelated local environment");check(b.selectedKey==="local/one","opening project settings preserves current chat");view.closed();panel.close();console.log("PROJECT_PREFERENCES_QML_PASS: project settings route retains exact remote target and current chat");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("PROJECT_PREFERENCES_QML_FAIL timeout");Qt.exit(1)}}
}
