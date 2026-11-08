import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Client

ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 readonly property string usb:"opaque-usb:input/alpha?channel=2"
 property var waiting:[]
 property string session:""
 FloatingWindow {id:window;visible:true;implicitWidth:820;implicitHeight:1000
  Client.T3NativeVoiceSettings {id:settings;x:12;y:210;width:parent.width-24;bridge:root.bridge}
 }
 TestCase {id:keys;parent:window.contentItem;when:false}
 function check(value,message){if(!value)throw Error(message)}
 function item(name){var value=keys.findChild(window.contentItem,name);check(!!value,"actual control "+name);return value}
 function waitFor(predicate,message){return new Promise(function(resolve,reject){root.waiting=root.waiting.concat([{predicate:predicate,message:message,resolve:resolve,reject:reject,until:Date.now()+4000}])})}
 Timer {interval:15;repeat:true;running:true;onTriggered:{var next=[];root.waiting.forEach(function(wait){try{if(wait.predicate())wait.resolve();else if(Date.now()>wait.until)wait.reject(Error(wait.message));else next.push(wait)}catch(error){wait.reject(error)}});root.waiting=next}}
 function choose(index,id){
  var picker=item('nativeVoiceMicrophone');keys.mouseClick(picker)
  return waitFor(function(){return picker.popup.visible&&!!picker.popup.contentItem.itemAtIndex(index)},'actual selector popup').then(function(){
   var row=picker.popup.contentItem.itemAtIndex(index)
   check(row.contentItem.textFormat===Text.PlainText,'native device labels stay plain text')
   keys.mouseClick(row,row.width/2,row.height/2)
   return waitFor(function(){return !picker.popup.visible&&!bridge.clientSettings.busy&&bridge.nativeVoicePreferences.voiceMicrophone===id},'actual preference CAS selection')
  })
 }
 function run(){
  var b=bridge
  b.state={threads:[{id:'one',key:'env/one',environmentId:'env',projectId:'project'}],environments:[{id:'env',name:'Fixture environment',state:'ready'}],projects:[]}
  b.selectedEnvironment='env';b.selectedThread='one';b.selectedKey='env/one'
  b.drafts={'env/one':{text:'first',loading:false,busy:false,attachments:[],attempt:null}}
  b.rememberDraftCursor(5,5);window.contentItem.Window.window.requestActivate()
  return waitFor(function(){return b.nativeVoiceAvailable&&b.nativeVoicePreferences!==null&&b.clientSettings.data.revision==='10'&&window.contentItem.Window.window.active},'real Backend and settings ready').then(function(){
   check(!item('nativeVoiceMicrophonesRefresh').enabled&&!b.nativeVoiceMicrophonesActive,'idle refresh unavailable')
   check(!b.nativeVoiceChoice&&!b.nativeVoicePermission&&!b.voiceController,'settings mount grants no audio/destination')
   return b.refreshNativeVoiceMicrophones()
  }).then(function(value){
   check(value===false&&!b.voiceController&&!b.nativeVoicePermission,'idle refresh cannot start capture or permission')
   settings.chooseDestination('environment:env')
   return waitFor(function(){return b.nativeVoiceChoice&&!settings.consentBusy},'explicit destination')
  }).then(function(){b.toggleNativeVoice();return waitFor(function(){return !!b.nativeVoicePermission},'explicit microphone permission')}).then(function(){
   root.session=b.nativeVoicePermission.session;b.decideNativeVoicePermission(true)
   return waitFor(function(){return b.voiceState.phase==='recording'&&b.nativeVoiceMicrophonesActive&&b.nativeVoiceMicrophones.length===3&&item('nativeVoiceMicrophonesRefresh').enabled},'already permitted active capture')
  }).then(function(){return choose(2,usb)}).then(function(){
   check(b.clientSettings.data.revision==='11','saved microphone uses original CAS')
   keys.mouseClick(item('nativeVoiceMicrophonesRefresh'))
   return waitFor(function(){return b.nativeVoiceMicrophonesRefreshing&&!item('nativeVoiceMicrophonesRefresh').enabled},'busy prevents duplicate refresh')
  }).then(function(){
   return waitFor(function(){return !b.nativeVoiceMicrophonesRefreshing&&b.nativeVoiceMicrophones[0].label==='Refreshed line input'},'metadata-only peer refresh reaches current capture')
  }).then(function(){
   var picker=item('nativeVoiceMicrophone')
   check(b.voiceState.phase==='recording'&&picker.currentIndex===3&&picker.contentItem.text==='<em>Refreshed USB & literal</em>'&&picker.contentItem.textFormat===Text.PlainText,'refreshed order preserves exact ID and literal label')
   check(b.clientSettings.data.revision==='11','refresh does not mutate preference revision')
   keys.mouseClick(item('nativeVoiceMicrophonesRefresh'))
   return waitFor(function(){return !b.nativeVoiceMicrophonesRefreshing&&!!b.nativeVoiceMicrophonesError},'enumeration error is visible')
  }).then(function(){
   check(b.voiceState.phase==='recording'&&item('nativeVoiceMicrophonesRefresh').enabled,'metadata error preserves recorder and allows retry')
   return b.voiceController.stop()
  }).then(function(){
   return waitFor(function(){return b.voiceState.phase==='idle'&&!b.nativeVoiceMicrophonesActive&&!item('nativeVoiceMicrophonesRefresh').enabled},'stop deactivates refresh')
  }).then(function(){
   var accepted=JSON.stringify(b.nativeVoiceMicrophones),picker=item('nativeVoiceMicrophone')
   check(b.nativeVoiceMicrophones.length===3&&picker.enabled&&picker.currentIndex===3,'accepted options remain selectable between recordings')
   b.nativeVoiceHost.receive({kind:'microphones',session:root.session,devices:[{deviceId:'stale-after-stop',label:'Stale'}]})
   check(JSON.stringify(b.nativeVoiceMicrophones)===accepted,'stale event after stop cannot overwrite cache')
   return b.refreshNativeVoiceMicrophones()
  }).then(function(value){check(value===false,'idle backend refresh remains inert');return choose(0,'')}).then(function(){
   check(b.clientSettings.data.revision==='12'&&item('nativeVoiceMicrophone').currentIndex===0,'next microphone can be selected while idle')
   b.toggleNativeVoice()
   return waitFor(function(){return !!b.nativeVoicePermission&&b.nativeVoicePermission.session!==root.session},'same-owner next recording has new explicit permission')
  }).then(function(){
   b.decideNativeVoicePermission(true)
   return waitFor(function(){return b.voiceState.phase==='recording'&&b.nativeVoiceMicrophonesActive&&!!b.nativeVoiceMicrophonesError},'initial metadata unavailable status after successful audio preparation')
  }).then(function(){
   check(b.nativeVoiceMicrophones.length===3&&b.nativeVoiceMicrophones[0].label==='Refreshed line input'&&item('nativeVoiceMicrophone').count===4,'failed initial enumeration preserves last three accepted devices')
   keys.mouseClick(item('nativeVoiceMicrophonesRefresh'))
   return waitFor(function(){return !b.nativeVoiceMicrophonesRefreshing&&b.nativeVoiceMicrophones.length===0&&!b.nativeVoiceMicrophonesError},'successful empty enumeration replaces cached list')
  }).then(function(){
   check(b.voiceState.phase==='recording'&&item('nativeVoiceMicrophone').count===1,'true empty list does not stop capture')
   return b.voiceController.stop()
  }).then(function(){
   b.releaseVoiceController()
   console.log('VOICE_MICROPHONES_REFRESH_QML_PASS '+JSON.stringify({manualRefreshes:3,explicitPermissions:2,casRevisions:['10','11','12'],idleCacheSelectable:true,lateAfterStopIgnored:true,failedInitialRetainsCache:true,genuineEmptyReplacesCache:true,plainLabels:true,scope:'Actual Backend/Settings/Host/controller with metadata-only peer; no native Rust/hardware/devicechange/PCM acceptance'}))
   Qt.quit()
  })
 }
 Timer {interval:350;running:true;onTriggered:root.run().catch(function(error){console.error('VOICE_MICROPHONES_REFRESH_QML_FAIL '+error+' '+error.stack);Qt.exit(1)})}
 Timer {interval:18000;running:true;onTriggered:{console.error('VOICE_MICROPHONES_REFRESH_QML_FAIL deadline');Qt.exit(1)}}
}
