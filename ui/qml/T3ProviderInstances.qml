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
 function setupOf(id){return (bridge.providerSetup || {})[id] || ({})}
 function load(){if(bridge&&environment&&environment!=="*"&&typeof bridge.requestProviderInstances==="function")bridge.requestProviderInstances(environment,"read")}
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
 Connections{target:root.bridge;ignoreUnknownSignals:true;function onAcpAgentPrepared(agent){if(!editor.opened)return;root.chooseDriver("acpRegistry");label.text=agent.name;instanceId.text=root.suggestedId(agent.name);root.draftConfig={source:"registry",agentId:agent.id,distribution:"auto"};root.step=1}}
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
   // Install progress and sign-in steps of providers with a flow in progress or just finished.
   Repeater{model:root.instances.filter(function(i){var s=root.setupOf(i.id);return !!s.state||!!s.error})
    ColumnLayout{required property var modelData;objectName:"providerSetup_"+modelData.id;Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(6)
     readonly property var setup:root.setupOf(modelData.id)
     readonly property var st:setup.state || ({})
     readonly property var interaction:st.interaction || null
     Text{Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption;color:parent.setup.error||parent.st.phase==="failed"?Color.urgent:root.fg
      text:modelData.label+" · "+(parent.setup.error?T3I18n.error(parent.setup.error):parent.setup.kind==="install"?(parent.st.phase==="downloading"&&parent.st.totalBytes?T3I18n.tr("Downloading… {percent}%",{percent:Math.round(100*parent.st.downloadedBytes/parent.st.totalBytes)}):T3I18n.tr(({idle:"Not installed",downloading:"Downloading…",extracting:"Installing…",verifying:"Verifying…",succeeded:"Installed",failed:"Install failed",cancelled:"Install cancelled"})[parent.st.phase]||String(parent.st.phase||""))):T3I18n.tr(({idle:"Signed out",starting:"Starting sign-in…",waiting:"Waiting for you to sign in",verifying:"Verifying…",succeeded:"Signed in",failed:"Sign-in failed",cancelled:"Sign-in cancelled"})[parent.st.phase]||String(parent.st.phase||"")))+(parent.st.message?" · "+parent.st.message:"")}
     RowLayout{visible:!!parent.interaction&&(parent.interaction.type==="browser"||parent.interaction.type==="deviceCode");spacing:Style.space(8)
      Text{visible:!!parent.parent.interaction&&parent.parent.interaction.type==="deviceCode";text:parent.parent.interaction?T3I18n.tr("Code: {code}",{code:parent.parent.interaction.userCode||""}):"";color:root.fg;font.family:"monospace";font.pixelSize:Style.font.body}
      T3PickerButton{objectName:"providerOpenSignIn_"+modelData.id;implicitHeight:Style.space(28);text:T3I18n.tr("Open sign-in page");onClicked:{var i=parent.parent.interaction;if(/^https?:\/\//i.test(i.url||""))Qt.openUrlExternally(i.url);if(i.type==="browser"&&i.requiresConsent)root.bridge.providerSetupAction(root.environment,modelData.id,{action:"respond",flowId:parent.parent.st.flowId,interactionId:i.id,response:{type:"browser",action:"accept"}})}}
     }
     ColumnLayout{id:credentialForm;visible:!!parent.interaction&&parent.interaction.type==="credentials";spacing:Style.space(4);property var values:({})
      Repeater{model:parent.interaction&&parent.interaction.fields?parent.interaction.fields:[]
       Controls.TextField{required property var modelData;Layout.preferredWidth:Style.space(280);placeholderText:modelData.label;echoMode:modelData.secret?TextInput.Password:TextInput.Normal;color:root.fg;onTextChanged:{var v=Object.assign({},credentialForm.values);v[modelData.name]=text;credentialForm.values=v}}}
      T3PickerButton{implicitHeight:Style.space(28);text:T3I18n.tr("Continue");onClicked:root.bridge.providerSetupAction(root.environment,modelData.id,{action:"respond",flowId:parent.parent.st.flowId,interactionId:parent.parent.interaction.id,response:{type:"credentials",values:credentialForm.values}})}
     }
     Text{visible:!!parent.interaction&&parent.interaction.type==="terminal";Layout.fillWidth:true;text:parent.interaction&&parent.interaction.output?String(parent.interaction.output).slice(-1200):"";color:root.muted;font.family:"monospace";font.pixelSize:Style.font.caption;wrapMode:Text.WrapAnywhere}
     T3PickerButton{visible:["starting","waiting","verifying","downloading","extracting"].indexOf(parent.st.phase)>=0;primary:false;implicitHeight:Style.space(26);text:T3I18n.tr("Cancel");onClicked:{var s=parent.st;if(parent.setup.kind==="install")root.bridge.providerSetupAction(root.environment,modelData.id,{action:"cancel-install",operationId:s.operationId});else root.bridge.providerSetupAction(root.environment,modelData.id,{action:"cancel-login",flowId:s.flowId})}}
    }
   }
   Repeater{model:root.instances
    RowLayout{required property var modelData;Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);Layout.bottomMargin:Style.space(8);spacing:Style.space(8)
     T3ProviderIcon{provider:({driverKind:modelData.driver});tint:root.fg;implicitWidth:Style.space(20)}
     ColumnLayout{Layout.fillWidth:true;spacing:Style.space(2)
      Text{Layout.fillWidth:true;text:modelData.label+" · "+modelData.id;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:(modelData.authStatus==="authenticated"?T3I18n.tr("Signed in"):modelData.installed===false?T3I18n.tr("Not installed"):T3I18n.tr("Sign in on this environment"))+(modelData.currentVersion?" · "+modelData.currentVersion:"")+(["queued","running"].indexOf(String(modelData.updateStatus))>=0?" · "+T3I18n.tr("Updating…"):"");color:root.muted;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
     }
     T3PickerButton{objectName:"providerUpdate_"+modelData.id;visible:modelData.versionStatus==="behind_latest"&&modelData.canUpdate===true&&["queued","running"].indexOf(String(modelData.updateStatus))<0;primary:false;implicitHeight:Style.space(28);enabled:!root.setupOf(modelData.id).busy;text:T3I18n.tr("Update to {version}",{version:modelData.latestVersion||""});onClicked:root.bridge.providerSetupAction(root.environment,modelData.id,{action:"update",driver:modelData.driver})}
     T3PickerButton{objectName:"providerInstall_"+modelData.id;visible:modelData.installed===false&&modelData.canInstall===true;primary:false;implicitHeight:Style.space(28);enabled:!root.setupOf(modelData.id).busy;text:T3I18n.tr("Install");onClicked:root.bridge.providerSetupAction(root.environment,modelData.id,{action:"install"})}
     T3PickerButton{objectName:"providerSignIn_"+modelData.id;visible:modelData.canAuthenticate===true&&modelData.authStatus!=="authenticated"&&modelData.installed!==false;primary:false;implicitHeight:Style.space(28);enabled:!root.setupOf(modelData.id).busy;text:T3I18n.tr("Sign in");onClicked:root.bridge.providerSetupAction(root.environment,modelData.id,{action:"login"})}
     T3PickerButton{objectName:"providerSignOut_"+modelData.id;visible:modelData.canLogout===true&&modelData.authStatus==="authenticated";primary:false;implicitHeight:Style.space(28);enabled:!root.setupOf(modelData.id).busy;text:T3I18n.tr("Sign out");onClicked:root.bridge.providerSetupAction(root.environment,modelData.id,{action:"logout"})}
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
      // Agents from the ACP registry: search, install, then fill in the instance.
      RowLayout{Layout.fillWidth:true;visible:root.drivers.some(function(d){return d.id==="acpRegistry"});spacing:Style.space(6)
       Controls.TextField{id:acpSearch;objectName:"acpRegistrySearch";Layout.fillWidth:true;placeholderText:T3I18n.tr("Search the ACP agent registry");color:root.fg;onAccepted:if(typeof root.bridge.acpRegistryAction==="function")root.bridge.acpRegistryAction(root.environment,{action:"search",query:text})}
       T3PickerButton{primary:false;implicitHeight:Style.space(30);text:T3I18n.tr("Search");enabled:!(root.bridge.acpRegistry||{}).busy;onClicked:root.bridge.acpRegistryAction(root.environment,{action:"search",query:acpSearch.text})}
      }
      Text{visible:!!(root.bridge.acpRegistry||{}).error;Layout.fillWidth:true;wrapMode:Text.Wrap;text:T3I18n.error((root.bridge.acpRegistry||{}).error||"");color:Color.urgent;font.pixelSize:Style.font.caption}
      Repeater{model:((root.bridge.acpRegistry||{}).agents||[]).slice(0,8)
       RowLayout{required property var modelData;Layout.fillWidth:true;spacing:Style.space(6)
        ColumnLayout{Layout.fillWidth:true;spacing:0
         Text{Layout.fillWidth:true;text:modelData.name+" "+modelData.version;color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
         Text{Layout.fillWidth:true;text:modelData.description||"";color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
        }
        T3PickerButton{objectName:"acpRegistryAdd";implicitHeight:Style.space(28);text:T3I18n.tr("Install and add");enabled:!(root.bridge.acpRegistry||{}).busy;onClicked:root.bridge.acpRegistryAction(root.environment,{action:"prepare",agent:modelData})}
       }
      }
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
