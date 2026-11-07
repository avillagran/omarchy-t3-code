import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Client
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property var controller:null
 property var resolveTranscript:null
 property var events:[]
 property var guiResolver:null
 property string contextReference:"![image](t3-context://v1/image/synthetic)"
 FloatingWindow{id:window;visible:true;implicitWidth:720;implicitHeight:380
  Client.T3Composer{id:composer;width:parent.width;bridge:root.bridge}
 }
 TestCase{id:keys;parent:window.contentItem;when:false}
 Timer{id:gui;interval:30;onTriggered:{var resolve=root.guiResolver;root.guiResolver=null;if(resolve)resolve()}}
 function guiTurn(){return new Promise(function(resolve){root.guiResolver=resolve;gui.start()})}
 function check(value,message){if(!value){console.error("VOICE_CONTROLLER_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function settle(){var chain=Promise.resolve();for(var i=0;i<96;i++)chain=chain.then(function(){});return chain}
 function run(){
  var b=bridge
  b.state={threads:[{id:"one",key:"env/one",environmentId:"env",projectId:"project"},{id:"two",key:"env/two",environmentId:"env",projectId:"project"}],environments:[],projects:[]}
  b.selectedEnvironment="env";b.selectedThread="one";b.selectedKey="env/one"
  b.drafts={"env/one":{text:"hello ",loading:false,busy:false,attachments:[],attempt:null},"env/two":{text:"other",loading:false,busy:false,attachments:[],attempt:null}}
  b.rememberDraftCursor(6,6);b.resetUndo(b.selectedKey)
  var recorder={uri:"memory:synthetic-recording",prepareToRecordAsync:function(){events.push('prepare');return Promise.resolve()},record:function(options){check(options.forDuration===300,"original recording limit");events.push('record')},stop:function(){events.push('stop');return Promise.resolve()}}
  var dependencies={recorder:recorder,
   getTranscriber:function(){return {prepare:function(){return Promise.resolve({locale:"en",transcribe:function(uri,options){check(uri==="memory:synthetic-recording","owned synthetic URI");options.signal.addEventListener('abort',function(){events.push('abort')},{once:true});return new Promise(function(resolve){root.resolveTranscript=resolve})}})}}},
   requestPermission:function(){return Promise.resolve({granted:true,canAskAgain:true})},configureRecording:function(){events.push('configure');return Promise.resolve()},releaseRecording:function(){events.push('release');return Promise.resolve()},deleteRecording:function(uri){events.push('delete-plan:'+uri)}}
  controller=b.createVoiceController(dependencies)
  return controller.start().then(function(){
   check(b.voiceState.phase==='recording'&&b.voiceBlocksSubmission,"real backend blocks submission during recording")
   check(composer.voiceBlocksSubmission&&!keys.findChild(composer,'t3SendAndNewThread').enabled,"actual composer reflects native submission block")
   b.submit(false);check(!b.draft.attempt&&!b.draft.busy&&b.draft.text==='hello ',"submission stops before dispatch or draft mutation")
   var stopping=controller.stop();return settle().then(function(){check(b.voiceState.phase==='transcribing',"actual transcribing state");root.resolveTranscript('voice');return stopping})
  }).then(function(){
   check(b.draft.text==='hello voice'&&b.draftCursors[b.selectedKey].start===11,"native commit reaches actual draft and cursor")
   check(b.voiceState.phase==='idle'&&!b.voiceBlocksSubmission,"completed insertion releases submission")
   return guiTurn()
  }).then(function(){
   check(composer.editorPlain()==='hello voice'&&composer.inputField.cursorPosition===11,"actual rich editor renders transcript and restores cursor")
   check(b.undoDraft(false)&&b.draft.text==='hello ',"voice insertion participates in real undo")
   check(b.undoDraft(true)&&b.draft.text==='hello voice',"real redo restores insertion")
   return guiTurn().then(function(){return controller.start()})
  }).then(function(){
   var stopping=controller.stop();return settle().then(function(){var original=b.draft.text;b.edit('changed',false);b.edit(original,false);root.resolveTranscript('must not insert');return stopping})
  }).then(function(){
   check(b.draft.text==='hello voice'&&b.voiceState.phase==='error',"A-B-A edit rejects stale transcript despite restored text")
   controller.cancel();composer.inputField.cursorPosition=composer.inputField.length;b.rememberDraftCursor(b.draft.text.length,b.draft.text.length);return controller.start()
  }).then(function(){
   var stopping=controller.stop();return settle().then(function(){b.patchDraft('env/two',{text:'updated elsewhere'});b.rememberDraftCursor(0,0);root.resolveTranscript('tail');return stopping})
  }).then(function(){
   check(b.draft.text==='hello voice tail'&&b.draftCursors[b.selectedKey].start===16,"another draft and current selection do not replace captured selection")
   return controller.start()
  }).then(function(){
   var stopping=controller.stop();return settle().then(function(){b.selectedThread='two';b.selectedKey='env/two';check(b.voiceState.phase==='idle',"owner change cancels current operation");root.resolveTranscript('foreign');return stopping})
  }).then(function(){
   check(b.drafts['env/one'].text==='hello voice tail'&&b.draft.text==='updated elsewhere',"late transcript cannot mutate either owner")
   check(events.filter(function(event){return event.indexOf('delete-plan:')===0}).length===4,"four synthetic recording release plans, no filesystem executor")
   var releasePreparation,old=b.createVoiceController(Object.assign({},dependencies,{getTranscriber:function(){return {prepare:function(){return new Promise(function(resolve){releasePreparation=resolve})}}}})),oldStart=old.start()
   return settle().then(function(){
    check(old.currentState.phase==='preparing',"old destination owns deferred preparation")
    var before=events.filter(function(event){return event==='record'}).length
    controller=b.createVoiceController(dependencies)
    var nextStart=controller.start()
    return settle().then(function(){
     check(controller.currentState.phase==='preparing'&&events.filter(function(event){return event==='record'}).length===before,"replacement waits until old preparation really settles")
     releasePreparation({locale:'en',transcribe:function(){return Promise.resolve('obsolete')}})
     return Promise.all([oldStart,nextStart])
    })
   })
  }).then(function(){
   check(b.voiceState.phase==='recording',"replacement receives current backend state")
   composer.inputField.forceActiveFocus();return guiTurn()
  }).then(function(){
   check(composer.inputField.activeFocus,"actual composer owns key focus")
   keys.keyClick(Qt.Key_Escape)
   check(b.voiceState.phase==='idle',"real Qt Escape cancels native voice controller")
   return settle().then(function(){return controller.start()})
  }).then(function(){
   b.visiblePanels=1;b.visiblePanels=0
   return settle()
  }).then(function(){
   check(b.voiceState.phase==='error'&&!b.voiceBlocksSubmission,"hidden panel interrupts actual controller")
   b.releaseVoiceController();check(!b.voiceController&&b.voiceState.phase==='idle',"explicit consent-owner release disposes and clears the binding")
   b.selectedThread='one';b.selectedKey='env/one';b.patchDraft(b.selectedKey,{text:root.contextReference+' end',context:null,attachments:[]})
   return guiTurn()
  }).then(function(){composer.inputField.cursorPosition=1;return guiTurn()}).then(function(){
   check(b.draftCursors[b.selectedKey].start===root.contextReference.length,"actual context chip cursor maps to source UTF-16 position")
   controller=b.createVoiceController(dependencies);return controller.start()
  }).then(function(){
   var stopping=controller.stop();return settle().then(function(){root.resolveTranscript('spoken');return stopping})
  }).then(function(){return guiTurn()}).then(function(){
   check(b.draft.text===root.contextReference+' spoken end'&&composer.inputField.cursorPosition===8,"voice preserves the context reference and restores logical rich-editor cursor")
   b.releaseVoiceController()
   console.log("VOICE_CONTROLLER_QML_PASS: actual Backend and rich Composer, synchronous draft/selection/undo, submit fencing, ABA revision, foreign edits, owner change, replacement settlement, physical Qt Escape, background interruption and context-chip source positions")
   Qt.quit()
  })
 }
 Timer{interval:300;running:true;onTriggered:root.run().catch(function(error){console.error("VOICE_CONTROLLER_QML_FAIL "+error+" "+error.stack);Qt.exit(1)})}
 Timer{interval:9000;running:true;onTriggered:{console.error("VOICE_CONTROLLER_QML_FAIL deadline");Qt.exit(1)}}
}
