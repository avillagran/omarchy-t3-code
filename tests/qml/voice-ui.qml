import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Client
ShellRoot{
 id:root
 readonly property var bridge:Client.T3Backend
 property var waiting:[]
 FloatingWindow{id:window;visible:true;implicitWidth:820;implicitHeight:1000
  Column{width:parent.width
   Client.T3Composer{id:composer;width:parent.width;bridge:root.bridge}
   Client.T3NativeVoiceSettings{id:settings;width:parent.width;bridge:root.bridge}
  }
 }
 TestCase{id:keys;parent:window.contentItem;when:false}
 function check(value,message){if(!value)throw Error(message)}
 function item(name){var value=keys.findChild(composer,name)||keys.findChild(window.contentItem,name);check(!!value,"actual item "+name);return value}
 function waitFor(predicate,message){return new Promise(function(resolve,reject){root.waiting=root.waiting.concat([{predicate:predicate,message:message,resolve:resolve,reject:reject,until:Date.now()+4000}])})}
 Timer{interval:15;repeat:true;running:true;onTriggered:{var next=[];root.waiting.forEach(function(wait){try{if(wait.predicate())wait.resolve();else if(Date.now()>wait.until)wait.reject(Error(wait.message));else next.push(wait)}catch(error){wait.reject(error)}});root.waiting=next}}
 function patch(key,value){settings.patch(key,value);return waitFor(function(){return !root.bridge.clientSettings.busy&&JSON.stringify(root.bridge.nativeVoicePreferences[key])===JSON.stringify(value)},"native preference "+key)}
 function run(){
  var b=bridge
  b.state={threads:[{id:"one",key:"env/one",environmentId:"env",projectId:"project"},{id:"two",key:"env/two",environmentId:"env",projectId:"project"}],environments:[{id:"env",name:"<b>Private environment</b>",state:"ready"}],projects:[]}
  b.selectedEnvironment="env";b.selectedThread="one";b.selectedKey="env/one"
  b.drafts={"env/one":{text:"hello ",loading:false,busy:false,attachments:[],attempt:null},"env/two":{text:"other",loading:false,busy:false,attachments:[],attempt:null}}
  b.rememberDraftCursor(6,6);b.resetUndo(b.selectedKey)
  window.contentItem.Window.window.requestActivate()
  return waitFor(function(){return b.nativeVoiceAvailable&&b.nativeVoicePreferences!==null&&composer.inputField.Window.window.active},"actual window and native metadata ready").then(function(){
   check(b.nativeVoiceChoice===null&&b.nativeVoiceCleanupGrant===null,"constructing settings grants nothing")
   check(item('t3ComposerDictate').visible&&composer.nativeVoiceMode,"actual native mic route")
   keys.mouseClick(item('t3ComposerDictate'))
   return waitFor(function(){return item('nativeVoiceSettingsPopup').visible},"mic with no choice opens settings")
  }).then(function(){
   check(b.voiceState.phase==='idle'&&!b.nativeVoicePermission,"settings never opens capture")
   settings.chooseDestination('environment:env')
   return waitFor(function(){return b.nativeVoiceChoice&&b.nativeVoiceChoice.destination.environmentId==='env'},"explicit environment consent")
  }).then(function(){
   item('nativeVoiceSettingsClose').clicked()
   settings.changeConsent(function(){return b.nativeVoiceHost.chooseCleanup(true)})
   return waitFor(function(){return !!b.nativeVoiceCleanupGrant&&!settings.consentBusy},"explicit cleanup consent")
  }).then(function(){
   settings.changeConsent(function(){return b.nativeVoiceHost.chooseCleanup(false)})
   return waitFor(function(){return !b.nativeVoiceCleanupGrant&&!settings.consentBusy},"cleanup revoked separately")
  }).then(function(){return patch('speechLanguage','es')})
   .then(function(){return patch('speechTranslateToEnglish',true)})
   .then(function(){return patch('voiceMicrophone','')})
   .then(function(){return patch('speechSilenceFiltering',false)})
   .then(function(){return patch('speechRemoveFillerWords',false)})
   .then(function(){return patch('speechPostProcessingPrompt',{mode:'custom',customInstructions:'Keep product names.'})})
   .then(function(){return patch('speechCorrectionWord','correction')})
   .then(function(){settings.saveWords([{term:'Omarchy',aliases:['Oh marchy']}]);return waitFor(function(){return !b.clientSettings.busy&&b.nativeVoicePreferences.speechCustomWords.length===1},'structured dictionary update')})
   .then(function(){settings.renameAlias('Omarchy','Oh marchy','O marchy');return waitFor(function(){return !b.clientSettings.busy&&b.nativeVoicePreferences.speechCustomWords[0].aliases[0]==='O marchy'},'structured alias editing')})
   .then(function(){
    check(settings.microphoneChoices.length===1&&settings.microphoneChoices[0].id==='',"only actual system default, no invented device IDs")
    composer.inputField.cursorPosition=6;b.rememberDraftCursor(6,6)
    keys.mouseClick(item('t3ComposerDictate'))
    return waitFor(function(){return !!b.nativeVoicePermission&&item('nativeVoiceMicrophonePermission').visible},"actual microphone permission modal")
   }).then(function(){
    check(item('nativeVoicePermissionBlock').activeFocus,"microphone permission defaults to Block")
    check(item('nativeVoicePermissionDestination').text.indexOf('<b>Private environment</b>')>=0&&item('nativeVoicePermissionDestination').textFormat===Text.PlainText,"real destination is plain text")
    keys.keyClick(Qt.Key_Escape)
    return waitFor(function(){return !b.nativeVoicePermission&&b.voiceState.phase==='error'},"Escape denies permission before recording")
   }).then(function(){
    keys.mouseClick(item('t3ComposerDictate'))
    return waitFor(function(){return !!b.nativeVoicePermission},"retry asks microphone permission again")
   }).then(function(){
    keys.mouseClick(item('nativeVoicePermissionAllow'))
    return waitFor(function(){return b.voiceState.phase==='recording'&&composer.microphoneLevel===0.65},"explicit Allow reaches real native controller/metadata adapter")
   }).then(function(){
    check(composer.nativeVoicePreviewText==='<b>confirmed</b> <i>partial</i>'&&item('nativeVoicePreview').text==='<b>confirmed</b> <i>partial</i>'&&item('nativeVoicePreview').textFormat===Text.PlainText,"stream preview remains plain text")
    keys.mouseClick(item('t3ComposerDictate'))
    return waitFor(function(){return b.voiceState.phase==='idle'&&b.draft.text==='hello spoken'},"native mic stop inserts actual controller transcript")
   }).then(function(){
    check(!item('nativeVoicePreview').visible&&b.nativeVoicePreview===null&&composer.microphoneLevel===0,"idle clears old preview and level")
    check(Client.T3I18n.error('voice_cleanup_unknown').indexOf('voice_cleanup_unknown')<0,"shutdown failure is user-facing")
    check(Client.T3I18n.error('voice_host_lost').indexOf('voice_host_lost')<0,"connection failure is user-facing")
    keys.mouseClick(item('t3ComposerDictate'))
    return waitFor(function(){return !!b.nativeVoicePermission},'new permission before owner switch')
   }).then(function(){
    check(b.nativeVoicePreview===null&&composer.nativeVoicePreviewText===''&&!item('nativeVoicePreview').visible,'fresh preparing never revives old preview')
    var old=b.nativeVoicePermission
    b.selectedThread='two';b.selectedKey='env/two'
    return waitFor(function(){return !b.nativeVoicePermission},'changing draft denies old permission')
   }).then(function(){
    check(b.drafts['env/two'].text==='other','stale permission never writes other draft')
    b.selectedThread='one';b.selectedKey='env/one'
    return patch('voiceShortcutMode','hold')
   }).then(function(){
    composer.inputField.forceActiveFocus()
    keys.keyPress(Qt.Key_Space,Qt.ControlModifier)
    return waitFor(function(){return !!b.nativeVoicePermission},'held shortcut waits for actual permission')
   }).then(function(){
    check(!composer.nativeVoiceShortcut({key:Qt.Key_A,isAutoRepeat:false},false),'unrelated release does not stop hold')
    keys.keyRelease(Qt.Key_Space,Qt.ControlModifier)
    keys.mouseClick(item('nativeVoicePermissionAllow'))
    return waitFor(function(){return b.voiceState.phase==='idle'&&b.draft.text==='hello spoken spoken'},'hold release waits for prepare then stops and commits')
   }).then(function(){
    settings.chooseDestination('off')
    return waitFor(function(){return b.nativeVoiceChoice==='off'},"explicit off")
   }).then(function(){
    check(b.voiceState.phase==='idle',"off does not auto-fallback to legacy")
    settings.chooseDestination('environment:env')
    return waitFor(function(){return b.nativeVoiceChoice&&b.nativeVoiceChoice.destination.environmentId==='env'},'restore explicit destination for helper-loss test')
   }).then(function(){
    keys.mouseClick(item('t3ComposerDictate'))
    return waitFor(function(){return !!b.voiceController&&!!b.nativeVoicePermission},'actual prepared controller exists before helper loss')
   }).then(function(){
    b.nativeVoiceHost.receive({kind:'ready',available:false})
    check(!b.nativeVoiceAvailable&&b.nativeVoiceInitialized&&!b.voiceController&&!b.nativeVoicePermission,'real ready(false) clears actual controller and permission')
    check(composer.nativeVoiceMode&&item('t3ComposerDictate').visible&&!item('t3ComposerDictate').enabled,'lost native helper leaves disabled native mic')
    check(!!composer.nativeVoiceFailure&&item('nativeVoiceErrorMessage').visible,'helper-loss guidance remains visible')
    composer.inputField.forceActiveFocus();keys.keyClick(Qt.Key_Space,Qt.ControlModifier)
    check(b.dictation.state==='idle'&&!b.voiceController,'lost native helper never silently falls back to legacy shortcut')
    console.log('VOICE_UI_QML_PASS: actual Backend/Host/platform/controller/Composer and Settings; transport-only synthetic peer, no microphone/network/credentials; no automatic grants, env/off/cleanup, native9field patches, dictionary, Block/Escape/Allow, plain identity/preview, real mic start/stop/commit and readable errors')
    Qt.quit()
   })
 }
 Timer{interval:350;running:true;onTriggered:root.run().catch(function(error){console.error('VOICE_UI_QML_FAIL '+error+' '+error.stack);Qt.exit(1)})}
 Timer{interval:18000;running:true;onTriggered:{console.error('VOICE_UI_QML_FAIL deadline');Qt.exit(1)}}
}
