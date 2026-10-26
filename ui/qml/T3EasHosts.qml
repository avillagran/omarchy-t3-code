import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 required property var environments
 property bool projectScope:false
 property string environment:environments[0]||""
 readonly property var view:((bridge.deviceView||{}).sessions||{})[environment]||({})
 readonly property var details:view.data||({settings:{},permissions:{}})
 readonly property var operation:(bridge.deviceOperations||{})[environment]||({})
 readonly property bool writable:!projectScope&&details.settings.easSupported===true&&details.permissions.configure===true&&operation.state!=="pending"
 readonly property var account:bridge.nightlyState?bridge.nightlyState("eas-account",environment,""):({})
 property var editing:null
 property bool saving:false
 spacing:Style.space(8)
 onEnvironmentChanged:{editing=null;saving=false}
 Connections{target:root.bridge;function onDeviceOperationsChanged(){if(root.saving&&root.operation.state==="complete"){root.saving=false;root.editing=null}}}
 function edit(host){editing=host||{id:bridge.uuid(),label:"EAS Simulator",appId:"",maxSessionMinutes:30,idleMinutes:10,maxConcurrentSessions:2}}
 function save(){bridge.deviceAction("eas-upsert",{host:{id:editing.id,label:label.text,appId:app.text,maxSessionMinutes:Number(session.text),idleMinutes:Number(idle.text),maxConcurrentSessions:Number(concurrent.text)}},[environment]);saving=true}
 T3Select{Layout.fillWidth:true;visible:root.environments.length>1;model:root.environments;onActivated:function(i){root.environment=model[i]}}
 Text{Layout.fillWidth:true;text:T3I18n.tr("Cloud simulators");color:Color.popups.text;font.pixelSize:Style.font.body}
 RowLayout{
  Layout.fillWidth:true
  T3PickerButton{objectName:"easAddHost";primary:false;text:T3I18n.tr("Add host");enabled:root.writable&&!root.editing;onClicked:root.edit(null)}
  T3PickerButton{objectName:"easAccount";primary:false;text:T3I18n.tr("Check Expo account");enabled:root.writable&&!root.account.busy;onClicked:root.bridge.requestNightly("eas-account",{},root.environment,"")}
 }
 Text{Layout.fillWidth:true;text:root.account.error||(root.account.data?root.account.data.signedInAs+" · "+(root.account.data.apps||[]).map(function(a){return a.fullName}).join(", "):"");color:Color.popups.text;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
 Repeater {
  model:root.details.settings.easDeviceHosts||[]
  RowLayout {
   required property var modelData
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:modelData.label+" · "+modelData.maxSessionMinutes+" min / "+modelData.idleMinutes+" min · "+modelData.maxConcurrentSessions;color:Color.popups.text;textFormat:Text.PlainText;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   T3IconButton{name:"user";tooltipText:T3I18n.tr("Check Expo account");enabled:root.writable;onClicked:root.bridge.requestNightly("eas-account",{hostId:modelData.id},root.environment,"")}
   T3IconButton{name:"square-pen";tooltipText:T3I18n.tr("Edit");enabled:root.writable;onClicked:root.edit(modelData)}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Remove");enabled:root.writable;onClicked:root.bridge.deviceAction("eas-remove",{host:modelData},[root.environment])}
  }
 }
 ColumnLayout {
  Layout.fillWidth:true;visible:!!root.editing
  T3TextField{id:label;objectName:"easLabel";Layout.fillWidth:true;text:root.editing?root.editing.label:"";placeholderText:T3I18n.tr("Name")}
  T3TextField{id:app;objectName:"easAppId";Layout.fillWidth:true;text:root.editing?root.editing.appId||"":"";placeholderText:T3I18n.tr("Expo project ID (optional)")}
  RowLayout{
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Session limit (minutes)");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3TextField{id:session;objectName:"easSessionMinutes";Layout.preferredWidth:Style.space(65);text:String(root.editing?root.editing.maxSessionMinutes:30);validator:IntValidator{bottom:1;top:115}}
  }
  RowLayout{
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Idle limit (minutes)");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3TextField{id:idle;objectName:"easIdleMinutes";Layout.preferredWidth:Style.space(65);text:String(root.editing?root.editing.idleMinutes:10);validator:IntValidator{bottom:1;top:60}}
  }
  RowLayout{
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Concurrent sessions");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3TextField{id:concurrent;objectName:"easConcurrent";Layout.preferredWidth:Style.space(65);text:String(root.editing?root.editing.maxConcurrentSessions:2);validator:IntValidator{bottom:1;top:20}}
  }
  RowLayout{
   T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:root.writable;onClicked:root.editing=null}
   T3PickerButton{objectName:"easSave";text:T3I18n.tr("Save");enabled:root.writable&&!!label.text.trim()&&Number(idle.text)<Number(session.text)&&session.acceptableInput&&idle.acceptableInput&&concurrent.acceptableInput;onClicked:root.save()}
  }
 }
 Text{Layout.fillWidth:true;visible:!!root.operation.error;text:root.operation.error||"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
}
