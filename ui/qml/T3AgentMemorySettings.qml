import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 property string environment:bridge.selectedEnvironment || "local"
 readonly property var settings:bridge.viewState || ({})
 readonly property var providerCatalog:(bridge.catalogs || {})[environment] || bridge.catalog || ({})
 readonly property var providers:[{id:"",title:T3I18n.tr("Default")}].concat((providerCatalog.providers || []).map(function(p){return {id:p.instanceId,title:p.name || p.instanceId}}))
 readonly property string provider:providers[providerSelect.currentIndex] ? providers[providerSelect.currentIndex].id : ""
 readonly property string providerKey:environment+"/"+provider
 readonly property var overrides:settings.idleReleaseProviderMinutes || ({})
 readonly property var value:provider&&Object.prototype.hasOwnProperty.call(overrides,providerKey)?overrides[providerKey]:settings.idleReleaseMinutes===undefined?5:settings.idleReleaseMinutes
 readonly property var choices:[{value:null,label:T3I18n.tr("Never")}].concat([1,5,10,15,30,60].map(function(n){return {value:n,label:String(n)+" "+T3I18n.tr("minutes")}}))
 function configure(patch){if(typeof bridge.configureCompaction==="function")bridge.configureCompaction(patch)}
 function setMinutes(value){if(provider){var all=Object.assign({},overrides);all[providerKey]=value;configure({idleReleaseProviderMinutes:all})}else configure({idleReleaseMinutes:value})}
 spacing:Style.space(12)
 RowLayout{Layout.fillWidth:true
  Text{Layout.fillWidth:true;text:T3I18n.tr("Release idle agents");color:Color.popups.text;font.pixelSize:Style.font.body;font.weight:Font.Medium}
  T3Switch{objectName:"idleReleaseEnabled";Accessible.name:T3I18n.tr("Release idle agents");checked:root.settings.idleReleaseEnabled!==false;onToggled:root.configure({idleReleaseEnabled:checked})}
 }
 Text{Layout.fillWidth:true;text:T3I18n.tr("O3 frees idle agent memory and resumes the saved conversation when you send a message.");color:Color.popups.text;opacity:0.65;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 RowLayout{Layout.fillWidth:true
  T3Select{id:providerSelect;objectName:"idleReleaseProvider";Layout.fillWidth:true;model:root.providers;textRole:"title"}
  T3Select{objectName:"idleReleaseMinutes";Layout.preferredWidth:Style.space(130);model:root.choices;textRole:"label";currentIndex:root.choices.findIndex(function(c){return c.value===root.value});enabled:root.settings.idleReleaseEnabled!==false;onActivated:function(i){root.setMinutes(root.choices[i].value)}}
 }
 Text{Layout.fillWidth:true;text:T3I18n.tr("Running tasks, questions and approvals keep the session open. O3 must remain running.");color:Color.popups.text;opacity:0.55;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 RowLayout{Layout.fillWidth:true
  Text{Layout.fillWidth:true;text:T3I18n.tr("Automatic compaction");color:Color.popups.text;font.pixelSize:Style.font.caption}
  T3PickerButton{objectName:"memoryCompactionSettings";text:T3I18n.tr("Settings");primary:false;onClicked:{compaction.active=true;Qt.callLater(function(){if(compaction.item)compaction.item.open()})}}
 }
 Text{objectName:"harnessCompactionExplanation";Layout.fillWidth:true;text:T3I18n.tr("Codex manages context compaction. Claude and other supported harnesses use their own cache lifetime.");color:Color.popups.text;opacity:0.55;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 Loader{id:compaction;active:false;sourceComponent:Component{T3CompactionSettings{bridge:root.bridge;environment:root.environment;providerCatalog:root.providerCatalog}}}
}
