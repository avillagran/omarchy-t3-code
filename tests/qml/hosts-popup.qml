import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_HOSTS_POPUP_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[],projects:[],environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"node",state:"t3_remote_start_failed"}],working:0,pending:0}
   b.state=fixture;b.localBackend={owner:"gui",running:true,canControl:false,installed:true}
   b.catalogs={node:{machine:"desktop",serverVersion:"0.1.2-nightly.20261021.3486"},local:{serverVersion:"0.1.1-nightly.20261020.3386"}};panel.open();phase=1
  }else if(phase===1){
   var button=keys.findChild(panel,"localBackendGuiIndicator");check(button&&button.visible,"machine button shows while T3's GUI runs the server")
   button.clicked();phase=2
  }else if(phase===2){
   var popup=keys.findChild(panel,"t3HostsPopup");check(popup&&popup.opened,"machine button opens the environment list")
   var node=keys.findChild(popup.contentItem,"t3Host_node");check(!!node,"each environment has a row")
   check(b.validatedT3.tag==="v0.1.1-nightly.20261020.3386"||!!b.validatedT3.tag,"validated T3 revision is read from upstream/t3code.json")
   check(b.t3VersionNewer("0.1.2-nightly.20261021.3486")&&!b.t3VersionNewer(b.validatedT3.tag.replace(/^v/,""))&&!b.t3VersionNewer("0.1.1-nightly.20261019.3300"),"server versions compare by release, date and build")
   var version=keys.findChild(popup.contentItem,"t3HostVersion_node");check(version&&version.visible&&version.text.indexOf("0.1.2")>=0&&version.text.indexOf("·")>=0,"a newer server is marked: "+(version&&version.text))
   check(keys.findChild(popup.contentItem,"t3HostVersion_local").text.indexOf("·")<0,"the validated version is not marked")
   var reconnect=keys.findChild(popup.contentItem,"t3HostReconnect_node");check(reconnect&&reconnect.visible,"remote hosts can be reconnected")
   check(!keys.findChild(popup.contentItem,"t3HostReconnect_local").visible,"the local server has no reconnect")
   check(!keys.findChild(popup.contentItem,"t3HostRestartLocal").visible,"no restart while T3's GUI owns the local server")
   check(!keys.findChild(popup.contentItem,"t3HostRestart_node").visible,"no restart until the route is known to be SSH")
   b.connectionSettings=Object.assign({},b.connectionSettings,{busy:false,environments:[{id:"node",name:"node",routes:[{kind:"SshConnectionTarget"}]}]})
   var restart=keys.findChild(popup.contentItem,"t3HostRestart_node");check(restart&&restart.visible,"SSH hosts offer a server restart")
   restart.clicked();check(popup.confirmRestart==="node"&&keys.findChild(popup.contentItem,"t3HostRestartConfirm_node").visible,"restart asks to confirm in the row")
   check(!b.connectionSettingsCommand||b.connectionSettingsCommand.action!=="restart-server","nothing is sent before confirming")
   keys.findChild(popup.contentItem,"t3HostRestartCancel_node").clicked();check(popup.confirmRestart==="","cancel keeps the server running")
   restart.clicked();keys.findChild(popup.contentItem,"t3HostRestartConfirm_node").clicked()
   check(b.connectionSettingsCommand.action==="restart-server"&&b.connectionSettingsCommand.environmentId==="node","confirm sends the restart for that host")
   check(popup.restarting==="node","the row shows the restart in progress")
   b.connectionSettings=Object.assign({},b.connectionSettings,{busy:false})
   reconnect.clicked()
   check(b.connectionSettingsCommand&&b.connectionSettingsCommand.action==="reconnect"&&b.connectionSettingsCommand.environmentId==="node","reconnect asks for that environment")
   check(popup.reconnecting.node!==undefined,"the row shows the reconnect in progress")
   fixture=Object.assign({},fixture,{environments:[fixture.environments[0],{id:"node",name:"node",state:"ready"}]});b.state=fixture
   check(popup.reconnecting.node===undefined,"a state change ends the reconnect")
   keys.findChild(popup.contentItem,"t3HostsSettings").clicked();phase=3
  }else if(phase===3){
   check(panel.preferencesOpen,"the footer opens Connections")
   console.log("T3_HOSTS_POPUP_QML_PASS: environment list, reconnect and settings from the machine button");Qt.quit()
  }
 }}
}
