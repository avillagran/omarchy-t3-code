import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

ColumnLayout {
 id:root
 required property var bridge
 readonly property var store:bridge.connectionPreferences || ({busy:false,error:"",client:{},github:{environments:[]}})
 readonly property var client:store.client || ({})
 readonly property var github:store.github || ({environments:[]})
 readonly property var machines:(bridge.connectionSettings || {}).environments || []
 readonly property var enabledMachines:((bridge.connectionSettings || {}).localEnabled!==false&&!!store.primaryEnvironmentId?[{id:"local",name:(((bridge.state || {}).environments || []).find(function(e){return e.id==="local"}) || {}).name || "T3 Code"}]:[]).concat(machines.filter(function(machine){return machine.enabled}))
 readonly property var rows:(github.environments || []).filter(function(entry){return enabledMachines.some(function(machine){return root.canonicalId(machine.id)===entry.environmentId})})
 property bool balancingExpanded:false
 property bool sharingExpanded:false
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.60)
 function canonicalId(id){return id==="local"?(store.primaryEnvironmentId || ""):id}
 function label(id){var machine=enabledMachines.find(function(m){return root.canonicalId(m.id)===id});return machine?machine.name:id}
 function read(){if(bridge.manageConnectionPreferences)bridge.manageConnectionPreferences("read",{})}
 function clientPatch(values){bridge.manageConnectionPreferences("client-update",values)}
 function githubPatch(id,permission){bridge.manageConnectionPreferences("github-update",{environmentId:id,permission:permission})}
 function preference(weight){if(weight===undefined||weight===50)return 50;if(weight===0)return 0;return weight<50?25:100}
 function balancingSummary(){if(!client.loadBalancingEnabled)return T3I18n.tr("Off");var weights=client.loadBalancingWeights || {};var summary=enabledMachines.filter(function(m){return preference(weights[canonicalId(m.id)])!==50}).map(function(m){var p=preference(weights[canonicalId(m.id)]);return m.name+" "+T3I18n.tr(p===0?"Manual only":p===25?"Less often":"Prefer").toLowerCase()});return summary.length?summary.join(" · "):T3I18n.tr("On")}
 function sharingSummary(){var entries=rows.filter(function(e){return e.permission!=="off"});return entries.length?entries.map(function(e){return label(e.environmentId)+" "+T3I18n.tr(e.permission==="read"?"read PRs":"read and act")}).join(" · "):T3I18n.tr("Off")}
 spacing:Style.space(16)
 Component.onCompleted:read()
 Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
 T3PreferenceSection{Layout.fillWidth:true;visible:root.enabledMachines.length>=2;title:"Load balancing";icon:"sliders-horizontal"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(8)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12)
    T3PickerButton{objectName:"connectionBalancingFold";primary:false;text:T3I18n.tr(root.balancingExpanded?"Hide":"Show");onClicked:root.balancingExpanded=!root.balancingExpanded}
    Text{Layout.fillWidth:true;text:root.balancingSummary();color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    T3Switch{objectName:"connectionBalancingEnabled";checked:!!root.client.loadBalancingEnabled;enabled:!root.store.busy&&!!root.client.revision;Accessible.name:T3I18n.tr("Automatically balance load");onToggled:root.clientPatch({loadBalancingEnabled:checked})}
   }
   ColumnLayout{Layout.fillWidth:true;visible:root.balancingExpanded;spacing:Style.space(9)
    Text{Layout.fillWidth:true;Layout.margins:Style.space(12);text:T3I18n.tr("New threads in shared projects start on the machine with the most free CPU and memory, weighted by each machine's preference.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Repeater{model:root.enabledMachines
     delegate:RowLayout{required property var modelData;Layout.fillWidth:true;Layout.margins:Style.space(12)
      Text{Layout.fillWidth:true;text:modelData.name;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      T3Select{objectName:"connectionLoadWeight_"+modelData.id;Layout.preferredWidth:Style.space(150);popupAbove:false;model:["Prefer","Normal","Less often","Manual only"];translateLabels:true;currentIndex:[100,50,25,0].indexOf(root.preference((root.client.loadBalancingWeights || {})[root.canonicalId(modelData.id)]));enabled:!root.store.busy&&!!root.client.revision&&!!root.client.loadBalancingEnabled&&!!root.canonicalId(modelData.id);Accessible.name:modelData.name+" "+T3I18n.tr("load preference");onActivated:function(i){root.clientPatch({environmentId:root.canonicalId(modelData.id),weight:[100,50,25,0][i]})}}
     }
    }
   }
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;visible:root.rows.length>=2;title:"GitHub sharing";icon:"github"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(8)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12)
    T3PickerButton{objectName:"connectionSharingFold";primary:false;text:T3I18n.tr(root.sharingExpanded?"Hide":"Show");onClicked:root.sharingExpanded=!root.sharingExpanded}
    Text{Layout.fillWidth:true;text:root.sharingSummary();color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   }
   ColumnLayout{Layout.fillWidth:true;visible:root.sharingExpanded;spacing:Style.space(9)
    Text{Layout.fillWidth:true;Layout.margins:Style.space(12);text:T3I18n.tr("Machines you trust here can read PR data through each other's GitHub access. Enable both machines. Read and act may use broader permissions than the machine that owns them. This applies only to this device.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Repeater{model:root.rows
     delegate:RowLayout{required property var modelData;Layout.fillWidth:true;Layout.margins:Style.space(12)
      Text{Layout.fillWidth:true;text:root.label(modelData.environmentId);color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      T3Select{objectName:"connectionGithubSharing_"+modelData.environmentId;Layout.preferredWidth:Style.space(150);popupAbove:false;model:["Off","Read PRs","Read and act"];translateLabels:true;currentIndex:Math.max(0,["off","read","read-write"].indexOf(modelData.permission));enabled:!root.store.busy&&!!modelData.available&&!!root.store.catalogRevision;Accessible.name:root.label(modelData.environmentId)+" GitHub routing";onActivated:function(i){root.githubPatch(modelData.environmentId,["off","read","read-write"][i])}}
     }
    }
   }
  }}
 }
}
