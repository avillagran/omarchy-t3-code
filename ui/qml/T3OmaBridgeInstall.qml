import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
// Set up a new host with an agent: the chat runs on a host ot3 is connected to
// (this device, or one that reaches the new host directly), starts clean in that
// host's scratch project, and has the setup prompt typed in for review before
// sending. This device writes a one-use invitation (which may ask to manage the
// new host); it is moved host to host over SSH and the prompt names its path only.
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
 property string step:""
 property string invitation:""
 property string profile:"bridge"
 property string environment:"local"
 readonly property var profiles:[
  {id:"bridge",title:T3I18n.tr("OmaBridge only (SSH)"),detail:T3I18n.tr("Only OmaBridge, publishing its SSH. No T3 Code and no AI run there; agents work on it from here through OmaBridge, with no port open.")},
  {id:"headless",title:T3I18n.tr("Headless server"),detail:T3I18n.tr("T3 Code as a background service and OmaBridge. For servers and VPSs without a desktop.")},
  {id:"ot3",title:T3I18n.tr("Omarchy with ot3"),detail:T3I18n.tr("The full Omarchy T3 Code client, for coding on that device too.")},
  {id:"t3code",title:T3I18n.tr("T3 Code and OmaBridge"),detail:T3I18n.tr("For a desktop without Omarchy where you code locally.")}]
 // Hosts the chat can run on: every connected environment.
 readonly property var environments:((bridge.state || {}).environments || []).filter(function(e){return e.state==="ready"})
 readonly property int defaultEnvironment:Math.max(0,environments.findIndex(function(e){return e.id==="local"}))
 readonly property var runsOn:environments.find(function(e){return e.id===root.environment}) || null
 // A host's scratch project, where the chat starts clean. T3 marks it isScratch
 // or names its folder in the host's settings; older hosts by the folder alone.
 function scratchFor(environment){
  var folderOf=function(path){return String(path || "").replace(/[/\\]+$/,"")}
  var configured=folderOf(((bridge.catalogs || {})[environment] || {}).scratchWorkspaceRoot)
  return ((bridge.state || {}).projects || []).find(function(p){
   var folder=folderOf(p.workspaceRoot)
   return p.environmentId===environment&&(p.isScratch===true || (!!configured&&folder===configured) || /\/\.t3\/scratch$/.test(folder))
  }) || null
 }
 readonly property var scratch:scratchFor(environment)
 readonly property var services:(published.services || []).filter(function(s){return s==="t3" || s==="ssh"})
 readonly property bool ready:!!device&&!!published.active&&services.length>0&&!!scratch
 parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(620),parent?parent.width-Style.space(32):Style.space(620));padding:Style.space(16);modal:true
 closePolicy:busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
 background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
 function errorText(code){
  if(code==="invalid_host")return T3I18n.tr("Use an SSH host, alias or user@host.")
  if(code==="omabridge_not_published" || code==="omabridge_service_not_published")return T3I18n.tr("Publish this device before inviting another one.")
  if(code==="ssh_environment_required")return T3I18n.tr("The invitation is copied to that host over SSH; choose a host connected by SSH.")
  return T3I18n.error(code)
 }
 function begin(index){
  device=null;published={};error="";invitation="";step="";busy=true;host.text="";profile="bridge";manage.checked=true;shareSsh.checked=false
  var e=environments[index>=0&&index<environments.length?index:defaultEnvironment];environment=e?e.id:"local"
  if(environment!=="local"&&bridge.ensureCatalog)bridge.ensureCatalog(environment)
  step="read";requestId=bridge.requestOmaBridgeManage({action:"read"});if(!requestId){busy=false;error="disconnected"}
  open()
 }
 // The invitation is created first: it is the one trust change, confirmed by this dialog.
 function start(){
  if(busy || !ready || !host.text.trim())return
  var wanted=services.filter(function(s){return s==="t3" || shareSsh.checked})
  error="";busy=true;step="invite"
  requestId=bridge.requestOmaBridgeManage({action:"invite",services:wanted.length?wanted:[services[0]],qr:false,ttl:3600,manage:manage.checked,confirmed:true})
  if(!requestId){busy=false;error="disconnected"}
 }
 function next(op,values){step=op;requestId=bridge.requestOmaBridgeInstall(op,values);if(!requestId){busy=false;error="disconnected"}}
 onClosed:{invitation="";error="";step=""}
 Connections{target:root.bridge
  function onOmabridgeManageFinished(id,text,exitCode){
   if(id!==root.requestId)return
   root.requestId="";var doc={};try{doc=JSON.parse(text)}catch(e){doc={error:"invalid_response"}}
   if(exitCode!==0 || doc.error&&doc.available===undefined){root.busy=false;root.error=doc.error || "invalid_response";return}
   root.device=doc.device || null;root.published=doc.published || ({})
   if(root.step==="invite"&&doc.created&&doc.created.path){root.next("omabridge-invite-handoff",{environment:root.environment,invitation:doc.created.path});return}
   root.busy=false
  }
  function onOmabridgeInstallAnswer(id,result,error){
   if(id!==root.requestId)return
   root.requestId=""
   if(error || !result){root.busy=false;root.error=error || "invalid_response";return}
   if(root.step==="omabridge-invite-handoff"){
    root.invitation=result.path || ""
    root.next("omabridge-install-prompt",{host:host.text.trim(),profile:root.profile,manage:manage.checked,invitation:root.invitation,device:root.device?root.device.name:"",runsOn:root.environment==="local"?"":(root.runsOn?root.runsOn.name:"")});return
   }
   root.busy=false
   if(!result.prompt){root.error="invalid_response";return}
   if(!root.scratch || !root.bridge.startPromptedChat(root.environment,root.scratch.id,result.prompt)){root.error="management_busy";return}
   root.close()
  }
 }
 contentItem:ColumnLayout{spacing:Style.space(8)
  Text{Layout.fillWidth:true;text:T3I18n.tr("Set up a new host with your AI");color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;text:T3I18n.tr("A new chat opens with the setup instructions typed in. Review them and send when ready. The agent works over SSH and pairs the device with this one through a one-use invitation that never appears in the chat.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
   Text{text:T3I18n.tr("Run on");color:root.fg;font.pixelSize:Style.font.caption;Layout.alignment:Qt.AlignTop}
   Text{objectName:"omabridgeInstallRunsOn";text:root.runsOn?root.runsOn.name:"";color:root.fg;font.pixelSize:Style.font.caption;font.bold:true;textFormat:Text.PlainText;Layout.alignment:Qt.AlignTop}
   Text{Layout.fillWidth:true;visible:root.environment!=="local";text:T3I18n.tr("The agent runs on that host and reaches the new one from there, for example when only it has direct access.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Item{Layout.fillWidth:true;visible:root.environment==="local"}
  }
  RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
   Text{text:T3I18n.tr("New host (SSH)");color:root.fg;font.pixelSize:Style.font.caption}
   T3PickerField{id:host;objectName:"omabridgeInstallHost";Layout.fillWidth:true;implicitHeight:Style.space(32);font.pixelSize:Style.font.caption;placeholderText:"server, user@host";selectByMouse:true;enabled:!root.busy}
  }
  // Profiles as selectable cards, two per row.
  GridLayout{Layout.fillWidth:true;columns:2;columnSpacing:Style.space(8);rowSpacing:Style.space(8)
   Repeater{model:root.profiles
    delegate:Controls.AbstractButton{id:card;required property var modelData
     objectName:"omabridgeInstallProfile_"+modelData.id;Layout.fillWidth:true;Layout.fillHeight:true;Layout.preferredWidth:1
     checkable:false;enabled:!root.busy;hoverEnabled:true;padding:Style.space(10)
     readonly property bool selected:root.profile===modelData.id
     onClicked:root.profile=modelData.id
     Accessible.role:Accessible.RadioButton;Accessible.name:modelData.title;Accessible.checked:selected
     background:Rectangle{radius:Style.space(8);color:card.selected?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):Qt.rgba(root.fg.r,root.fg.g,root.fg.b,card.hovered?0.06:0.03);border.width:1;border.color:card.selected?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12)}
     contentItem:ColumnLayout{spacing:Style.space(2)
      RowLayout{Layout.fillWidth:true;spacing:Style.space(6)
       Rectangle{width:Style.space(12);height:width;radius:width/2;color:"transparent";border.width:1;border.color:card.selected?Color.accent:root.muted
        Rectangle{anchors.centerIn:parent;width:Style.space(6);height:width;radius:width/2;color:Color.accent;visible:card.selected}}
       Text{Layout.fillWidth:true;text:card.modelData.title;color:root.fg;font.pixelSize:Style.font.caption;font.bold:card.selected;elide:Text.ElideRight}
      }
      Text{Layout.fillWidth:true;Layout.fillHeight:true;verticalAlignment:Text.AlignTop;text:card.modelData.detail;color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     }
     HoverHandler{cursorShape:card.enabled?Qt.PointingHandCursor:Qt.ArrowCursor}
    }
   }
  }
  T3Switch{id:manage;objectName:"omabridgeInstallManage";Layout.fillWidth:true;text:T3I18n.tr("Let this device manage it");checked:true;enabled:!root.busy}
  T3Switch{id:shareSsh;objectName:"omabridgeInstallSsh";Layout.fillWidth:true;visible:root.services.indexOf("ssh")>=0;text:T3I18n.tr("Also let it reach this device's SSH");checked:false;enabled:!root.busy}
  Text{Layout.fillWidth:true;visible:!root.busy&&!root.ready&&!root.error;color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap
   text:!root.device?T3I18n.tr("This device has no OmaBridge identity yet."):!root.published.active?T3I18n.tr("Publish this device before inviting another one."):T3I18n.tr("That host has no project for chats without a folder yet. Open T3 Code there once.")}
  Text{objectName:"omabridgeInstallError";Layout.fillWidth:true;visible:!!root.error;text:root.errorText(root.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}
   T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.busy;onClicked:root.close()}
   T3PickerButton{objectName:"omabridgeInstallStart";text:T3I18n.tr("Open chat");enabled:!root.busy&&root.ready&&!!host.text.trim();onClicked:root.start()}
  }
 }
}
