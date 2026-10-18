import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeKeybindings.js" as Native
import "../js/KeybindingLabels.js" as Labels
ColumnLayout {
 id:root
 objectName:"t3Keybindings"
 required property var bridge
 required property var targets
 property string mountedSignature:""
 property var presentedBindings:[]
 property string presentedRevision:""
 property string query:""
 property bool adding:false
 property string newCommand:""
 readonly property var k:Native.nativeKeys
 readonly property var scope:bridge.keybindingScope
 readonly property var configs:bridge.keybindingConfigs
 readonly property var ids:scope.environments || []
 readonly property var primary:ids.length?configs[ids[0]]:null
 readonly property var snapshot:primary?primary.data:null
 readonly property var bindings:presentedBindings
 readonly property var allRows:k.buildKeybindingRows(bindings,"")
 readonly property var groups:{var english=k.buildKeybindingRows(bindings,query),q=query.trim().toLocaleLowerCase();return k.groupKeybindingRows(allRows.filter(function(row){return english.some(function(r){return r.id===row.id})||Labels.command(row.command,function(key){return T3I18n.tr(key)}).toLocaleLowerCase().indexOf(q)>=0}))}
 readonly property bool loading:ids.some(function(id){return root.configs[id]&&root.configs[id].busy})
 readonly property bool saving:!!bridge.keybindingWrite.busy
 readonly property bool writable:ids.length>0&&!scope.error&&!saving&&ids.every(function(id){var c=root.configs[id];return c&&!c.busy&&!c.error&&c.data&&c.data.writable})
 readonly property bool currentWrite:bridge.keybindingWrite.signature===scope.signature
 readonly property var results:currentWrite?bridge.keybindingWrite.results || ({}):({})
 readonly property var commands:k.buildKeybindingCommandOptions(bindings).map(function(command){return {id:command,name:Labels.command(command,function(key){return T3I18n.tr(key)})}})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 spacing:Style.space(24)
 function environmentName(id){return ((bridge.state.environments || []).find(function(e){return e.id===id}) || {}).name || id}
 function refresh(){if(!saving)bridge.requestKeybindings(targets,true)}
 function write(action,input){if(writable)bridge.writeKeybinding(action,input)}
 function capture(value){bridge.keybindingCaptureActive=value}
 function syncTargets(){var ids=[];(targets || []).forEach(function(t){if(ids.indexOf(t.environment)<0)ids.push(t.environment)});var signature=JSON.stringify(ids);if(signature===mountedSignature)return;mountedSignature=signature;adding=false;newCommand="";bridge.keybindingCaptureActive=false;bridge.requestKeybindings(targets,false)}
 function presentData(){var revision=snapshot?snapshot.environment+"/"+snapshot.originKey+"/"+snapshot.revision:"";if(revision===presentedRevision)return;presentedRevision=revision;presentedBindings=snapshot?k.mergeWithDefaultKeybindings(snapshot.keybindings):[]}
 onSnapshotChanged:presentData()
 onTargetsChanged:if(bridge)syncTargets()
 Component.onCompleted:{syncTargets();presentData()}
 Component.onDestruction:bridge.dismissKeybindings()
 Connections{target:root.bridge;function onKeybindingWriteChanged(){if(root.adding&&root.currentWrite&&!root.saving&&Object.keys(root.results).length&&Object.keys(root.results).every(function(id){return root.results[id].state==="complete"}))root.adding=false}}
 T3PreferenceSection{Layout.fillWidth:true;title:"Keyboard shortcuts";icon:"keyboard"
  body:Component{RowLayout{width:parent.width;spacing:Style.space(8)
   Controls.TextField{objectName:"keybindingSearch";Layout.fillWidth:true;Layout.margins:Style.space(12);placeholderText:T3I18n.tr("Search keybindings");text:root.query;onTextEdited:root.query=text;color:root.fg;placeholderTextColor:root.dim;selectByMouse:true;background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.035);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)}}
   T3PickerButton{objectName:"keybindingAdd";primary:false;text:T3I18n.tr("Add keybinding");enabled:root.writable&&!root.adding;onClicked:{root.adding=true;root.newCommand=""}}
   T3PickerButton{objectName:"keybindingsOpenFile";primary:false;text:"{}";Accessible.name:T3I18n.tr("Open keybindings.json");Controls.ToolTip.visible:hovered;Controls.ToolTip.delay:350;Controls.ToolTip.text:T3I18n.tr("Open keybindings.json");enabled:!!root.snapshot&&!!root.snapshot.canOpen&&!!root.snapshot.path&&(root.snapshot.editors || []).length>0&&!root.bridge.keybindingOpen.busy;onClicked:root.bridge.openKeybindings(root.ids[0],root.snapshot.viewId)}
   T3IconButton{objectName:"keybindingsDismissResult";visible:Object.keys(root.results).length>0&&!root.saving;name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.bridge.dismissKeybindingResult()}
   T3IconButton{objectName:"keybindingsReload";name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.loading&&!root.saving;onClicked:root.refresh()}
  }}
 }
 WorkIndicator{visible:root.loading;active:visible;Layout.alignment:Qt.AlignHCenter}
 Text{visible:!!root.scope.error;Layout.fillWidth:true;text:T3I18n.error(root.scope.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
 Text{visible:root.ids.length>0&&root.bridge.keybindingOpen.environment===root.ids[0]&&!!root.bridge.keybindingOpen.error;Layout.fillWidth:true;text:T3I18n.error(root.bridge.keybindingOpen.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
 Repeater{model:root.ids
  Text{required property string modelData;visible:!!(root.configs[modelData] || {}).error;Layout.fillWidth:true;text:root.environmentName(modelData)+": "+T3I18n.error((root.configs[modelData] || {}).error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 }
 Text{visible:!root.writable&&!root.loading&&!root.saving;Layout.fillWidth:true;text:T3I18n.tr(root.ids.length?"This connection can view keybindings but cannot change them.":"Connect an environment to change keybindings.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Repeater{model:root.snapshot?root.snapshot.issues || []:[]
  Text{required property var modelData;Layout.fillWidth:true;text:modelData.message;color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 }
 Repeater{model:Object.keys(root.results)
  Text{required property string modelData;Layout.fillWidth:true;text:root.environmentName(modelData)+" · "+T3I18n.tr(root.results[modelData].state==="complete"?"Saved":root.results[modelData].state==="pending"?"Pending":root.results[modelData].state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":"Failed")+(root.results[modelData].error?"\n"+T3I18n.error(root.results[modelData].error):"");color:root.results[modelData].error?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 }
 T3PreferenceSection{visible:root.adding;Layout.fillWidth:true;title:"Add keybinding";icon:"keyboard"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(10)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12)
    T3Select{objectName:"keybindingCommand";Layout.fillWidth:true;model:root.commands;textRole:"name";currentIndex:model.findIndex(function(c){return c.id===root.newCommand});popupAbove:false;enabled:root.writable;onActivated:function(i){root.newCommand=model[i].id}}
    T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.saving;onClicked:root.adding=false}
   }
   Loader{Layout.fillWidth:true;active:!!root.newCommand;visible:active;sourceComponent:Component{T3KeybindingRow{isNew:true;row:({id:"new",command:root.newCommand,key:"",when:"",source:"Custom",defaultKey:null,defaultWhen:""});allRows:root.allRows;writable:root.writable;saving:root.saving;onSaveRequested:function(input){root.write("upsert",input)};onCaptureChanged:function(value){root.capture(value)}}}}
  }}
 }
 Repeater{model:root.groups
  T3PreferenceSection{required property var modelData;Layout.fillWidth:true;title:modelData.title;icon:"keyboard"
   body:Component{Column{width:parent.width
    Repeater{model:modelData.rows
     T3KeybindingRow{required property var modelData;required property int index;width:parent.width;row:modelData;allRows:root.allRows;divided:index>0;writable:root.writable;saving:root.saving;onCaptureChanged:function(value){root.capture(value)};onSaveRequested:function(input){root.write("upsert",input)};onResetRequested:function(input){root.write("upsert",input)};onRemoveRequested:function(input){root.write("remove",input)}}
   }}}
  }
 }
 Text{visible:!root.loading&&!!root.snapshot&&!root.groups.length;Layout.fillWidth:true;text:T3I18n.tr("No keybindings match your search.");color:root.dim;font.pixelSize:Style.font.body;horizontalAlignment:Text.AlignHCenter}
}
