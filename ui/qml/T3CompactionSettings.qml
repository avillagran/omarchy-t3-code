import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Controls.Popup {
 id:root
 required property var bridge
 parent:Controls.Overlay.overlay;anchors.centerIn:parent
 width:Style.space(420);padding:Style.space(18);modal:true
 readonly property var providerControl:providerSelect
 property var providers:[{id:"",title:T3I18n.tr("Default")}].concat((bridge.catalog.providers||[]).map(function(p){return {id:p.instanceId,title:p.name || p.instanceId}}))
 readonly property string provider:providers[providerSelect.currentIndex] ? providers[providerSelect.currentIndex].id : ""
 readonly property string providerKey:bridge.selectedEnvironment+"/"+provider
 readonly property int minutes:provider ? (bridge.viewState.autoCompactProviderMinutes || {})[providerKey] || bridge.viewState.autoCompactFallbackMinutes || 60 : bridge.viewState.autoCompactFallbackMinutes || 60
 readonly property var status:bridge.compactionStates["@engine"] && bridge.compactionStates["@engine"].status==="error" ? bridge.compactionStates["@engine"] : (bridge.viewState.autoCompactExcludedThreads||[]).indexOf(bridge.selectedKey)>=0 ? ({status:"disabled"}) : bridge.compactionStates[bridge.selectedKey] || ({status:"unknown_context"})
 function statusText(code){var labels={scheduled:"Scheduled",due:"Scheduled",claimed:"Working",accepted:"Compaction requested",uncertain:"Outcome unknown",failed:"Failed",cancelled:"Cancelled",expired:"Cache expired",unknown_context:"Context unavailable",too_small:"Context too small",unsupported:"Provider does not support compaction",server:"Compacted by the T3 server",disabled:"Disabled",busy:"Working",blocked:"Waiting",provider_changed:"Context unavailable",error:"Failed"};return T3I18n.tr(labels[code]||"Context unavailable")}
 function setMinutes(value){if(provider){var all=Object.assign({},bridge.viewState.autoCompactProviderMinutes||{});all[providerKey]=value;bridge.configureCompaction({autoCompactProviderMinutes:all})}else bridge.configureCompaction({autoCompactFallbackMinutes:value})}
 function toggleThread(enabled){var keys=(bridge.viewState.autoCompactExcludedThreads||[]).filter(function(k){return k!==root.bridge.selectedKey});if(!enabled)keys.push(bridge.selectedKey);bridge.configureCompaction({autoCompactExcludedThreads:keys})}
 component Toggle:Controls.CheckBox{
  Layout.fillWidth:true;Layout.minimumWidth:0
  spacing:Style.space(8);leftPadding:Style.space(26);rightPadding:0;topPadding:Style.space(4);bottomPadding:Style.space(4)
  indicator:Rectangle{x:0;y:(parent.height-height)/2;width:Style.space(17);height:width;radius:Style.space(4);color:parent.checked?Color.accent:"transparent";border.width:1;border.color:parent.activeFocus?Color.accent:Color.muted;T3Icon{anchors.centerIn:parent;implicitWidth:Style.space(12);implicitHeight:implicitWidth;name:"check";visible:parent.parent.checked;tint:Color.popups.background}}
  contentItem:Text{text:parent.text;color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText;verticalAlignment:Text.AlignVCenter}
 }
 property var minuteChoices:[1,5,15,30,60,120,360,1440].concat([root.minutes]).filter(function(v,i,a){return a.indexOf(v)===i}).sort(function(a,b){return a-b}).map(function(m){return {minutes:m,label:m+" min"}})
 property var tokenChoices:[1000,10000,20000,50000,100000,250000].concat([bridge.viewState.autoCompactMinTokens||20000]).filter(function(v,i,a){return a.indexOf(v)===i}).sort(function(a,b){return a-b}).map(function(n){return {tokens:n,label:String(n)}})
 background:Rectangle{color:Color.popups.background;radius:Style.space(10);border.width:1;border.color:Color.accent}
 contentItem:ColumnLayout{
  spacing:Style.space(12)
  RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr("Automatic compaction");color:Color.popups.text;font.pixelSize:Style.font.body;font.bold:true}T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}}
  Toggle{objectName:"autoCompactEnabled";text:T3I18n.tr("Compact inactive threads before cache expiry.");checked:root.bridge.viewState.autoCompactEnabled!==false;onToggled:root.bridge.configureCompaction({autoCompactEnabled:checked})}
  T3Select{id:providerSelect;Layout.fillWidth:true;model:root.providers;textRole:"title";enabled:root.bridge.viewState.autoCompactEnabled!==false}
  RowLayout{
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Estimated cache lifetime");color:Color.popups.text;font.pixelSize:Style.font.caption}
   T3Select{objectName:"autoCompactMinutes";Layout.preferredWidth:Style.space(110);Layout.maximumWidth:Style.space(110);model:root.minuteChoices;textRole:"label";currentIndex:root.minuteChoices.findIndex(function(c){return c.minutes===root.minutes});onActivated:function(index){root.setMinutes(root.minuteChoices[index].minutes)}}
  }
  Text{objectName:"autoCompactPriority";Layout.fillWidth:true;Layout.minimumHeight:implicitHeight;text:T3I18n.tr("Reported provider lifetime takes priority.");color:Color.popups.text;opacity:0.55;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  RowLayout{
   Text{Layout.fillWidth:true;text:T3I18n.tr("Minimum context tokens");color:Color.popups.text;font.pixelSize:Style.font.caption}
   T3Select{objectName:"autoCompactMinimum";Layout.preferredWidth:Style.space(110);Layout.maximumWidth:Style.space(110);model:root.tokenChoices;textRole:"label";currentIndex:root.tokenChoices.findIndex(function(c){return c.tokens===(root.bridge.viewState.autoCompactMinTokens||20000)});onActivated:function(index){root.bridge.configureCompaction({autoCompactMinTokens:root.tokenChoices[index].tokens})}}
  }
  Rectangle{Layout.fillWidth:true;implicitHeight:1;color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.12)}
  Toggle{visible:!!root.bridge.selectedKey;objectName:"autoCompactThread";text:T3I18n.tr("Enable for this thread");checked:(root.bridge.viewState.autoCompactExcludedThreads||[]).indexOf(root.bridge.selectedKey)<0;onToggled:root.toggleThread(checked)}
  Text{Layout.fillWidth:true;text:root.statusText(root.bridge.viewState.autoCompactEnabled===false?"disabled":root.status.status)+(root.status.estimated?" · "+T3I18n.tr("Estimated"):"");color:root.status.status==="failed"||root.status.status==="uncertain"?Color.urgent:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;visible:root.status.status==="scheduled"&&!!root.status.dueAt;text:T3I18n.tr("Compacts at {time}",{time:Qt.formatDateTime(new Date(root.status.dueAt||0),"dd MMM · HH:mm")});color:Color.popups.text;opacity:0.55;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 }
}
