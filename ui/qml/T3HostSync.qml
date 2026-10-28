import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Preferences → Connections: leave a connected SSH host with this computer's
// working setup (host_sync). Preview first, nothing deleted, replaced files are
// backed up on the host, one result per item.
ColumnLayout {
 id:root
 required property var bridge
 readonly property var sync:bridge.hostSync || ({})
 readonly property var hosts:((bridge.connectionSettings || {}).environments || []).filter(function(e){return e.enabled&&(e.routes || []).some(function(r){return r.kind==="SshConnectionTarget"})})
 property string host:""
 readonly property string hostId:hosts.some(function(h){return h.id===root.host})?host:(hosts.length?hosts[0].id:"")
 readonly property string hostName:(hosts.find(function(h){return h.id===root.hostId}) || {}).name || ""
 readonly property bool busy:!!sync.busy
 readonly property var preview:sync.preview&&sync.preview.environment===hostId?sync.preview:null
 readonly property var result:sync.result&&sync.result.environment===hostId?sync.result:null
 readonly property var last:(sync.last || {})[hostId] || null
 readonly property string error:sync.environment===hostId?(sync.error || ""):""
 property var chosen:({vault:true,instructions:true,skills:true,config:true})
 readonly property var ids:["vault","instructions","skills","config"]
 readonly property var names:({vault:"Development vault",instructions:"Agent instructions",skills:"Skills",config:"Harness settings"})
 readonly property var details:({vault:"The same vault, cloned or updated at the same place in the host's home.",instructions:"Global instructions of Claude, Codex, OpenCode and Gemini.",skills:"Your skills, with links resolved. Skills the host links itself, such as Omarchy's, are kept.",config:"Settings, agents, commands and MCP servers without credentials. Paths in your home point to the host's home."})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.60)
 function item(id){return ((preview || {}).items || []).find(function(i){return i.id===id}) || null}
 function done(id){return ((result || {}).items || []).find(function(i){return i.id===id}) || null}
 function selected(){return ids.filter(function(id){var i=root.item(id);return root.chosen[id]&&!!i&&i.available&&i.pending>0})}
 function choose(id,on){var c=Object.assign({},chosen);c[id]=on;chosen=c}
 function request(action,values){if(typeof bridge.hostSyncAction==="function")bridge.hostSyncAction(action,action==="read"?"":hostId,values)}
 function runPreview(){request("preview")}
 function apply(){request("apply",{items:selected(),confirmed:true})}
 function signIn(){request("github-login")}
 function errorText(code){
  var known={host_sync_needs_ssh:"This host has no SSH connection. Sync needs SSH, directly or through OmaBridge.",host_sync_busy:"Another sync is running.",ssh_unavailable:"Could not reach the host over SSH.",ssh_timeout:"The host did not answer in time.",host_sync_probe_failed:"The host's answer could not be read.",host_sync_apply_failed:"The host could not apply the sync."}
  return known[code]?T3I18n.tr(known[code]):T3I18n.error(code)
 }
 function actionText(a){return T3I18n.tr(({add:"New",update:"Update",same:"Up to date",kept:"Kept: the host's own",skipped:"Not a file on the host",left:"Not sent"})[a] || a)}
 function reasonText(r){return T3I18n.tr(({credentials:"Looks like a credential",system:"Provided by the system","too-large":"Too large",unreadable:"Unreadable","not-a-file":"Not a file on the host"})[r] || r)}
 function countsText(i){
  var c=i.counts || {},parts=[T3I18n.tr("{add} new · {update} to update · {same} up to date",{add:c.add || 0,update:c.update || 0,same:c.same || 0})]
  if(c.kept)parts.push(T3I18n.tr("{count} kept as the host's own",{count:c.kept}))
  if(c.left)parts.push(T3I18n.tr("{count} not sent",{count:c.left}))
  return parts.join(" · ")
 }
 function vaultText(v){
  if(!v)return ""
  if(!v.available)return T3I18n.tr(({host_sync_vault_outside_home:"The vault must be inside your home folder.",host_sync_vault_remote_unsupported:"The vault needs a network remote, such as GitHub, to be cloned on another host."})[v.error] || "Set up Memory/Obsidian on this computer first.")
  if(v.hostState==="nogit"||v.access==="nogit")return T3I18n.tr("Git is not installed on the host.")
  if(v.action==="occupied")return T3I18n.tr("{path} on the host holds something else and is left untouched.",{path:v.path})
  if(v.access==="auth")return T3I18n.tr("The host cannot read {remote}. Sign in to GitHub there, then preview again.",{remote:v.remote})
  if(v.access!=="ok")return T3I18n.tr("The host cannot reach {remote}.",{remote:v.remote})
  return v.action==="clone"?T3I18n.tr("Clone into {path} from {remote}",{path:v.path,remote:v.remote}):T3I18n.tr("Update {path} from {remote}",{path:v.path,remote:v.remote})
 }
 function resultText(r){
  if(!r)return ""
  if(r.id==="vault")return T3I18n.tr(({cloned:"Cloned",pulled:"Updated",occupied:"Left untouched",auth:"Not synced: the host cannot read the remote",unreachable:"Not synced: the host cannot reach the remote",conflict:"Not updated: the pull stopped on a conflict. Nothing was discarded.",nogit:"Git is not installed on the host.",unavailable:"Set up Memory/Obsidian on this computer first."})[r.status] || "Failed")
  var c=r.counts || {},parts=[T3I18n.tr("{added} added · {updated} updated · {unchanged} up to date",{added:c.added || 0,updated:c.updated || 0,unchanged:c.unchanged || 0})]
  if(c.changed)parts.push(T3I18n.tr("{count} changed on the host meanwhile and were not replaced",{count:c.changed}))
  if(c.failed)parts.push(T3I18n.tr("{count} failed",{count:c.failed}))
  return parts.join(" · ")
 }
 spacing:Style.space(8)
 Component.onCompleted:request("read")
 T3PreferenceSection{Layout.fillWidth:true;title:"Sync this computer's setup";icon:"refresh-cw"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(9)
   Text{Layout.fillWidth:true;Layout.margins:Style.space(12);Layout.bottomMargin:0;text:T3I18n.tr("Leave a connected host with the same working setup as this computer. Credentials, sessions and history are never copied: each host keeps its own sign-ins. Nothing is deleted.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{objectName:"hostSyncNoHosts";Layout.fillWidth:true;Layout.margins:Style.space(12);visible:root.hosts.length===0;text:T3I18n.tr("No SSH host is connected. Add one under Environments or with the connection assistants.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12);visible:root.hosts.length>0;spacing:Style.space(8)
    Text{text:T3I18n.tr("Host");color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
    T3Select{objectName:"hostSyncHost";Layout.fillWidth:true;popupAbove:false;model:root.hosts;textRole:"name";enabled:!root.busy;currentIndex:Math.max(0,root.hosts.findIndex(function(h){return h.id===root.hostId}));Accessible.name:T3I18n.tr("Host");onActivated:function(i){root.host=root.hosts[i].id}}
    T3PickerButton{objectName:"hostSyncPreview";primary:!root.preview;outlined:!!root.preview;text:T3I18n.tr(root.preview?"Check again":"Preview");enabled:!root.busy&&!!root.hostId;onClicked:root.runPreview()}
   }
   Text{objectName:"hostSyncLast";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);visible:!!root.last&&!root.result;text:root.last?T3I18n.tr("Last synced {time}",{time:root.bridge.formatTimestamp?root.bridge.formatTimestamp(root.last.at):String(root.last.at)}):"";color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   Text{objectName:"hostSyncError";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);visible:!!root.error;text:root.errorText(root.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);visible:root.busy&&root.sync.environment===root.hostId;text:T3I18n.tr(root.sync.action==="apply"?"Syncing…":"Looking at the host…");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   Repeater{model:root.preview?root.ids:[]
    delegate:ColumnLayout{id:row;required property string modelData
     readonly property var entry:root.item(modelData)
     readonly property var outcome:root.done(modelData)
     Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(4)
     visible:!!entry
     Rectangle{Layout.fillWidth:true;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
     RowLayout{Layout.fillWidth:true
      ColumnLayout{Layout.fillWidth:true;spacing:Style.space(2)
       Text{Layout.fillWidth:true;text:T3I18n.tr(root.names[row.modelData]);color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
       Text{Layout.fillWidth:true;text:T3I18n.tr(root.details[row.modelData]);color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      }
      T3Switch{objectName:"hostSyncItem_"+row.modelData;checked:!!root.chosen[row.modelData]&&!!row.entry&&row.entry.available&&row.entry.pending>0;enabled:!root.busy&&!!row.entry&&row.entry.available&&row.entry.pending>0;Accessible.name:T3I18n.tr(root.names[row.modelData]);onToggled:root.choose(row.modelData,checked)}
     }
     Text{objectName:"hostSyncSummary_"+row.modelData;Layout.fillWidth:true;text:!row.entry?"":row.modelData==="vault"?root.vaultText(row.entry.vault):row.entry.available?root.countsText(row.entry):T3I18n.tr("Nothing here to sync.");color:row.modelData==="vault"&&row.entry&&row.entry.vault&&row.entry.vault.available&&row.entry.vault.access!=="ok"?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
     T3PickerButton{objectName:"hostSyncGithubLogin";visible:row.modelData==="vault"&&!!row.entry&&!!row.entry.vault&&row.entry.vault.access==="auth"&&row.entry.vault.github&&row.entry.vault.gh!=="missing";primary:false;text:T3I18n.tr("Sign in to GitHub on {host}…",{host:root.hostName});enabled:!root.busy;onClicked:root.signIn()}
     Text{visible:row.modelData==="vault"&&!!row.entry&&!!row.entry.vault&&row.entry.vault.access==="auth"&&row.entry.vault.gh==="missing";Layout.fillWidth:true;text:T3I18n.tr("Install the GitHub CLI (gh) on the host to sign in to GitHub.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
     Flow{Layout.fillWidth:true;spacing:Style.space(6);visible:row.modelData!=="vault"&&!!row.entry&&(row.entry.entries || []).some(function(e){return e.action!=="same"})
      Repeater{model:row.entry&&row.modelData!=="vault"?(row.entry.entries || []).filter(function(e){return e.action!=="same"}).slice(0,60):[]
       delegate:Text{required property var modelData;text:modelData.path+" · "+(modelData.reason?root.reasonText(modelData.reason):root.actionText(modelData.action));color:modelData.action==="update"?root.fg:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      }
     }
     Text{objectName:"hostSyncResult_"+row.modelData;visible:!!row.outcome;Layout.fillWidth:true;text:root.resultText(row.outcome);color:row.outcome&&["failed","partial","auth","unreachable","conflict","nogit"].indexOf(row.outcome.status)>=0?Color.urgent:T3Palette.done;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    }
   }
   Text{objectName:"hostSyncBackup";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);visible:!!root.result&&!!root.result.backup;text:root.result&&root.result.backup?T3I18n.tr("Replaced files were backed up on the host in {path}.",{path:root.result.backup}):"";color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{objectName:"hostSyncNothing";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);visible:!!root.preview&&!root.result&&root.selected().length===0;text:T3I18n.tr("Nothing to sync: the host is up to date.");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12);visible:!!root.preview
    Text{Layout.fillWidth:true;text:T3I18n.tr(root.preview&&root.preview.transport==="omabridge"?"Reached through OmaBridge":"Reached over SSH");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    T3PickerButton{objectName:"hostSyncApply";text:T3I18n.tr("Sync");enabled:!root.busy&&root.selected().length>0;onClicked:confirm.open()}
   }
  }}
 }
 Controls.Popup{id:confirm;objectName:"hostSyncConfirm";parent:Controls.Overlay.overlay;anchors.centerIn:parent;modal:true;width:Math.min(Style.space(500),parent?parent.width-Style.space(32):Style.space(500));padding:Style.space(20)
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.dim}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Sync to {host}?",{host:root.hostName});color:root.fg;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:T3I18n.tr("{items} will be copied to {host}. Nothing is deleted; any file that is replaced is backed up on the host first.",{items:root.selected().map(function(id){return T3I18n.tr(root.names[id])}).join(", "),host:root.hostName});color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:confirm.close()}T3PickerButton{objectName:"hostSyncConfirmButton";text:T3I18n.tr("Sync");onClicked:{confirm.close();root.apply()}}}
  }
 }
}
