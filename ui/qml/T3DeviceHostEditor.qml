import QtQuick
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 objectName:"deviceHostEditor"
 required property var bridge
 required property var environments
 property var original:null
 property string hostId:""
 property string label:""
 property string target:""
 property string identity:""
 property string port:""
 property bool advanced:false
 signal saved(var host)
 signal cancelled()
 readonly property bool pending:environments.some(function(e){return (root.bridge.deviceOperations[e] || {}).state==="pending"})
 readonly property var draft:parse(false)
 readonly property bool valid:!!draft&&!!label.trim()
 readonly property bool writable:environments.length>0&&!pending&&root.bridge.deviceAllowed(environments[0],"host-upsert")
 function parse(probe){var p=port.trim(),key=identity.trim(),name=label.trim() || (probe?target.trim():"");if(!/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(hostId)||hostId==="local"||!name||!target.trim()||!/^[^\s-][^\s]*$/.test(target.trim())||p&&(!/^\d+$/.test(p)||Number(p)<1||Number(p)>65535))return null;var h={id:hostId,label:name,target:target.trim()};if(key)h.identityFile=key;if(p)h.port=Number(p);return h}
 function connectionKey(h){return h?JSON.stringify([h.target.trim(),h.port,h.identityFile?h.identityFile.trim():undefined]):""}
 function load(h,source){original=source?JSON.parse(JSON.stringify(source)):null;hostId=h.id;label=h.label || "";target=h.target || "";identity=h.identityFile || "";port=h.port?String(h.port):"";advanced=!!h.identityFile||!!h.port}
 function check(){var h=parse(true);if(h&&!pending)bridge.deviceAction("test-host",{host:h},environments)}
 function save(){if(valid&&writable&&!pending)saved(draft)}
 spacing:Style.space(10)
 Text{Layout.fillWidth:true;text:T3I18n.tr(root.original?"Edit device host":"Add device host");color:Color.popups.text;font.pixelSize:Style.font.body;font.bold:true}
 Text{Layout.fillWidth:true;text:T3I18n.tr("SSH aliases and identity files are resolved on each selected environment.");color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Repeater{model:[{key:"label",name:"Name"},{key:"target",name:"SSH target"}].concat(root.advanced?[{key:"identity",name:"Identity file"},{key:"port",name:"Port"}]:[])
  T3PreferenceField{required property var modelData;Layout.fillWidth:true;field:({key:"deviceHost_"+modelData.key,label:modelData.name,kind:"text",value:root[modelData.key],maximumLength:modelData.key==="port"?5:4096});edit:root[modelData.key];writable:!root.pending;onStaged:function(v){root[modelData.key]=v};onChanged:function(v){root[modelData.key]=v}}
 }
 T3PickerButton{primary:false;text:T3I18n.tr("SSH options");onClicked:root.advanced=!root.advanced}
 Repeater{model:root.environments
  Text{required property string modelData;Layout.fillWidth:true;readonly property var result:root.bridge.deviceOperations[modelData] || ({});readonly property bool matches:result.action==="test-host"&&root.connectionKey((result.input || {}).host)===root.connectionKey(root.parse(true));visible:matches;text:modelData+" · "+T3I18n.tr(result.state==="pending"?"Checking…":result.state==="complete"?result.host&&result.host.kind==="local"?"Already available locally":"Connected":result.state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":"Failed")+(result.error?" · "+T3I18n.error(result.error):"");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 }
 RowLayout{Layout.fillWidth:true
  T3PickerButton{objectName:"deviceHostTest";primary:false;text:T3I18n.tr("Test connection");enabled:!!root.parse(true)&&root.writable&&!root.pending;onClicked:root.check()}
  Item{Layout.fillWidth:true}
  T3PickerButton{objectName:"deviceHostCancel";primary:false;text:T3I18n.tr("Cancel");enabled:!root.pending;onClicked:root.cancelled()}
  T3PickerButton{objectName:"deviceHostSave";text:T3I18n.tr("Save");enabled:root.valid&&root.writable&&!root.pending;onClicked:root.save()}
 }
}
