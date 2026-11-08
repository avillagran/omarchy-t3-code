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
 property string oldSession:""
 FloatingWindow {id:window;visible:true;implicitWidth:820;implicitHeight:1000
  Client.T3NativeVoiceSettings {id:settings;x:12;y:210;width:parent.width-24;bridge:root.bridge}
 }
 TestCase {id:keys;parent:window.contentItem;when:false}
 function check(value,message){if(!value)throw Error(message)}
 function item(name){var result=keys.findChild(window.contentItem,name);check(!!result,"actual control "+name);return result}
 function waitFor(predicate,message){return new Promise(function(resolve,reject){root.waiting=root.waiting.concat([{predicate:predicate,message:message,resolve:resolve,reject:reject,until:Date.now()+4000}])})}
 Timer {interval:15;repeat:true;running:true;onTriggered:{var next=[];root.waiting.forEach(function(wait){try{if(wait.predicate())wait.resolve();else if(Date.now()>wait.until)wait.reject(Error(wait.message));else next.push(wait)}catch(error){wait.reject(error)}});root.waiting=next}}
 function chooseMicrophone(index,expected){
  var picker=item('nativeVoiceMicrophone')
  keys.mouseClick(picker)
  return waitFor(function(){return picker.popup.visible&&!!picker.popup.contentItem.itemAtIndex(index)},'real microphone popup delegate').then(function(){
   var row=picker.popup.contentItem.itemAtIndex(index)
   check(row.contentItem.text===picker.model[index].name&&row.contentItem.textFormat===Text.PlainText,'popup device label is literal plain text')
   keys.mouseClick(row,row.width/2,row.height/2)
   return waitFor(function(){return !picker.popup.visible&&!bridge.clientSettings.busy&&bridge.nativeVoicePreferences.voiceMicrophone===expected},'selection reaches native preference CAS boundary')
  })
 }
 function run(){
  var b=bridge
  b.state={threads:[{id:'one',key:'env/one',environmentId:'env',projectId:'project'},{id:'two',key:'env/two',environmentId:'env',projectId:'project'}],environments:[{id:'env',name:'Fixture environment',state:'ready'}],projects:[]}
  b.selectedEnvironment='env';b.selectedThread='one';b.selectedKey='env/one'
  b.drafts={'env/one':{text:'first',loading:false,busy:false,attachments:[],attempt:null},'env/two':{text:'second',loading:false,busy:false,attachments:[],attempt:null}}
  b.rememberDraftCursor(5,5)
  window.contentItem.Window.window.requestActivate()
  return waitFor(function(){return b.nativeVoiceAvailable&&b.nativeVoicePreferences!==null&&b.clientSettings.data.revision==='10'&&window.contentItem.Window.window.active},'actual Backend/Settings ready').then(function(){
   var picker=item('nativeVoiceMicrophone')
   check(b.nativeVoiceChoice===null&&b.nativeVoiceCleanupGrant===null&&!b.nativeVoicePermission&&!b.voiceController,'mounting settings grants no destination or microphone')
   check(settings.microphoneChoices.length===1&&picker.count===1&&picker.model[0].id===''&&b.nativeVoiceMicrophones.length===0,'unowned initial metadata cannot populate choices')
   settings.chooseDestination('environment:env')
   return waitFor(function(){return b.nativeVoiceChoice&&b.nativeVoiceChoice.destination.environmentId==='env'&&!settings.consentBusy},'explicit destination choice')
  }).then(function(){
   b.toggleNativeVoice()
   return waitFor(function(){return !!b.nativeVoicePermission},'explicit first permission boundary')
  }).then(function(){
   root.oldSession=b.nativeVoicePermission.session
   check(b.nativeVoiceMicrophones.length===0,'permission request alone publishes no device list')
   b.decideNativeVoicePermission(true)
   return waitFor(function(){return b.voiceState.phase==='recording'&&b.nativeVoiceMicrophones.length===3},'current scope metadata through actual Host')
  }).then(function(){
   var picker=item('nativeVoiceMicrophone')
   check(settings.microphoneChoices.length===4&&picker.count===4&&picker.model[0].id===''&&picker.model[1].id==='default'&&picker.model[2].id===usb,'system default and exact opaque device IDs')
   check(picker.model[2].name==='<b>USB & "quoted"</b>'&&!picker.translateLabels,'device names remain data')
   return chooseMicrophone(2,usb)
  }).then(function(){
   var picker=item('nativeVoiceMicrophone')
   check(b.clientSettings.data.revision==='11'&&picker.currentIndex===2&&picker.contentItem.text==='<b>USB & "quoted"</b>'&&picker.contentItem.textFormat===Text.PlainText,'actual selected content and CAS revision')
   b.selectedThread='two';b.selectedKey='env/two'
   return waitFor(function(){return b.nativeVoiceAvailable&&!b.nativeVoicePermission&&b.voiceState.phase==='idle'},'original controller idle after owner change')
  }).then(function(){
   b.toggleNativeVoice()
   return waitFor(function(){return !!b.nativeVoicePermission&&b.nativeVoicePermission.session!==root.oldSession},'successor permission has another scope')
  }).then(function(){
   b.decideNativeVoicePermission(true)
   return waitFor(function(){return b.voiceState.phase==='recording'&&b.nativeVoiceMicrophones.length===3&&b.nativeVoiceMicrophones[0].label==='Successor line input'},'successor list survives old transport event')
  }).then(function(){
   var picker=item('nativeVoiceMicrophone'),before=JSON.stringify(b.nativeVoiceMicrophones)
   check(picker.currentIndex===3&&picker.model[3].id===usb&&picker.contentItem.text==='<i>USB successor & literal</i>'&&picker.contentItem.textFormat===Text.PlainText,'saved ID follows current reordered list with plain label')
   b.nativeVoiceHost.receive({kind:'microphones',session:root.oldSession,devices:[{deviceId:'stale-direct',label:'<b>STALE</b>'}]})
   b.nativeVoiceHost.receive({kind:'microphones',session:'never-owned',devices:[{deviceId:'foreign-direct',label:'FOREIGN'}]})
   check(JSON.stringify(b.nativeVoiceMicrophones)===before,'stale and foreign scope cannot replace current list')
   return chooseMicrophone(0,'')
  }).then(function(){
   check(b.clientSettings.data.revision==='12'&&item('nativeVoiceMicrophone').currentIndex===0,'explicit system default is stored as original empty ID')
   b.releaseVoiceController()
   console.log('VOICE_MICROPHONES_QML_PASS '+JSON.stringify({devices:3,choices:4,casRevisions:['10','11','12'],plainLabels:true,staleOwnerRejected:true,scope:'Actual Backend/Settings/Host/platform/controller with metadata-only transport peer; no audio hardware, PCM, network or Rust CAS execution'}))
   Qt.quit()
  })
 }
 Timer {interval:350;running:true;onTriggered:root.run().catch(function(error){console.error('VOICE_MICROPHONES_QML_FAIL '+error+' '+error.stack);Qt.exit(1)})}
 Timer {interval:18000;running:true;onTriggered:{console.error('VOICE_MICROPHONES_QML_FAIL deadline');Qt.exit(1)}}
}
