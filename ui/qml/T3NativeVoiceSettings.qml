import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

Column {
 id:root
 required property var bridge
 spacing:Style.space(8)
 readonly property bool available:bridge.nativeVoiceAvailable===true
 readonly property var preferences:bridge.nativeVoicePreferences || null
 readonly property bool ready:available&&preferences!==null
 readonly property bool writable:ready&&!((bridge.clientSettings || {}).busy)
 readonly property var audioChoice:bridge.nativeVoiceChoice || null
 readonly property bool cleanupEnabled:!!bridge.nativeVoiceCleanupGrant
 property bool consentBusy:false
 property string consentError:""
 property string dictionaryError:""
 readonly property var words:preferences&&preferences.speechCustomWords?preferences.speechCustomWords:[]
 property var lifetime:({active:true})
 Component.onDestruction:lifetime.active=false
 readonly property var destinations:[{id:"off",name:T3I18n.tr("Off")}].concat(((bridge.state || {}).environments || []).filter(function(e){return e.state==="ready"}).map(function(e){return {id:"environment:"+e.id,name:String(e.name||e.title||e.id),environmentId:e.id}}))
 readonly property string destinationValue:audioChoice==="off"?"off":audioChoice&&audioChoice.destination&&audioChoice.destination.kind==="environment"?"environment:"+audioChoice.destination.environmentId:""
 readonly property var microphoneChoices:{
  var choices=[{id:"",name:T3I18n.tr("System default")}],known={"":true};
  (bridge.nativeVoiceMicrophones || []).forEach(function(device){var id=String(device.deviceId||"");if(id&&!known[id]){known[id]=true;choices.push({id:id,name:String(device.label||T3I18n.tr("Microphone"))})}});
  var selected=preferences?String(preferences.voiceMicrophone||""):"";
  if(selected&&!known[selected])choices.push({id:selected,name:T3I18n.tr("Selected microphone (unavailable)")});
  return choices;
 }
 function patch(key,value){if(!writable)return;var update={};update[key]=value;bridge.requestClientSettings(update)}
 function changeConsent(action){
  if(!available||consentBusy)return;
  consentBusy=true;consentError="";var life=lifetime;
  Promise.resolve().then(action).then(function(){if(life.active)root.consentBusy=false},function(error){if(!life.active)return;root.consentBusy=false;root.consentError=String(error.message||error)})
 }
 function saveWords(next){if(!writable)return;dictionaryError="";patch("speechCustomWords",next)}
 function editWord(term,change){
  var next=words.map(function(word){return {term:word.term,aliases:word.aliases.slice()}}),index=next.findIndex(function(word){return word.term===term});
  if(index<0){dictionaryError=T3I18n.tr("The dictionary changed. Try again.");return}
  change(next,index);saveWords(next);
 }
 function renameWord(term,value){editWord(term,function(next,index){next[index].term=value.trim()})}
 function removeWord(term){editWord(term,function(next,index){next.splice(index,1)})}
 function addAlias(term,value){if(!value.trim())return;editWord(term,function(next,index){if(next[index].aliases.length<8)next[index].aliases.push(value.trim())})}
 function renameAlias(term,alias,value){editWord(term,function(next,index){var at=next[index].aliases.indexOf(alias);if(at>=0)next[index].aliases[at]=value.trim()})}
 function removeAlias(term,alias){editWord(term,function(next,index){var at=next[index].aliases.indexOf(alias);if(at>=0)next[index].aliases.splice(at,1)})}
 function chooseDestination(id){
  if(id==="off"){changeConsent(function(){return root.bridge.nativeVoiceHost.chooseAudio(null)});return}
  var choice=destinations.find(function(item){return item.id===id});
  if(choice&&choice.environmentId)changeConsent(function(){return root.bridge.nativeVoiceHost.chooseAudio({kind:"environment",environmentId:choice.environmentId})})
 }
 Text{width:parent.width;text:T3I18n.tr("Choose where microphone audio is sent for transcription. Nothing is selected automatically.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 RowLayout{width:parent.width
  Text{Layout.fillWidth:true;text:T3I18n.tr("Transcribe with");color:Color.popups.text;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
  T3Select{objectName:"nativeVoiceDestination";Layout.preferredWidth:Style.space(220);model:root.destinations;textRole:"name";translateLabels:false;currentIndex:root.destinations.findIndex(function(choice){return choice.id===root.destinationValue});enabled:root.available&&!root.consentBusy;onActivated:function(index){root.chooseDestination(model[index].id)}}
 }
 Text{width:parent.width;visible:!root.audioChoice;text:T3I18n.tr("Choose a transcription destination to enable voice input.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 Text{width:parent.width;visible:root.audioChoice&&root.audioChoice!=="off"&&root.audioChoice.destination&&root.audioChoice.destination.kind==="cloud";text:visible?T3I18n.tr("Current cloud destination: {provider}",{provider:String(root.audioChoice.destination.provider||"")}):"";color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 Text{width:parent.width;visible:!root.available;text:T3I18n.tr("Native voice input is unavailable.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 Text{width:parent.width;visible:root.available&&!root.ready;text:T3I18n.tr("Loading voice settings…");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 RowLayout{width:parent.width
  Text{Layout.fillWidth:true;text:T3I18n.tr("Microphone");color:Color.popups.text;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
  T3Select{objectName:"nativeVoiceMicrophone";Layout.preferredWidth:Style.space(220);model:root.microphoneChoices;textRole:"name";translateLabels:false;currentIndex:root.microphoneChoices.findIndex(function(choice){return choice.id===(root.preferences?root.preferences.voiceMicrophone||"":"")});enabled:root.writable;onActivated:function(index){root.patch("voiceMicrophone",model[index].id)}}
 }
 Text{width:parent.width;text:T3I18n.tr("System default follows your operating system.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceShortcut";field:({key:"voiceShortcutMode",label:"Voice shortcut",kind:"choice",value:root.preferences?root.preferences.voiceShortcutMode||"auto":"auto",choices:[{id:"auto",name:"Automatic"},{id:"hold",name:"Hold to talk"},{id:"toggle",name:"Toggle"}]});writable:root.writable;onChanged:function(value){root.patch("voiceShortcutMode",value)}}
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceLanguage";field:({key:"speechLanguage",label:"Language",description:"Use auto for automatic detection, or enter a supported language code.",kind:"text",commitOnBlur:true,maximumLength:16,value:root.preferences?root.preferences.speechLanguage||"auto":"auto"});writable:root.writable;onChanged:function(value){root.patch("speechLanguage",value)}}
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceTranslate";field:({key:"speechTranslateToEnglish",label:"Translate to English",kind:"bool",value:root.preferences?root.preferences.speechTranslateToEnglish===true:false});writable:root.writable;onChanged:function(value){root.patch("speechTranslateToEnglish",value)}}
 Column{width:parent.width;spacing:Style.space(6)
  Text{text:T3I18n.tr("Custom words");color:Color.popups.text;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
  Repeater{model:root.words
   delegate:Column{
    id:wordRow;required property var modelData;required property int index
    width:parent.width;spacing:Style.space(4)
    RowLayout{width:parent.width
     Controls.TextField{objectName:"nativeVoiceTerm_"+wordRow.index;Layout.fillWidth:true;text:wordRow.modelData.term;maximumLength:50;enabled:root.writable;placeholderText:T3I18n.tr("Word");onEditingFinished:if(text!==wordRow.modelData.term)root.renameWord(wordRow.modelData.term,text)}
     T3IconButton{objectName:"nativeVoiceRemoveWord_"+wordRow.index;name:"trash-2";tooltipText:T3I18n.tr("Remove word");enabled:root.writable;onClicked:root.removeWord(wordRow.modelData.term)}
    }
    Repeater{model:wordRow.modelData.aliases
     delegate:RowLayout{
      id:aliasRow;required property string modelData;required property int index
      width:parent.width
      Controls.TextField{objectName:"nativeVoiceAlias_"+wordRow.index+"_"+aliasRow.index;Layout.fillWidth:true;text:aliasRow.modelData;maximumLength:50;enabled:root.writable;placeholderText:T3I18n.tr("Alias");onEditingFinished:if(text!==aliasRow.modelData)root.renameAlias(wordRow.modelData.term,aliasRow.modelData,text)}
      T3IconButton{name:"x";tooltipText:T3I18n.tr("Remove alias");enabled:root.writable;onClicked:root.removeAlias(wordRow.modelData.term,aliasRow.modelData)}
     }
    }
    RowLayout{width:parent.width;visible:wordRow.modelData.aliases.length<8
     Controls.TextField{id:newAlias;objectName:"nativeVoiceNewAlias_"+wordRow.index;Layout.fillWidth:true;maximumLength:50;enabled:root.writable;placeholderText:T3I18n.tr("Alias")}
     T3PickerButton{objectName:"nativeVoiceAddAlias_"+wordRow.index;text:T3I18n.tr("Add alias");primary:false;enabled:root.writable&&!!newAlias.text.trim();onClicked:root.addAlias(wordRow.modelData.term,newAlias.text)}
    }
   }
  }
  RowLayout{width:parent.width;visible:root.words.length<100
   Controls.TextField{id:newTerm;objectName:"nativeVoiceNewWord";Layout.fillWidth:true;maximumLength:50;enabled:root.writable;placeholderText:T3I18n.tr("Word")}
   T3PickerButton{objectName:"nativeVoiceAddWord";text:T3I18n.tr("Add word");primary:false;enabled:root.writable&&!!newTerm.text.trim()&&!root.words.some(function(word){return word.term===newTerm.text.trim()});onClicked:root.saveWords(root.words.concat([{term:newTerm.text.trim(),aliases:[]}]))}
  }
  Text{width:parent.width;visible:!!root.dictionaryError;text:root.dictionaryError;color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 }
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceSilence";field:({key:"speechSilenceFiltering",label:"Filter silence",kind:"bool",value:root.preferences?root.preferences.speechSilenceFiltering!==false:true});writable:root.writable;onChanged:function(value){root.patch("speechSilenceFiltering",value)}}
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceFillers";field:({key:"speechRemoveFillerWords",label:"Remove filler words",kind:"bool",value:root.preferences?root.preferences.speechRemoveFillerWords!==false:true});writable:root.writable;onChanged:function(value){root.patch("speechRemoveFillerWords",value)}}
 T3PreferenceField{width:parent.width;controlName:"nativeVoiceCleanup";field:({key:"nativeVoiceCleanup",label:"Clean up transcripts with a provider",description:"Sends the transcript and nearby draft text to the text generation provider on the draft’s environment to fix punctuation, numbers, and spoken corrections. Sends text only; off by default.",kind:"bool",value:root.cleanupEnabled});writable:root.available&&!root.consentBusy;onChanged:function(value){root.changeConsent(function(){return root.bridge.nativeVoiceHost.chooseCleanup(value===true)})}}
 T3PreferenceField{width:parent.width;visible:root.cleanupEnabled;controlName:"nativeVoiceCleanupMode";field:({key:"speechPostProcessingPrompt",label:"Cleanup instructions",kind:"choice",value:root.preferences&&root.preferences.speechPostProcessingPrompt?root.preferences.speechPostProcessingPrompt.mode:"default",choices:[{id:"default",name:"Default"},{id:"custom",name:"Custom"}]});writable:root.writable;onChanged:function(value){root.patch("speechPostProcessingPrompt",{mode:value,customInstructions:(root.preferences.speechPostProcessingPrompt||{}).customInstructions||""})}}
 T3PreferenceField{width:parent.width;visible:root.cleanupEnabled&&root.preferences&&(root.preferences.speechPostProcessingPrompt||{}).mode==="custom";controlName:"nativeVoiceCleanupInstructions";field:({key:"nativeVoiceCleanupInstructions",label:"Custom cleanup instructions",kind:"text",multiline:true,maximumLength:10000,value:root.preferences?(root.preferences.speechPostProcessingPrompt||{}).customInstructions||"":""});writable:root.writable;onChanged:function(value){root.patch("speechPostProcessingPrompt",{mode:"custom",customInstructions:value})}}
 T3PreferenceField{width:parent.width;visible:root.cleanupEnabled;controlName:"nativeVoiceCorrectionCue";field:({key:"speechCorrectionWord",label:"Correction cue",kind:"text",commitOnBlur:true,maximumLength:50,value:root.preferences?root.preferences.speechCorrectionWord||"":""});writable:root.writable;onChanged:function(value){root.patch("speechCorrectionWord",value.trim())}}
 Text{width:parent.width;visible:!!root.consentError||!!((root.bridge.clientSettings||{}).error);text:root.consentError?T3I18n.error(root.consentError):T3I18n.error((root.bridge.clientSettings||{}).error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
}
