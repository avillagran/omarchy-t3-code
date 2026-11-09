import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client

ShellRoot {
 id: root
 readonly property var bridge: Client.T3Backend
 property var catalogs: ({})
 property bool reading: false
 property int phase: 0
 property double phaseStarted: Date.now()
 function check(value, message) { if (!value) throw Error(message) }
 function matches(name, label, rate) {
  var catalog=catalogs[name]
  return catalog && catalog.defaultProfileId==='work' && catalog.recordingFrameRate===rate
   && catalog.profiles.length===3 && catalog.profiles[0].id==='default'
   && catalog.profiles[1].id==='incognito' && catalog.profiles[1].incognito===true
   && catalog.profiles[2].id==='work' && catalog.profiles[2].name===label
 }
 function deniedUnchanged() {
  var value=catalogs.denied
  return value && value.profiles.length===2 && value.defaultProfileId===undefined
   && value.recordingFrameRate===30
 }
 function next() { phase++;phaseStarted=Date.now() }
 Timer {interval:40;repeat:true;running:true;onTriggered:{
  if(root.reading)return
  root.reading=true
  var request=new XMLHttpRequest()
  request.onreadystatechange=function(){
   if(request.readyState!==XMLHttpRequest.DONE)return
   root.reading=false
   if((request.status===0||request.status===200)&&request.responseText)
    root.catalogs=JSON.parse(request.responseText)
  }
  request.open('GET','file:///evidence/catalog.json');request.send()
 }}
 Timer {interval:40;repeat:true;running:true;onTriggered:{try{
  var b=root.bridge
  check(!b.panelVisible&&!b.previewVisible&&!b.selectedThread,
   'production singleton remains hidden and unselected')
  check(Date.now()-root.phaseStarted<12000,'phase deadline '+root.phase)
  if(root.phase===0){
   if(!matches('a','Work',30)||!matches('b','Work',30))return
   check(deniedUnchanged(),'actual missing preview permission leaves denied catalog unchanged')
   check(b.profileReportTargets.size===2,'exactly the two actual admitted write targets')
   check(b.viewStateLoaded&&b.clientSettings.data.browserDefaultProfileId==='work',
    'actual client preferences hydrate before initial report')
   b.requestClientSettings({browserRecordingFrameRate:60});next()
  }else if(root.phase===1){
   if(b.clientSettings.busy||!matches('a','Work',60)||!matches('b','Work',60))return
   check(!b.clientSettings.error&&deniedUnchanged(),'real CAS and both report RPCs succeed')
   b.requestClientSettings({browserProfiles:[{id:'work',name:'Work <updated>',kind:'persistent'}]});next()
  }else if(root.phase===2){
   if(b.clientSettings.busy||!matches('a','Work <updated>',60)||!matches('b','Work <updated>',60))return
   check(!b.clientSettings.error&&deniedUnchanged(),'real profile replacement reaches both actual catalogs')
   next()
  }else if(root.phase===3){
   // Let the normal watch/snapshot cycle run without a fixture protocol event.
   if(Date.now()-root.phaseStarted<3500)return
   check(matches('a','Work <updated>',60)&&matches('b','Work <updated>',60)&&deniedUnchanged(),
    'real background snapshots retain both reports and denied isolation')
   console.log('BROWSER_PROFILE_CHAIN_QML_PASS '+JSON.stringify({actualServers:3,
    allowedEnvironments:2,deniedEnvironment:1,hidden:true,initialReport:true,
    nativePreferenceCas:true,frameRateUpdate:true,profileReplacement:true,
    scope:'Actual QML/CLI watch/HTTP auth/WebSocket RPC/server catalog; no transport peer, synthetic target or action receipt'}))
   Qt.quit()
  }
 }catch(error){console.error('BROWSER_PROFILE_CHAIN_QML_FAIL '+error+' '+error.stack);Qt.exit(1)}}}
 Timer {interval:35000;running:true;onTriggered:{console.error('BROWSER_PROFILE_CHAIN_QML_FAIL total deadline');Qt.exit(1)}}
}
