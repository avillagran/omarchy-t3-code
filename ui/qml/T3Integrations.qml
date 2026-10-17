import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeDeviceSetup.js" as Native
ColumnLayout {
 id:root
 objectName:"t3Integrations"
 required property var bridge
 required property var targets
 property var settings:({})
 property bool settingsWritable:false
 signal settingChanged(string key,var value)
 signal inheritRequested(string key)
 readonly property var sessions:bridge.deviceView.sessions || ({})
 readonly property var environments:Object.keys(sessions)
 readonly property string representative:environments.length?environments[0]:""
 readonly property var current:sessions[representative] || ({})
 readonly property var deviceData:current.data || ({settings:{},state:{hosts:[],devices:[]},permissions:{}})
 readonly property bool projectScope:targets.length>0&&targets[0].projectId!==null
 readonly property bool loading:environments.some(function(e){return root.sessions[e].busy})
 readonly property bool pending:environments.some(function(e){return (root.bridge.deviceOperations[e] || {}).state==="pending"})
 readonly property bool anyHub:environments.some(function(e){return !!root.sessions[e].data&&root.sessions[e].data.settings.enableDeviceSupport})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 spacing:Style.space(20)
 function field(key){return (settings.fields || []).find(function(f){return f.key===key}) || null}
 function scopeAllowed(action){return environments.length>0&&!pending&&root.bridge.deviceAllowed(representative,action)}
 function hubField(){var values=environments.map(function(e){return root.sessions[e].data?root.sessions[e].data.settings.enableDeviceSupport:null});return {key:"enableDeviceSupport",kind:"bool",label:"Device hub",description:Native.nativeDeviceSetup.deviceHubDescription,value:values[0]===true,mixed:values.some(function(v){return v!==values[0]})}}
 function agentField(){if(projectScope)return field("enableAgentDeviceAccess") || {key:"enableAgentDeviceAccess",kind:"bool",label:"Agent device access",value:false};var values=environments.map(function(e){return root.sessions[e].data?root.sessions[e].data.settings.enableAgentDeviceAccess:null});return {key:"enableAgentDeviceAccess",kind:"bool",label:"Agent device access",description:Native.nativeDeviceSetup.agentDeviceDescription,value:values[0]===true,mixed:values.some(function(v){return v!==values[0]})}}
 function editHost(h){hostEditor.load(h || {id:bridge.uuid(),label:"",target:""},h);hostPopup.open()}
 property var savingHost:null
 Connections{target:root.bridge;function onDeviceOperationsChanged(){if(!root.savingHost)return;var done=root.environments.every(function(e){var r=root.bridge.deviceOperations[e];return r&&r.action==="host-upsert"&&JSON.stringify(r.input.host)===JSON.stringify(root.savingHost)&&r.state==="complete"});if(done){root.savingHost=null;hostPopup.close()}}}
 onTargetsChanged:{savingHost=null;if(hostPopup)hostPopup.close();bridge.requestDevices(targets,false)}
 Component.onCompleted:bridge.requestDevices(targets,false)
 Component.onDestruction:bridge.dismissDevices()
 T3PreferenceSection{Layout.fillWidth:true;title:"Browser";icon:"globe"
  body:Component{Column{width:parent.width
   T3PreferenceField{width:parent.width;visible:!!root.field("enableAgentBrowserAccess");field:root.field("enableAgentBrowserAccess") || ({key:"enableAgentBrowserAccess",label:"Agent browser access",kind:"bool"});writable:root.settingsWritable&&field.scopeWritable!==false;onChanged:function(v){root.settingChanged(field.key,v)};onInheritRequested:root.inheritRequested(field.key)}
   T3BrowserProfiles{width:parent.width;bridge:root.bridge;targets:root.targets}
   T3BrowserDefaults{width:parent.width;bridge:root.bridge}
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"Devices";icon:"smartphone"
  body:Component{Column{width:parent.width
   T3PreferenceField{width:parent.width;field:root.hubField();writable:!root.projectScope&&root.scopeAllowed("hub");onChanged:function(v){root.bridge.deviceAction("hub",{enabled:v})}}
   T3PreferenceField{width:parent.width;divided:true;field:root.agentField();writable:root.projectScope?root.settingsWritable&&field.scopeWritable!==false:root.anyHub&&root.scopeAllowed("agent");onChanged:function(v){if(root.projectScope)root.settingChanged(field.key,v);else root.bridge.deviceAction("agent",{enabled:v})};onInheritRequested:root.inheritRequested(field.key)}
   ColumnLayout{width:parent.width;spacing:Style.space(8)
    Repeater{model:root.environments
     ColumnLayout{required property string modelData;Layout.fillWidth:true;Layout.margins:Style.space(14);spacing:Style.space(8)
      readonly property var session:root.sessions[modelData]
      readonly property var deviceData:session.data
      readonly property var operation:root.bridge.deviceOperations[modelData] || ({})
      RowLayout{Layout.fillWidth:true
       Text{Layout.fillWidth:true;text:modelData;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}
       WorkIndicator{visible:session.busy||operation.state==="pending";active:visible;implicitWidth:Style.space(16)}
       T3PickerButton{objectName:"deviceReload_"+modelData;primary:false;text:T3I18n.tr("Reload");enabled:!session.busy;onClicked:root.bridge.refreshDevice(modelData)}
      }
      Text{visible:!!session.error;Layout.fillWidth:true;text:T3I18n.error(session.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      Text{visible:!!deviceData;Layout.fillWidth:true;text:deviceData?T3I18n.tr(deviceData.state.hostStatus)+(deviceData.state.hostStatusDetail?" · "+deviceData.state.hostStatusDetail:""):"";color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      ColumnLayout{visible:!!operation.state&&operation.state!=="complete";Layout.fillWidth:true
       Text{Layout.fillWidth:true;text:T3I18n.tr(operation.state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":operation.state==="pending"?"Operation pending in native T3.":"Failed")+(operation.error?" · "+T3I18n.error(operation.error):"");color:operation.state==="failed"?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
       T3PickerButton{visible:operation.state!=="pending";primary:false;text:T3I18n.tr("Dismiss");onClicked:root.bridge.acknowledgeDevice(modelData)}
      }
      Repeater{model:deviceData&&deviceData.settings.enableDeviceSupport?["ios","android"]:[]
       Text{required property string modelData;Layout.fillWidth:true;readonly property var status:Native.nativeDeviceSetup.platformSetupStatus(deviceData.state,modelData);text:(modelData==="ios"?"iOS":"Android")+" · "+T3I18n.tr(status.message);color:status.ready?root.fg:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      }
      RowLayout{visible:!!deviceData;Layout.fillWidth:true
       T3PickerButton{primary:false;text:T3I18n.tr("Check versions");visible:!!deviceData&&deviceData.state.supportsToolInspection;enabled:root.bridge.deviceAllowed(modelData,"inspect");onClicked:root.bridge.deviceAction("inspect",{},[modelData])}
       T3PickerButton{primary:false;text:T3I18n.tr("Refresh");visible:!!deviceData&&deviceData.settings.enableDeviceSupport;enabled:root.bridge.deviceAllowed(modelData,"refresh");onClicked:root.bridge.deviceAction("refresh",{},[modelData])}
      }
      Repeater{model:deviceData?(deviceData.state.hosts || []):[]
       ColumnLayout{required property var modelData;Layout.fillWidth:true;spacing:Style.space(4)
        readonly property var host:modelData
        readonly property string environment:parent.modelData
        Text{Layout.fillWidth:true;text:host.label;color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
        Repeater{model:host.tools?["hub","agent"]:[]
         RowLayout{required property string modelData;Layout.fillWidth:true
          readonly property var version:host.tools[modelData]
          Text{Layout.fillWidth:true;text:T3I18n.tr(modelData==="hub"?"Device hub":"Agent device")+" · "+T3I18n.tr("Running")+": "+(version.runningVersion || T3I18n.tr("Not running"))+" · "+T3I18n.tr("Required")+": "+version.requiredVersion+" · "+T3I18n.tr("Installed")+": "+(version.installedVersions.join(", ") || T3I18n.tr("None"));color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
          T3PickerButton{primary:false;text:T3I18n.tr("Update");visible:host.kind==="local"&&version.installedVersions.indexOf(version.requiredVersion)<0;enabled:root.bridge.deviceAllowed(environment,"update-tool");onClicked:root.bridge.deviceAction("update-tool",{tool:modelData},[environment])}
         }
        }
        RowLayout{Layout.fillWidth:true
         readonly property var status:(deviceData.state.hostStatuses || {})[host.id] || ({status:host.kind==="local"?deviceData.state.hostStatus:"idle"})
         Text{Layout.fillWidth:true;text:T3I18n.tr(parent.status.status)+(parent.status.detail?" · "+parent.status.detail:"");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
         T3PickerButton{primary:false;text:T3I18n.tr("Retry");visible:parent.status.status==="failed";enabled:root.bridge.deviceAllowed(environment,"retry-host");onClicked:root.bridge.deviceAction("retry-host",{hostId:host.id},[environment])}
        }
        Text{visible:!!host.toolInspectionError;Layout.fillWidth:true;text:host.toolInspectionError || "";color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
       }
      }
     }
    }
   }
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"Device hosts";icon:"server"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(12)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Add remote machines with simulator or emulator runtimes installed, and the selected environments will connect over SSH and set up device tools automatically.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    T3PickerButton{objectName:"deviceHostAdd";primary:false;text:T3I18n.tr("Add host");enabled:!root.projectScope&&root.scopeAllowed("host-upsert")&&!hostPopup.opened;onClicked:root.editHost(null)}
   }
   Repeater{model:root.environments
    ColumnLayout{required property string modelData;readonly property string hostEnvironment:modelData;Layout.fillWidth:true;Layout.margins:Style.space(14)
     Text{visible:root.environments.length>1;text:modelData;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
     Repeater{model:root.sessions[hostEnvironment].data?root.sessions[hostEnvironment].data.settings.deviceHosts:[]
      RowLayout{required property var modelData;Layout.fillWidth:true
       ColumnLayout{Layout.fillWidth:true
        Text{text:modelData.label;color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
        Text{text:modelData.target;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
       }
       T3PickerButton{primary:false;text:T3I18n.tr("Edit");enabled:!root.projectScope&&root.scopeAllowed("host-upsert")&&!hostPopup.opened;onClicked:root.editHost(modelData)}
       T3PickerButton{primary:false;text:T3I18n.tr("Remove");enabled:!root.projectScope&&root.scopeAllowed("host-remove");onClicked:root.bridge.deviceAction("host-remove",{host:modelData,original:modelData})}
      }
     }
    }
   }
  }}
 }
 Controls.Popup{id:hostPopup;objectName:"deviceHostEditorPopup";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(580),parent?parent.width-Style.space(32):Style.space(580));padding:Style.space(20);modal:true;focus:true;closePolicy:root.pending?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16)}
  contentItem:T3DeviceHostEditor{id:hostEditor;bridge:root.bridge;environments:root.environments;onCancelled:hostPopup.close();onSaved:function(host){root.savingHost=JSON.parse(JSON.stringify(host));root.bridge.deviceAction("host-upsert",{host:host,original:original || host})}}
 }

}
