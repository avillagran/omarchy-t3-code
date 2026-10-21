import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
// "Install on another device with your AI": choose the host and what it should
// run, then a new chat opens with the setup prompt typed in, for review before
// sending. This device writes a one-use invitation (which may ask to manage the
// new device); the prompt names its path only and the agent copies it over SSH.
Controls.Popup {
 id:root
 required property var bridge
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property var device:null
 property var published:({})
 property bool busy:false
 property string error:""
 property string requestId:""
 property string invitation:""
 property string profile:"headless"
 readonly property var profiles:[
  {id:"headless",title:T3I18n.tr("Headless server"),detail:T3I18n.tr("T3 Code as a background service and OmaBridge. For servers and VPSs without a desktop.")},
  {id:"ot3",title:T3I18n.tr("Omarchy with ot3"),detail:T3I18n.tr("The full Omarchy T3 Code client, for coding on that device too.")},
  {id:"t3code",title:T3I18n.tr("T3 Code and OmaBridge"),detail:T3I18n.tr("For a desktop without Omarchy where you code locally.")}]
 // The chat opens in this device's scratch project when there is one.
 readonly property var projects:((bridge.state || {}).projects || []).filter(function(p){return p.environmentId==="local"})
 // T3 marks it isScratch; servers that do not are recognized by its folder.
 function scratch(p){return !!p&&(p.isScratch===true || /\/\.t3\/scratch\/?$/.test(String(p.workspaceRoot || "")))}
 function projectName(p){return scratch(p)?T3I18n.tr("No project"):p.title}
 readonly property int defaultProject:Math.max(0,projects.findIndex(scratch))
 readonly property var services:(published.services || []).filter(function(s){return s==="t3" || s==="ssh"})
 readonly property bool ready:!!device&&!!published.active&&services.length>0
 parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(560),parent?parent.width-Style.space(32):Style.space(560));padding:Style.space(20);modal:true
 closePolicy:busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
 background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
 function errorText(code){
  if(code==="invalid_host")return T3I18n.tr("Use an SSH host, alias or user@host.")
  if(code==="omabridge_not_published" || code==="omabridge_service_not_published")return T3I18n.tr("Publish this device before inviting another one.")
  return T3I18n.error(code)
 }
 function begin(index){
  device=null;published={};error="";invitation="";busy=true;host.text="";profile="headless";manage.checked=true;shareSsh.checked=false
  project.currentIndex=index>=0&&index<projects.length?index:defaultProject
  requestId=bridge.requestOmaBridgeManage({action:"read"});if(!requestId){busy=false;error="disconnected"}
  open()
 }
 // The invitation is created first: it is the one trust change, confirmed by this dialog.
 function start(){
  if(busy || !ready)return
  var wanted=services.filter(function(s){return s==="t3" || shareSsh.checked})
  error="";busy=true
  requestId=bridge.requestOmaBridgeManage({action:"invite",services:wanted.length?wanted:[services[0]],qr:false,ttl:3600,manage:manage.checked,confirmed:true})
  if(!requestId){busy=false;error="disconnected"}
 }
 function prompt(){
  requestId=bridge.requestOmaBridgeInstallPrompt({host:host.text.trim(),profile:profile,manage:manage.checked,invitation:invitation,device:device?device.name:""})
  if(!requestId){busy=false;error="disconnected"}
 }
 onClosed:{invitation="";error=""}
 Connections{target:root.bridge
  function onOmabridgeManageFinished(id,text,exitCode){
   if(id!==root.requestId)return
   root.requestId="";var doc={};try{doc=JSON.parse(text)}catch(e){doc={error:"invalid_response"}}
   if(exitCode!==0 || doc.error&&doc.available===undefined){root.busy=false;root.error=doc.error || "invalid_response";return}
   root.device=doc.device || null;root.published=doc.published || ({})
   if(doc.created&&doc.created.path){root.invitation=doc.created.path;root.prompt();return}
   root.busy=false
  }
  function onOmabridgeInstallPromptReady(id,text,error){
   if(id!==root.requestId)return
   root.requestId="";root.busy=false
   if(error || !text){root.error=error || "invalid_response";return}
   var p=root.projects[project.currentIndex]
   if(!p || !root.bridge.startPromptedChat(p.environmentId,p.id,text)){root.error="management_busy";return}
   root.close()
  }
 }
 contentItem:ColumnLayout{spacing:Style.space(12)
  Text{Layout.fillWidth:true;text:T3I18n.tr("Set up a new host with your AI");color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;text:T3I18n.tr("A new chat opens with the setup instructions typed in. Review them and send when ready. The agent works over SSH and pairs the device with this one through a one-use invitation that never appears in the chat.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;text:T3I18n.tr("SSH host");color:root.fg;font.pixelSize:Style.font.caption}
  T3PickerField{id:host;objectName:"omabridgeInstallHost";Layout.fillWidth:true;placeholderText:"server, user@host";selectByMouse:true;enabled:!root.busy}
  Repeater{model:root.profiles
   delegate:Controls.RadioButton{required property var modelData;objectName:"omabridgeInstallProfile_"+modelData.id;Layout.fillWidth:true;checked:root.profile===modelData.id;enabled:!root.busy;onClicked:root.profile=modelData.id
    contentItem:Column{leftPadding:Style.space(28);spacing:Style.space(2)
     Text{width:parent.width-parent.leftPadding;text:modelData.title;color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     Text{width:parent.width-parent.leftPadding;text:modelData.detail;color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    }
   }
  }
  T3Switch{id:manage;objectName:"omabridgeInstallManage";text:T3I18n.tr("Let this device manage it");checked:true;enabled:!root.busy}
  T3Switch{id:shareSsh;objectName:"omabridgeInstallSsh";visible:root.services.indexOf("ssh")>=0;text:T3I18n.tr("Also let it reach this device's SSH");checked:false;enabled:!root.busy}
  RowLayout{Layout.fillWidth:true
   Text{text:T3I18n.tr("Chat in");color:root.fg;font.pixelSize:Style.font.caption}
   Controls.ComboBox{id:project;objectName:"omabridgeInstallProject";Layout.fillWidth:true;model:root.projects.map(root.projectName);enabled:!root.busy}
  }
  Text{Layout.fillWidth:true;visible:!root.busy&&!root.ready&&!root.error;text:root.device?T3I18n.tr("Publish this device before inviting another one."):T3I18n.tr("This device has no OmaBridge identity yet.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  Text{objectName:"omabridgeInstallError";Layout.fillWidth:true;visible:!!root.error;text:root.errorText(root.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}
   T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.busy;onClicked:root.close()}
   T3PickerButton{objectName:"omabridgeInstallStart";text:T3I18n.tr("Open chat");enabled:!root.busy&&root.ready&&!!host.text.trim()&&root.projects.length>0;onClicked:root.start()}
  }
 }
}
