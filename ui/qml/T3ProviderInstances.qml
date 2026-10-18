import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root;objectName:"providerInstances"
 required property var bridge
 required property string environment
 readonly property var state:bridge.providerInstances || ({instances:[],drivers:[],busy:false,error:"",writable:false})
 readonly property bool current:state.environment===environment
 readonly property bool writable:current&&state.writable&&!state.busy
 readonly property var drivers:current?state.drivers || []:[]
 readonly property var instances:current?state.instances || []:[]
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 readonly property var selected:instances.find(function(v){return v.id===root.selectedId}) || null
 readonly property var definition:drivers.find(function(v){return v.id===root.driver}) || null
 readonly property var fields:definition?definition.fields || []:[]
 property string selectedId:""
 property string driver:"codex"
 property string mode:"create"
 property int step:0
 property string formError:""
 property bool awaitingSave:false
 property bool customId:false
 property var draftConfig:({})
 property var draftEnvironment:[]
 spacing:Style.space(12)
 function load(){if(environment&&environment!=="*"&&typeof bridge.requestProviderInstances==="function")bridge.requestProviderInstances(environment,"read")}
 onEnvironmentChanged:{selectedId="";editor.close();confirmation.close();load()}
 Component.onCompleted:load()
 Connections {target:root.bridge;function onProviderInstancesChanged(){
  if(!root.state.busy&&root.state.environment!==root.environment){root.load();return}
  if(!root.awaitingSave||root.state.busy||root.state.environment!==root.environment)return
  root.awaitingSave=false
  if(!root.state.error){editor.close();confirmation.close();if(root.mode==="create")root.selectedId=instanceId.text;else if(root.mode==="remove")root.selectedId="";root.draftConfig={};root.draftEnvironment=[]}
 }}
 function suggestedId(value){
  var slug=value.trim().toLowerCase().replace(/[^a-z0-9]+/g,"_").replace(/^_+|_+$/g,"").slice(0,48)
  var base=slug&&value!==definition.label?driver+"_"+slug:driver
  var used=instances.map(function(i){return i.id}).concat(["codex","claudeAgent"])
  if(used.indexOf(base)<0)return base
  var suffix=2,candidate="";do{var tail="_"+suffix++;candidate=base.slice(0,64-tail.length)+tail}while(used.indexOf(candidate)>=0)
  return candidate
 }
 function chooseDriver(kind){
  var item=drivers.find(function(d){return d.id===kind});if(!item)return
  driver=kind;formError="";customId=false;label.text=item.label;instanceId.text=suggestedId(item.label);color.text=""
  draftConfig=kind==="acpRegistry"?{source:"local",commandArgs:[]}:{}
  draftEnvironment=[]
 }
 function add(){if(!writable)return;mode="create";step=0;formError="";awaitingSave=false;chooseDriver(drivers.length?drivers[0].id:"codex");editor.open()}
 function edit(instance){if(!writable||!instance)return;mode="update";step=1;formError="";awaitingSave=false;driver=instance.driver
  label.text=instance.label;instanceId.text=instance.id;color.text=instance.accentColor || ""
  draftConfig=Object.assign({},instance.config || {});draftEnvironment=(instance.environment || []).map(function(e){return Object.assign({},e)})
  editor.open()
 }
 function setConfigValue(key,value){var next=Object.assign({},draftConfig);next[key]=value;draftConfig=next}
 function setEnvironment(index,key,value){var rows=draftEnvironment.slice();rows[index]=Object.assign({},rows[index],{[key]:value});draftEnvironment=rows}
 function toggle(instance){if(writable)bridge.requestProviderInstances(environment,"update",{instanceId:instance.id,enabled:!instance.enabled})}
 function submit(){
  if(!writable)return
  var id=instanceId.text.trim(),name=label.text.trim();if(!/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(id)||mode==="create"&&["codex","claudeAgent"].indexOf(id)>=0){formError=T3I18n.tr("Invalid or reserved instance ID");return}
  if(!name){formError=T3I18n.tr("Label is required");return}
  var config={};Object.keys(draftConfig).forEach(function(k){var v=draftConfig[k];if(typeof v==="string")v=v.trim();if(v!==""&&v!==null&&v!==undefined)config[k]=v})
  if(config.autoCompactWindow&&!/^(?:[1-9][0-9]{5}|1000000)$/.test(config.autoCompactWindow)){formError=T3I18n.tr("Use 100000 to 1000000 tokens");return}
  if(driver==="acpRegistry"&&config.source==="local"&&!config.commandPath){formError=T3I18n.tr("Executable is required");return}
  if(driver==="acpRegistry"&&config.source==="registry"&&!config.agentId){formError=T3I18n.tr("Registry agent ID is required");return}
  formError="";awaitingSave=true
  bridge.requestProviderInstances(environment,mode,{driver:driver,instanceId:id,label:name,accentColor:color.text.trim(),config:config,providerEnvironment:draftEnvironment})
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"Providers";icon:"acp"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(8)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12);spacing:Style.space(8)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Provider instances and accounts on {environment}",{environment:root.environment});color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.state.busy;onClicked:root.load()}
    T3PickerButton{objectName:"providerAdd";text:T3I18n.tr("Add provider");enabled:root.writable;onClicked:root.add()}
   }
   Text{Layout.fillWidth:true;Layout.margins:Style.space(12);visible:!!root.state.error;text:T3I18n.error(root.state.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   Repeater{model:root.instances
    RowLayout{required property var modelData;Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);Layout.bottomMargin:Style.space(8);spacing:Style.space(8)
     T3ProviderIcon{provider:({driverKind:modelData.driver});tint:root.fg;implicitWidth:Style.space(20)}
     ColumnLayout{Layout.fillWidth:true;spacing:Style.space(2)
      Text{Layout.fillWidth:true;text:modelData.label+" · "+modelData.id;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:(modelData.authStatus==="authenticated"?T3I18n.tr("Signed in"):modelData.installed===false?T3I18n.tr("Not installed"):T3I18n.tr("Sign in on this environment"));color:root.muted;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
     }
     T3Switch{checked:modelData.enabled;enabled:root.writable;onToggled:root.toggle(modelData)}
     T3IconButton{name:"square-pen";tooltipText:T3I18n.tr("Edit provider");enabled:root.writable;onClicked:root.edit(modelData)}
     T3IconButton{name:"trash-2";tint:Color.urgent;tooltipText:T3I18n.tr("Remove provider");visible:!modelData.isDefault;enabled:root.writable;onClicked:{root.selectedId=modelData.id;root.mode="remove";confirmation.open()}}
    }
   }
  }}
 }
 Controls.Popup{id:editor;parent:Controls.Overlay.overlay;anchors.centerIn:parent;modal:true;closePolicy:root.awaitingSave?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  width:Math.min(Style.space(600),parent?parent.width-Style.space(32):Style.space(600));padding:Style.space(20)
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.18)}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.mode==="create"?"Add provider":"Edit provider");color:root.fg;font.pixelSize:Style.font.body;font.bold:true}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Provider → Identity → Config")+" · "+root.environment;color:root.muted;font.pixelSize:Style.font.caption}
   Controls.ScrollView{Layout.fillWidth:true;Layout.preferredHeight:Math.min(Style.space(450),stepContent.implicitHeight+Style.space(12));contentWidth:availableWidth;clip:true
    ColumnLayout{id:stepContent;width:parent.width;spacing:Style.space(12)
     ColumnLayout{visible:root.step===0;Layout.fillWidth:true;spacing:Style.space(6)
      Text{text:T3I18n.tr("Provider");color:root.fg;font.pixelSize:Style.font.body}
      Repeater{model:root.drivers
       T3PickerButton{required property var modelData;objectName:"providerDriver_"+modelData.id;Layout.fillWidth:true;text:modelData.label+(root.driver===modelData.id?" ✓":"");primary:root.driver===modelData.id;onClicked:root.chooseDriver(modelData.id)}
      }
     }
     ColumnLayout{visible:root.step===1;Layout.fillWidth:true;spacing:Style.space(6)
      Text{text:T3I18n.tr("Label");color:root.fg;font.pixelSize:Style.font.caption}
      Controls.TextField{id:label;objectName:"providerLabel";Layout.fillWidth:true;selectByMouse:true;onTextEdited:if(root.mode==="create"&&!root.customId)instanceId.text=root.suggestedId(text)}
      Text{text:T3I18n.tr("Instance ID");color:root.fg;font.pixelSize:Style.font.caption}
      Controls.TextField{id:instanceId;objectName:"providerInstanceId";Layout.fillWidth:true;enabled:root.mode==="create";selectByMouse:true;onTextEdited:root.customId=true}
      Text{text:T3I18n.tr("Accent color (optional)");color:root.fg;font.pixelSize:Style.font.caption}
      T3TextField{id:color;objectName:"providerAccentColor";Layout.fillWidth:true;placeholderText:"#4285F4";selectByMouse:true}
     }
     ColumnLayout{visible:root.step===2;Layout.fillWidth:true;spacing:Style.space(9)
      Repeater{model:root.fields
       ColumnLayout{required property var modelData;Layout.fillWidth:true;spacing:Style.space(4)
        Text{text:T3I18n.tr(modelData.label);color:root.fg;font.pixelSize:Style.font.caption}
        Loader{Layout.fillWidth:true;active:modelData.control==="select";visible:active;sourceComponent:Component{T3Select{width:parent.width;model:modelData.options;textRole:"name";currentIndex:Math.max(0,model.findIndex(function(o){return o.id===root.draftConfig[modelData.key]}));onActivated:function(i){root.setConfigValue(modelData.key,model[i].id)}}}}
        T3TextField{objectName:"providerField_"+modelData.key;visible:modelData.control!=="select";Layout.fillWidth:true;placeholderText:modelData.control==="secret"||modelData.control==="password"&&root.selected&&root.selected.secrets[modelData.key]?T3I18n.tr("Saved value"):modelData.placeholder || "";text:root.draftConfig[modelData.key] || "";echoMode:modelData.control==="secret"||modelData.control==="password"?TextInput.Password:TextInput.Normal;selectByMouse:true;onTextEdited:root.setConfigValue(modelData.key,text)}
       }
      }
      Text{Layout.fillWidth:true;text:T3I18n.tr("Environment variables");color:root.fg;font.pixelSize:Style.font.body}
      Repeater{model:root.draftEnvironment
       RowLayout{required property var modelData;required property int index;Layout.fillWidth:true;spacing:Style.space(6)
        Controls.TextField{Layout.preferredWidth:Style.space(135);placeholderText:T3I18n.tr("Name");text:modelData.name || "";selectByMouse:true;onEditingFinished:root.setEnvironment(index,"name",text.trim())}
        T3TextField{Layout.fillWidth:true;placeholderText:modelData.configured?T3I18n.tr("Saved value"):T3I18n.tr("Value");text:modelData.value || "";echoMode:modelData.sensitive?TextInput.Password:TextInput.Normal;selectByMouse:true;onEditingFinished:root.setEnvironment(index,"value",text)}
        T3Switch{text:T3I18n.tr("Sensitive");checked:!!modelData.sensitive;onToggled:root.setEnvironment(index,"sensitive",checked)}
        T3IconButton{name:"x";tooltipText:T3I18n.tr("Remove");onClicked:root.draftEnvironment=root.draftEnvironment.filter(function(_,i){return i!==index})}
       }
      }
      T3PickerButton{primary:false;text:T3I18n.tr("Add variable");onClicked:root.draftEnvironment=root.draftEnvironment.concat([{name:"",value:"",sensitive:false}])}
     }
    }
   }
   Text{visible:!!root.formError||!!root.state.error;text:root.formError?root.formError:T3I18n.error(root.state.error || "");color:Color.urgent;Layout.fillWidth:true;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}
    T3PickerButton{primary:false;text:T3I18n.tr(root.step===0?"Cancel":"Back");enabled:!root.state.busy;onClicked:if(root.step===0)editor.close();else root.step--}
    T3PickerButton{text:T3I18n.tr(root.step===2?(root.mode==="create"?"Add instance":"Save changes"):"Next");enabled:!root.state.busy;onClicked:if(root.step<2){root.formError="";root.step++}else root.submit()}
   }
  }
 }
 Controls.Popup{id:confirmation;parent:Controls.Overlay.overlay;anchors.centerIn:parent;modal:true;width:Math.min(Style.space(420),parent?parent.width-Style.space(32):Style.space(420));padding:Style.space(20)
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Remove provider instance?");color:root.fg;font.bold:true}
   Text{Layout.fillWidth:true;text:root.selected?root.selected.label+" · "+root.selected.id:"";color:root.muted;textFormat:Text.PlainText}
   RowLayout{Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:confirmation.close()}T3PickerButton{text:T3I18n.tr("Remove");enabled:root.writable;onClicked:if(root.selected){root.awaitingSave=true;root.bridge.requestProviderInstances(root.environment,"remove",{instanceId:root.selected.id})}}}
  }
 }
}
