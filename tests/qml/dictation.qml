import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_DICTATION_QML_FAIL: "+msg);Qt.exit(1)}}
 function named(item,name){if(item.objectName===name)return item;var kids=item.children || [];for(var i=0;i<kids.length;i++){var found=named(kids[i],name);if(found)return found}return null}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:150;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,button=root.named(panel.composerView,"t3ComposerDictate"),editor=root.named(panel.composerView,"t3ComposerEditor")
  if(phase===0){if(!b.viewStateLoaded||!b.dictationCatalog.loaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;check(!!button&&button.visible,"microphone shown when the helper is available")
   b.edit("Antes después",false);editor.forceActiveFocus();phase=2
  }else if(phase===2){editor.cursorPosition=5;b.rememberView({dictationVocabulary:"Omarchy"});check(b.dictationPrompt().indexOf("Omarchy")===0&&b.dictationPrompt().indexOf("One")>0,"vocabulary, project and title in the prompt: "+b.dictationPrompt())
   b.startDictation();check(b.dictation.state==="starting","dictation starts");phase=3
  }else if(phase===3){if(b.dictation.state!=="recording"||b.dictation.levels.length<3)return;check(button.listening,"button shows recording")
   var wave=root.named(panel.composerView,"t3ComposerDictationWave");check(wave&&wave.shown&&wave.visible,"the bubble shows while recording")
   check(wave.bar(0.2)===0&&wave.bar(0.8)>0.9,"room noise draws flat, a voice draws a tall bar: "+wave.bar(0.2)+" "+wave.bar(0.8));b.stopDictation();phase=4
  }else if(phase===4){if(b.dictation.state!=="idle")return
   check(b.draft.text==="Antes hola mundo después","text inserted at the cursor with spaces: "+JSON.stringify(b.draft.text));check(!b.dictation.error,"no error "+b.dictation.error)
   b.startDictation();phase=5
  }else if(phase===5){if(b.dictation.state!=="recording")return
   var bubble=root.named(panel.composerView,"t3ComposerDictationWave");b.dictation=Object.assign({},b.dictation,{muted:true})
   check(bubble.muted&&bubble.notice.length>0,"a muted microphone says so: "+bubble.notice);b.dictationLine('{"type":"level","value":0.5}');check(!bubble.muted,"sound clears the muted notice")
   b.dictation=Object.assign({},b.dictation,{levels:Array(14).fill(0)});check(bubble.silent&&bubble.notice.length>0,"a second of zeros says no sound arrives");b.cancelDictation();phase=6
  }else if(phase===6){if(b.dictation.state!=="idle")return;check(b.draft.text==="Antes hola mundo después"&&!b.dictation.error,"cancel inserts nothing")
   b.rememberView({dictationModel:"small"});check(!b.dictationModelInfo.installed,"small not installed yet");b.startDictation();check(b.dictationDownload.busy&&b.dictation.state==="idle","first use downloads the model");phase=7
  }else if(phase===7){if(b.dictationDownload.busy||!b.dictationModelInfo.installed)return;check(!b.dictationDownload.error,"download succeeded")
   b.rememberView({dictationButton:false});phase=8
  }else if(phase===8){check(!button.visible,"preference hides the microphone");panel.close();console.log("T3_DICTATION_QML_PASS: insert at cursor, prompt vocabulary, level wave, cancel, model download and visibility preference");Qt.quit()}
 }}
}
