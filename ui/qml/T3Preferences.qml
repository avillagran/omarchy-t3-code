import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/ProjectModel.js" as Projects
Rectangle {
 id:root
 required property var bridge
 property string shortcuts:""
 property string initialProjectKey:""
 property string projectKey:""
 property string checkoutKey:""
 property bool initialized:false
 signal closed()
 signal usageRequested(bool value)
 signal groupingRequested(bool value)
 property string page:"general"
 property var edits:({})
 property string savingKey:""
 property string savingRequest:""
 function save(key,value){if(!writable)return;bridge.requestSettings(environment,key,value,scope);if(result.busy){savingKey=key;savingRequest=result.requestId}}
 Connections {target:root.bridge
  function onSettingsResultChanged(){var r=root.result;if(r.busy||!root.savingRequest||r.requestId!==root.savingRequest)return;if(!r.error&&!(r.errors || []).length){var next=Object.assign({},root.edits);delete next[root.savingKey];root.edits=next}root.savingKey="";root.savingRequest=""}
 }
 function serverUpdateReason(code){var reasons={backend_owned_by_gui:"the T3 Code desktop app runs this server and updates it itself.",service_required:"this server does not run as a T3 background service.",service_profile_mismatch:"the background service belongs to another T3 home.",ssh_environment_required:"this server is not connected over SSH.",t3_environment_mismatch:"the host answered with another T3 environment.",t3_update_failed:"t3 update did not finish.",t3_update_timeout:"t3 update took longer than 15 minutes.",t3_remote_start_failed:"the server did not come back after updating.",t3_cli_unavailable:"the t3 command was not found on that host.",ssh_unavailable:"the host could not be reached over SSH.",release_index_unavailable:"the T3 release list could not be read.",t3_update_running:"an update is already running.",t3_update_not_available:"no newer release is known yet."};return reasons[code]?T3I18n.tr(reasons[code]):T3I18n.error(code)}
 function clearOverride(key){if(writable)bridge.requestSettings(environment,key,null,scope,true)}
 function stage(key,value){var next=Object.assign({},edits);next[key]=value;edits=next}
 property string environment:bridge.selectedEnvironment || "local"
 readonly property var result:bridge.settingsResult
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 readonly property var projectGroups:Projects.groups((bridge.state.projects || []).filter(function(p){return !p.isScratch}),Object.assign({},bridge.viewState || {},{nativeProjectGrouping:bridge.state.nativeProjectGrouping}))
 readonly property var selectedGroup:projectGroups.find(function(p){return p.key===root.projectKey}) || null
 readonly property var matchingMembers:selectedGroup?selectedGroup.members.filter(function(p){return root.environment==="*"||p.environmentId===root.environment}):[]
 readonly property var scope:({targets:projectKey?matchingMembers.filter(function(p){return !root.checkoutKey||p.key===root.checkoutKey}).map(function(p){return {environment:p.environmentId,projectId:p.id}}):(environment==="*"?environments.map(function(e){return {environment:e.id,projectId:null}}):[{environment:environment,projectId:null}])})
 readonly property string scopeSignature:JSON.stringify(scope)
 readonly property string representativeEnvironment:scope.targets.length?scope.targets[0].environment:environment==="*"?(bridge.selectedEnvironment || "local"):environment
 readonly property bool projectScope:!!projectKey
 readonly property bool supportsScope:page==="storage"||page==="integrations"||page==="general"||page==="source-control"||page==="projects"||page==="archived"||page==="shortcuts"
 onScopeSignatureChanged:if(initialized){edits={};savingKey="";savingRequest="";load()}
 function selectProject(key){var ready=initialized;initialized=false;projectKey=key;checkoutKey="";if(key&&page!=="archived"&&page!=="shortcuts"&&page!=="integrations"&&page!=="storage")page="projects";initialized=ready;if(ready){edits={};load()}}
 function selectEnvironment(id){var ready=initialized;initialized=false;checkoutKey="";environment=id;initialized=ready;if(ready){edits={};load()}}
 function selectInitialProject(){if(!initialProjectKey)return;var group=projectGroups.find(function(g){return g.key===root.initialProjectKey||g.members.some(function(p){return p.key===root.initialProjectKey})});if(group){var ready=initialized;initialized=false;var member=group.members.find(function(p){return p.key===root.initialProjectKey});if(member)environment=member.environmentId;projectKey=group.key;checkoutKey=member?member.key:"";page="projects";initialized=ready;if(ready){edits={};load()}}}
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property bool writable:result.environment===environment&&!result.busy&&!result.error&&result.writable!==false&&(!result.scope||JSON.stringify(result.scope)===scopeSignature)
 Shortcut{sequence:"Ctrl+F";enabled:root.visible&&!(root.bridge.keybindingCaptureActive || false);onActivated:{search.forceActiveFocus();search.selectAll()}}
 color:Color.popups.background
 property string loadedPage:""
 function load(){loadedPage=page;if(page==="licenses"){bridge.requestLicenses(true);return}if(typeof bridge.requestClientSettings==="function")bridge.requestClientSettings();if(page==="archived"){bridge.requestArchive(scope.targets,true);return}if(page==="shortcuts"){bridge.requestKeybindings(scope.targets,true);return}bridge.requestSettings(environment,undefined,undefined,scope);if(typeof bridge.refreshCatalogIn==="function")bridge.refreshCatalogIn(representativeEnvironment)}
 Component.onCompleted:{selectInitialProject();initialized=true;load()}

 property string query:""
 readonly property bool compact:width<Style.space(780)
 readonly property var pages:[{id:"general",name:"General",icon:"settings"},{id:"appearance",name:"Appearance",icon:"layers"},{id:"projects",name:"Projects",icon:"folder-git-2"},{id:"shortcuts",name:"Keyboard shortcuts",icon:"keyboard"},{id:"providers",name:"Providers",icon:"acp"},{id:"integrations",name:"Integrations",icon:"plug"},{id:"source-control",name:"Source control",icon:"git-branch"},{id:"connections",name:"Connections",icon:"server"},{id:"storage",name:"Storage",icon:"hard-drive"},{id:"memory",name:"Memory/Obsidian",icon:"book-open"},{id:"archived",name:"Archive",icon:"archive"},{id:"licenses",name:"Open-source licenses",icon:"file-text",nav:false}]
 readonly property bool licenseSearchMatch:matches("Open-source licenses")||matches("Third-party notices")||matches("Notices for dependencies, assets, and tools used by ot3.")
 readonly property var currentPage:pages.find(function(p){return p.id===root.page}) || pages[0]
 readonly property var sections:[
  {page:"general",title:"New threads",icon:"square-pen",keys:["defaultModelSelection","defaultRuntimeMode","defaultThreadEnvMode","worktreeSubmodules"]},
  {page:"general",title:"Organization",icon:"list-todo",keys:["autoResumeLimitedThreads","snoozeLimitedThreads","sidebarAutoSettleOnMerge","sidebarAutoSettleAfterDays","sidebarAutoSettleKeepPullRequests"]},
  {page:"general",title:"Behavior",icon:"settings",keys:["responseStreamingMode","idleCompaction","enableProviderUpdateChecks","continueThreadsAfterServerUpdate","backgroundActivityProfile"]},
  {page:"general",title:"Projects & threads",icon:"folder-git-2",keys:["newWorktreesStartFromOrigin"]},
  {page:"general",title:"Text generation",icon:"star",keys:["textGenerationModelSelection","threadTitleInstructions"]},
  {page:"general",title:"Telemetry export",icon:"server",keys:["observability.otlpTracesUrl","observability.otlpMetricsUrl","observability.otlpLogsUrl"]},
  {page:"source-control",title:"Repositories",icon:"folder-git-2",keys:["defaultAutoPull","removeAgentCreditsOnMerge","pullRequestMergeMethod"]},
  {page:"source-control",title:"Text generation",icon:"square-pen",keys:["branchNamingMode","branchNamePrefix","branchNameInstructions","sourceControlWritingStyle.mode","sourceControlWritingStyle.customInstructions","sourceControlWritingStyle.followChangeRequestTemplates","sourceControlWriterModelSelection"]},
  {page:"source-control",title:"Review guides",icon:"git-pull-request",keys:["reviewGuidesEnabled","reviewGuideModelSelection","reviewGuideInstructions"]}]
 // Sections that filter themselves (not in `groups`) still count as search results.
 readonly property bool sectionSearchMatch:["T3 Code compatibility","Follow-up behavior","Steer","Plan mode (legacy)","Legacy features","Notifications","Thread notifications","HTML renders","Language"].some(function(t){return root.matches(t)})
 function matches(text){return !query.trim() || (T3I18n.tr(text).toLocaleLowerCase().indexOf(query.trim().toLocaleLowerCase())>=0 || String(text).toLocaleLowerCase().indexOf(query.trim().toLocaleLowerCase())>=0)}
 readonly property var groups:sections.filter(function(s){return root.query.trim() || s.page===root.page || root.page==="projects"&&root.projectScope}).map(function(s){return Object.assign({},s,{fields:s.keys.map(function(key){return (root.result.fields || []).find(function(f){return f.key===key})}).filter(function(f){return f&&(root.matches(s.title)||root.matches(f.label)||root.matches(f.description || ""))})})}).filter(function(s){return s.fields.length>0})
 onQueryChanged:if(settingsScroll.contentItem)settingsScroll.contentItem.contentY=0
 onPageChanged:{if(!supportsScope&&projectKey){projectKey="";checkoutKey=""}if(!supportsScope&&environment==="*")environment=bridge.selectedEnvironment || "local";if(settingsScroll.contentItem)settingsScroll.contentItem.contentY=0;if(initialized&&(["archived","shortcuts","licenses","integrations","providers"].indexOf(page)>=0||["archived","shortcuts","licenses","integrations"].indexOf(loadedPage)>=0))load()}
 RowLayout {
  anchors.fill:parent;spacing:0
  Rectangle {objectName:"preferencesNavigation";Layout.preferredWidth:Style.space(root.compact?185:256);Layout.fillHeight:true;color:Qt.darker(Color.popups.background,1.07)
   Rectangle{anchors.right:parent.right;width:1;height:parent.height;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)}
   ColumnLayout {anchors.fill:parent;anchors.margins:Style.space(12);spacing:Style.space(6)
    RowLayout{Layout.fillWidth:true;Layout.preferredHeight:Style.space(45);Layout.leftMargin:Style.space(8);spacing:Style.space(10)
     T3Icon{name:"settings";tint:root.dim;implicitWidth:Style.space(18)}
     Text{text:T3I18n.tr("Preferences");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold;Layout.fillWidth:true}
    }
    Controls.TextField{id:search;objectName:"preferencesSearch";Layout.fillWidth:true;implicitHeight:Style.space(32);text:root.query;onTextEdited:root.query=text;placeholderText:T3I18n.tr("Search settings");placeholderTextColor:root.dim;color:root.fg;font.pixelSize:Style.font.caption;leftPadding:Style.space(30);rightPadding:Style.space(8);selectByMouse:true
     background:Rectangle{radius:Style.space(7);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:search.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
     T3Icon{anchors.left:parent.left;anchors.leftMargin:Style.space(9);anchors.verticalCenter:parent.verticalCenter;name:"search";tint:root.dim;implicitWidth:Style.space(14)}
     Keys.onEscapePressed:if(root.query){root.query=""}else root.closed()
    }
    Repeater {model:root.pages.filter(function(p){return p.nav!==false})
     Controls.AbstractButton {id:nav;required property var modelData;objectName:"preferencesTab_"+modelData.id;Layout.fillWidth:true;implicitHeight:Style.space(36);hoverEnabled:true;checked:(root.page===modelData.id||root.page==="licenses"&&modelData.id==="general")&&!root.query.trim();Accessible.name:T3I18n.tr(modelData.name);onClicked:{root.query="";root.page=modelData.id}
      leftPadding:Style.space(10);rightPadding:Style.space(8)
      background:Rectangle{radius:Style.space(6);color:nav.checked?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08):nav.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent";border.width:nav.activeFocus?1:0;border.color:Color.accent}
      contentItem:RowLayout{spacing:Style.space(10);T3Icon{name:nav.modelData.icon;tint:nav.checked?root.fg:root.dim;implicitWidth:Style.space(16)}Text{text:T3I18n.tr(nav.modelData.name);color:nav.checked?root.fg:root.dim;font.pixelSize:Style.font.caption;font.weight:nav.checked?Font.Medium:Font.Normal;Layout.fillWidth:true;elide:Text.ElideRight}}
      HoverHandler{cursorShape:Qt.PointingHandCursor}
     }
    }
    Item{Layout.fillHeight:true}
    Controls.AbstractButton {id:back;objectName:"preferencesBack";Layout.fillWidth:true;implicitHeight:Style.space(34);onClicked:root.closed();hoverEnabled:true
     background:Rectangle{radius:Style.space(6);color:back.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent";border.width:back.activeFocus?1:0;border.color:Color.accent}
     contentItem:RowLayout{spacing:Style.space(8);T3Icon{name:"chevron-left";tint:root.dim;implicitWidth:Style.space(14)}Text{text:T3I18n.tr("Back to chat");font.pixelSize:Style.font.caption;color:root.dim;Layout.fillWidth:true}}
     leftPadding:Style.space(10);rightPadding:Style.space(10);HoverHandler{cursorShape:Qt.PointingHandCursor}
    }
    RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(8);spacing:Style.space(8)
     T3Icon{name:root.environment==="local"?"laptop":"server";tint:Color.accent;implicitWidth:Style.space(14)}
     Text{text:root.environment==="*"?T3I18n.tr("All environments"):(root.environments.find(function(e){return e.id===root.environment}) || {}).name || root.environment;color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;Layout.fillWidth:true}
    }
   }
  }
  ColumnLayout {Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
   Rectangle{Layout.fillWidth:true;Layout.preferredHeight:Style.space(52);color:"transparent"
    Rectangle{anchors.bottom:parent.bottom;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)}
    RowLayout{anchors.fill:parent;anchors.leftMargin:Style.space(24);anchors.rightMargin:Style.space(20);spacing:Style.space(10)
     Text{text:T3I18n.tr("Preferences");color:root.dim;font.pixelSize:Style.font.caption}
     Text{text:"/";color:root.dim;font.pixelSize:Style.font.caption}
     Text{text:T3I18n.tr(root.currentPage.name);color:root.fg;font.pixelSize:Style.font.caption;Layout.fillWidth:true;elide:Text.ElideRight}
     T3Select{objectName:"preferencesEnvironment";visible:root.page!=="licenses";enabled:!root.result.busy;Layout.preferredWidth:Style.space(root.compact?135:170);model:(root.supportsScope?[{id:"*",name:T3I18n.tr("All environments")}]:[]).concat(root.environments);textRole:"name";popupAbove:false;currentIndex:Math.max(0,model.findIndex(function(e){return e.id===root.environment}));onActivated:function(i){root.selectEnvironment(model[i].id)}}
     T3IconButton{name:"refresh-cw";enabled:!root.result.busy;tooltipText:T3I18n.tr("Refresh");onClicked:root.load()}
    }
   }
   Controls.ScrollView {id:settingsScroll;objectName:"preferencesScroll";Layout.fillWidth:true;Layout.fillHeight:true;contentWidth:availableWidth;clip:true
    T3Wheel{flickable:settingsScroll.contentItem}
    Item {width:settingsScroll.availableWidth;implicitHeight:content.implicitHeight+Style.space(64)
     ColumnLayout {id:content;objectName:"preferencesContent";anchors.top:parent.top;anchors.topMargin:Style.space(24);anchors.horizontalCenter:parent.horizontalCenter;width:Math.min(Style.space(848),parent.width-Style.space(root.compact?32:48));spacing:Style.space(24)
      ColumnLayout{Layout.fillWidth:true;visible:root.supportsScope&&!root.query.trim();spacing:Style.space(10)
       Text{objectName:"preferencesScope";Layout.fillWidth:true;text:root.page==="archived"?T3I18n.tr("Archive"):root.projectScope?T3I18n.tr("Settings for {project}",{project:root.selectedGroup?root.selectedGroup.title:T3I18n.tr("Unavailable selection")}):root.environment==="*"?T3I18n.tr("Applying settings for all projects on all environments"):T3I18n.tr("Applying settings for all projects on {environment}",{environment:(root.environments.find(function(e){return e.id===root.environment}) || {}).name || root.environment});color:root.dim;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
       RowLayout{Layout.fillWidth:true
        T3Select{objectName:"preferencesProjectScope";Layout.fillWidth:true;enabled:!root.result.busy;model:[{key:"",title:T3I18n.tr("All projects")}].concat(root.projectGroups);textRole:"title";popupAbove:false;currentIndex:model.findIndex(function(p){return p.key===root.projectKey});onActivated:function(i){root.selectProject(model[i].key)}}
        T3Select{objectName:"preferencesCheckoutScope";visible:root.projectScope;Layout.fillWidth:true;enabled:!root.result.busy;model:[{key:"",name:T3I18n.tr("All checkouts")}].concat(root.matchingMembers.map(function(p){return {key:p.key,name:((root.environments.find(function(e){return e.id===p.environmentId}) || {}).name || p.environmentId)+" · "+p.workspaceRoot}}));textRole:"name";popupAbove:false;currentIndex:model.findIndex(function(p){return p.key===root.checkoutKey});onActivated:function(i){root.checkoutKey=model[i].key}}
       }
      }
      Text{visible:!!root.query.trim();Layout.fillWidth:true;text:T3I18n.tr("Search settings");color:root.fg;font.pixelSize:Style.font.body}
      Text{visible:["archived","shortcuts","licenses"].indexOf(root.page)<0&&!!root.result.error;Layout.fillWidth:true;text:T3I18n.error(root.result.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      Text{visible:["archived","shortcuts","licenses"].indexOf(root.page)<0&&(root.result.errors || []).length>0&&(root.result.savedTargets || []).length>0;Layout.fillWidth:true;text:T3I18n.tr("Saved on {count} targets",{count:(root.result.savedTargets || []).length});color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
      Repeater{model:["archived","shortcuts","licenses"].indexOf(root.page)>=0?[]:root.result.errors || []
       Text{required property var modelData;Layout.fillWidth:true;text:(modelData.target.environment || "")+": "+T3I18n.error(modelData.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
      }
      WorkIndicator{visible:["archived","shortcuts","licenses"].indexOf(root.page)<0&&root.result.busy;active:visible}
      Repeater {model:root.groups
       T3PreferenceSection{required property var modelData;Layout.fillWidth:true;title:modelData.title;icon:modelData.icon
        body:Component{Column{width:parent.width;Repeater{model:modelData.fields
         T3PreferenceField{required property var modelData;required property int index;width:parent.width;field:modelData;divided:index>0;writable:root.writable&&modelData.scopeWritable!==false;backend:root.bridge;environment:root.representativeEnvironment;modelCatalog:root.result.modelCatalog || (root.bridge.catalogs || {})[root.representativeEnvironment] || ({});edit:root.edits[modelData.key];onStaged:function(value){root.stage(modelData.key,value)};onChanged:function(value){root.save(modelData.key,value)};onInheritRequested:root.clearOverride(modelData.key)}
        }}}
       }
      }
      Text{visible:!!root.query.trim()&&!root.groups.length&&!root.licenseSearchMatch&&!root.sectionSearchMatch;text:T3I18n.tr("No settings found");color:root.dim;font.pixelSize:Style.font.body;Layout.fillWidth:true;horizontalAlignment:Text.AlignHCenter;Layout.topMargin:Style.space(32)}
      T3PreferenceSection{visible:root.query.trim()?root.licenseSearchMatch:root.page==="general";Layout.fillWidth:true;title:"Diagnostics";icon:"info"
       body:Component{RowLayout{width:parent.width;spacing:Style.space(14)
        ColumnLayout{Layout.fillWidth:true;Layout.margins:Style.space(14);spacing:Style.space(5)
         Text{text:T3I18n.tr("Open-source licenses");color:root.fg;font.pixelSize:Style.font.caption}
         Text{Layout.fillWidth:true;text:T3I18n.tr("Notices for dependencies, assets, and tools used by ot3.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
        }
        T3PickerButton{objectName:"preferencesOpenLicenses";Layout.rightMargin:Style.space(14);text:T3I18n.tr("View licenses");primary:false;onClicked:{root.query="";root.page="licenses"}}
       }}
      }
      T3PreferenceSection{visible:root.page==="general"&&(!root.query.trim()||root.matches("Plan mode (legacy)")||root.matches("Legacy features"));Layout.fillWidth:true;title:"Legacy features";icon:"settings"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;field:({key:"planModeEnabled",label:"Plan mode (legacy)",description:"Restore Build/Plan, /plan, /default, and Shift+Tab. Off uses build mode.",kind:"bool",value:!!((root.bridge.clientSettings || {}).data || {}).planModeEnabled});writable:!!((root.bridge.clientSettings || {}).data || {}).revision&&!((root.bridge.clientSettings || {}).busy);onChanged:function(value){root.bridge.requestClientSettings({planModeEnabled:value})}}
        Text{width:parent.width;visible:!!(root.bridge.clientSettings || {}).error;text:T3I18n.error((root.bridge.clientSettings || {}).error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
       }}
      }
      // Check the OTLP endpoints the server would export to (T3 telemetry export).
      T3PreferenceSection{objectName:"otlpChecks";visible:root.page==="general"&&!root.query.trim()&&(root.result.fields||[]).some(function(f){return String(f.key).indexOf("observability.")===0&&!!f.value});Layout.fillWidth:true;title:"Check telemetry endpoints";icon:"server"
       body:Component{Column{width:parent.width;spacing:Style.space(6)
        Repeater{model:(root.result.fields||[]).filter(function(f){return String(f.key).indexOf("observability.")===0&&!!f.value})
         RowLayout{required property var modelData;width:parent.width;spacing:Style.space(8)
          readonly property string signal:modelData.key==="observability.otlpTracesUrl"?"traces":modelData.key==="observability.otlpMetricsUrl"?"metrics":"logs"
          readonly property var check:(root.bridge.otlpChecks||{})[signal]||({})
          Text{Layout.fillWidth:true;text:T3I18n.tr(modelData.label)+" · "+(check.busy?T3I18n.tr("Checking…"):check.error?T3I18n.error(check.error):check.result?(check.result._tag==="Accepted"?T3I18n.tr("Accepted in {ms} ms",{ms:Math.round(check.result.latencyMs)}):check.result._tag==="Rejected"?T3I18n.tr("Rejected with HTTP {status}",{status:check.result.status}):T3I18n.tr(check.result.timedOut?"Timed out":"Unreachable")):modelData.value);color:check.result&&check.result._tag!=="Accepted"?Color.urgent:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
          T3PickerButton{primary:false;implicitHeight:Style.space(26);text:T3I18n.tr("Check");enabled:!check.busy;onClicked:root.bridge.checkOtlp(root.environment,parent.signal,modelData.value)}
         }
        }
       }}
      }
      // The T3 Code revision ot3 is validated with, and each connected server's version.
      T3PreferenceSection{objectName:"preferencesT3Compatibility";visible:root.page==="general"&&(!root.query.trim()||root.matches("T3 Code compatibility")||root.matches("T3 server updates"));Layout.fillWidth:true;title:"T3 Code compatibility";icon:"server"
       body:Component{Column{width:parent.width;spacing:Style.space(6)
        Text{width:parent.width;text:T3I18n.tr("ot3 is validated with T3 Code {version} ({date}).",{version:(root.bridge.validatedT3 || {}).tag || "—",date:(root.bridge.validatedT3 || {}).date || "—"});color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
        // T3 server updates: the native `t3 update`, run by ot3 when the server is idle.
        T3PreferenceField{width:parent.width;controlName:"preferenceT3ServerUpdates";writable:!!root.bridge.setServerUpdateMode
         field:({key:"t3ServerUpdates",label:"T3 server updates",description:"Keep each connected T3 server on the newest release of its channel. Automatic updates wait until no agent has worked on that server for ten minutes, because the update restarts it.",kind:"choice",value:root.bridge.serverUpdateMode || "auto",choices:[{id:"auto",name:"Automatically when idle"},{id:"notify",name:"Only show available updates"},{id:"off",name:"Off"}]})
         onChanged:function(value){root.bridge.setServerUpdateMode(value)}}
        Repeater{model:(root.bridge.state.environments || []).filter(function(e){return e.state==="ready"&&!!root.bridge.serverVersion(e.id)})
         Column{required property var modelData;width:parent.width;spacing:Style.space(2)
          readonly property bool newer:root.bridge.t3VersionNewer(root.bridge.serverVersion(modelData.id))
          readonly property var update:root.bridge.serverUpdate?root.bridge.serverUpdate(modelData.id):null
          readonly property string failure:(root.bridge.serverUpdateErrors || {})[modelData.id] || (update&&update.state==="failed"?update.error || "":"")
          Text{objectName:"t3CompatibilityRow_"+modelData.id;width:parent.width
           text:(modelData.name || modelData.id)+": T3 "+root.bridge.serverVersion(modelData.id)+(parent.newer?" · "+T3I18n.tr("newer: features added since then may be missing or behave differently."):" · "+T3I18n.tr("validated"))
           color:parent.newer?T3Palette.waiting:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
          RowLayout{width:parent.width;spacing:Style.space(8);visible:!!parent.update&&(!!parent.update.latest||parent.update.state==="running"||!!parent.failure)
           Text{objectName:"t3ServerUpdateStatus_"+modelData.id;Layout.fillWidth:true;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText
            readonly property var update:parent.parent.update || ({})
            readonly property string failure:parent.parent.failure
            text:update.state==="running"?T3I18n.tr("Updating to T3 {version}… The server restarts when it finishes.",{version:update.latest || ""})
             :failure?T3I18n.tr("The update failed: {reason}",{reason:root.serverUpdateReason(failure)})
             :!update.canUpdate?T3I18n.tr("T3 {version} is available. ot3 cannot update it: {reason}",{version:update.latest,reason:root.serverUpdateReason(update.reason)})
             :root.bridge.serverUpdateMode==="auto"?T3I18n.tr("T3 {version} is available. It installs once the server has been idle for ten minutes.",{version:update.latest})
             :T3I18n.tr("T3 {version} is available.",{version:update.latest})
            color:failure?Color.urgent:update.state==="running"?T3Palette.working:root.fg}
           T3PickerButton{objectName:"t3ServerUpdateNow_"+modelData.id;primary:true;implicitHeight:Style.space(26);text:T3I18n.tr("Update now")
            readonly property var update:parent.parent.update || ({})
            visible:!!update.latest&&!!update.canUpdate&&update.state!=="running";onClicked:root.bridge.updateServer(modelData.id)}
          }
         }
        }
       }}
      }
      // T3 followUpBehavior, shared with T3 through its client settings.
      T3PreferenceSection{visible:root.page==="general"&&(!root.query.trim()||root.matches("Follow-up behavior")||root.matches("Steer"));Layout.fillWidth:true;title:"Follow-up behavior";icon:"corner-up-right"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;controlName:"preferenceFollowUpBehavior";field:({key:"followUpBehavior",label:"Follow-up behavior",description:"Queue follow-ups while the agent runs or steer the current run. Ctrl+Enter, or Ctrl over the send button, does the opposite for one message.",kind:"choice",value:root.bridge.followUpBehavior,choices:[{id:"queue",name:"Queue"},{id:"steer",name:"Steer"}]});writable:!!((root.bridge.clientSettings || {}).data || {}).revision&&!((root.bridge.clientSettings || {}).busy);onChanged:function(value){root.bridge.requestClientSettings({followUpBehavior:value})}}
       }}
      }
      T3PreferenceSection{visible:root.page==="general"&&(!root.query.trim()||root.matches("Notifications")||root.matches("Thread notifications"));Layout.fillWidth:true;title:"Notifications";icon:"bell"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;controlName:"preferenceThreadNotifications";field:({key:"threadNotifications",label:"Thread notifications",description:"How you hear about threads that finish or need input while the panel is closed. Threads finishing together play one sound.",kind:"choice",value:root.bridge.viewState.threadNotifications || "notifications",choices:[{id:"notifications",name:"Notifications only"},{id:"notifications-and-sound",name:"Notifications with sound"},{id:"sound",name:"Sound only"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadNotifications:value})}}
       }}
      }
      T3PreferenceSection{visible:root.page==="general"&&(!root.query.trim()||root.matches("Screenshots")||root.matches("Screenshot button")||root.matches("Screenshot command"));Layout.fillWidth:true;title:"Screenshots";icon:"camera"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;controlName:"preferenceScreenshotButton";field:({key:"screenshotButton",label:"Screenshot button",description:"Show a button next to Attach that captures the screen and attaches the image.",kind:"bool",value:root.bridge.viewState.screenshotButton!==false});writable:true;onChanged:function(value){root.bridge.rememberView({screenshotButton:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceScreenshotHide";field:({key:"screenshotHidePanel",label:"Hide the panel while capturing",kind:"bool",value:root.bridge.viewState.screenshotHidePanel!==false});writable:true;onChanged:function(value){root.bridge.rememberView({screenshotHidePanel:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceHtmlRenders";field:({key:"htmlRenders",label:"HTML renders",description:"Where pages made by the agent appear. The side panel needs a wide window; narrow windows keep them in the conversation.",kind:"choice",value:root.bridge.viewState.htmlRenders || "inline",choices:[{id:"inline",name:"In the conversation"},{id:"side",name:"Side panel"}]});writable:true;onChanged:function(value){root.bridge.rememberView({htmlRenders:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"screenshotCommand",label:"Screenshot command",description:"Empty uses Omasnap, then Omarchy's screenshot tool, then grim and slurp. A command may write to {file}, print an image path, or copy a PNG to the clipboard.",kind:"text",commitOnBlur:true,maximumLength:4096,placeholder:"Automatic",value:root.bridge.viewState.screenshotCommand || ""});writable:true;onChanged:function(value){root.bridge.rememberView({screenshotCommand:value.trim()})}}
       }}
      }
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="appearance";Layout.fillWidth:true;title:"Omarchy theme";icon:"layers"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;field:({key:"language",label:"Language",kind:"choice",value:T3I18n.choice,choices:T3I18n.languages.map(function(l){return {id:l.id,name:l.name}})});writable:true;onChanged:function(value){root.bridge.rememberView({language:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceUsage";field:({key:"usage",label:"Show provider usage",kind:"bool",value:!!root.bridge.viewState.usageExpanded});writable:true;onChanged:function(value){root.usageRequested(value)}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceGrouping";field:({key:"grouping",label:"Group working, snoozed and settled threads",kind:"bool",value:!!root.bridge.viewState.sidebarCompact});writable:true;onChanged:function(value){root.groupingRequested(value)}}
       }}
      }
      T3PreferenceSection{visible:root.page==="appearance"&&(!root.query.trim()||root.matches("Math rendering")||root.matches("Messages"));Layout.fillWidth:true;title:"Messages";icon:"message-square"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;controlName:"preferenceMathRendering";field:({key:"mathRenderingEnabled",label:"Math rendering",description:"Typeset LaTeX math, such as $x^2$ or \\[x^2\\], in messages and Markdown previews. The composer keeps TeX source editable.",kind:"bool",value:!!((root.bridge.clientSettings || {}).data || {}).mathRenderingEnabled});writable:!!((root.bridge.clientSettings || {}).data || {}).revision&&!((root.bridge.clientSettings || {}).busy);onChanged:function(value){root.bridge.requestClientSettings({mathRenderingEnabled:value})}}
       }}
      }
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="appearance";Layout.fillWidth:true;title:"Animations";icon:"star"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;controlName:"preferenceStarfield";field:({key:"starfieldMode",label:"Animated starfield",description:"Auto lowers the frame rate on slower machines and stays still with software rendering.",kind:"choice",value:root.bridge.viewState.starfieldMode || "auto",choices:[{id:"auto",name:"Auto"},{id:"full",name:"Full"},{id:"light",name:"Light"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({starfieldMode:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"starfieldSpeed",label:"Star speed",kind:"choice",value:root.bridge.viewState.starfieldSpeed || "normal",choices:[{id:"slow",name:"Slow"},{id:"normal",name:"Normal"},{id:"fast",name:"Fast"}]});writable:true;onChanged:function(value){root.bridge.rememberView({starfieldSpeed:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"starfieldMeteors",label:"Shooting stars",description:"They cross the sky while a thread is working.",kind:"choice",value:root.bridge.viewState.starfieldMeteors || "normal",choices:[{id:"none",name:"None"},{id:"few",name:"Few"},{id:"normal",name:"Normal"},{id:"many",name:"Many"}]});writable:true;onChanged:function(value){root.bridge.rememberView({starfieldMeteors:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadWorkingEffect",label:"Working threads",description:"Thread effects stay still while the starfield is off.",kind:"choice",value:root.bridge.viewState.threadWorkingEffect || "comet",choices:[{id:"comet",name:"Comet"},{id:"pulse",name:"Pulse"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadWorkingEffect:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadInputEffect",label:"Threads needing input",kind:"choice",value:root.bridge.viewState.threadInputEffect || "glints",choices:[{id:"glints",name:"Glints"},{id:"pulse",name:"Pulse"},{id:"static",name:"Static"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadInputEffect:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadChangeEffect",label:"Thread state changes",kind:"choice",value:root.bridge.viewState.threadChangeEffect || "star",choices:[{id:"star",name:"Shooting star"},{id:"flash",name:"Flash"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadChangeEffect:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadMoveEffect",label:"Threads changing section",description:"How a thread travels when it moves to another sidebar section.",kind:"choice",value:root.bridge.viewState.threadMoveEffect || "arc",choices:[{id:"arc",name:"Swoop"},{id:"glide",name:"Glide"},{id:"fade",name:"Fade"},{id:"off",name:"Off"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadMoveEffect:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadMoveSpeed",label:"Section change speed",kind:"choice",value:root.bridge.viewState.threadMoveSpeed || "normal",choices:[{id:"slow",name:"Slow"},{id:"normal",name:"Normal"},{id:"fast",name:"Fast"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadMoveSpeed:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"threadEffectIntensity",label:"Effect intensity",kind:"choice",value:root.bridge.viewState.threadEffectIntensity || "normal",choices:[{id:"subtle",name:"Subtle"},{id:"normal",name:"Normal"},{id:"vivid",name:"Vivid"}]});writable:true;onChanged:function(value){root.bridge.rememberView({threadEffectIntensity:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceAnimationRate";field:({key:"animationRate",label:"Animation smoothness",description:"Adaptive runs each animation at the rate it needs: the drifting sky slowly, spinners and comets at 60 fps, shooting stars at the display rate. Every frame is recomposed by the compositor, so higher rates use more CPU and battery.",kind:"choice",value:root.bridge.viewState.animationRate || "adaptive",choices:[{id:"adaptive",name:"Adaptive"},{id:"saver",name:"Saver (30 fps)"},{id:"max",name:"Maximum (display rate)"}]});writable:true;onChanged:function(value){root.bridge.rememberView({animationRate:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceBarIndicator";field:({key:"barIndicator",label:"Bar indicator",description:"How the T3 mark in the bar shows work, pending questions and errors.",kind:"choice",value:root.bridge.viewState.barIndicator || "orbit",choices:[{id:"orbit",name:"Orbit"},{id:"scanner",name:"Scanner"},{id:"equalizer",name:"Equalizer"},{id:"heartbeat",name:"Heartbeat"},{id:"ticks",name:"Ticks"},{id:"logo",name:"Living logo"}]});writable:true;onChanged:function(value){root.bridge.rememberView({barIndicator:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceFpsMeter";field:({key:"fpsMeter",label:"Frame rate meter",description:"Show frames per second in the panel corner to spot stutter.",kind:"bool",value:root.bridge.viewState.fpsMeter===true});writable:true;onChanged:function(value){root.bridge.rememberView({fpsMeter:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"transitionStyle",label:"Show and hide",description:"How the sidebar, usage, search and lists appear and disappear.",kind:"choice",value:root.bridge.viewState.transitionStyle || "nebula",choices:[{id:"nebula",name:"Nebula"},{id:"warp",name:"Hyperspace"},{id:"fold",name:"Fold"},{id:"spring",name:"Spring"},{id:"instant",name:"Instant"}]});writable:true;onChanged:function(value){root.bridge.rememberView({transitionStyle:value})}}
        T3PreferenceField{width:parent.width;divided:true;field:({key:"transitionSpeed",label:"Transition speed",kind:"choice",value:root.bridge.viewState.transitionSpeed || "normal",choices:[{id:"slow",name:"Slow"},{id:"normal",name:"Normal"},{id:"fast",name:"Fast"}]});writable:true;onChanged:function(value){root.bridge.rememberView({transitionSpeed:value})}}
       }}
      }
      Loader{Layout.fillWidth:true;active:root.page==="storage";visible:active;sourceComponent:Component{T3StorageSettings{bridge:root.bridge;settings:root.result;projectScope:root.projectScope;writable:root.writable;query:root.query;drafts:root.edits;onSettingChanged:function(key,value){root.save(key,value)};onStaged:function(key,value){root.stage(key,value)}}}}
      Loader{Layout.fillWidth:true;active:!root.query.trim()&&root.page==="integrations";visible:active;sourceComponent:Component{T3Integrations{bridge:root.bridge;targets:root.scope.targets;settings:root.result;settingsWritable:root.writable;onSettingChanged:function(key,value){root.save(key,value)};onInheritRequested:function(key){root.clearOverride(key)}}}}
      Loader{Layout.fillWidth:true;active:root.page==="memory";visible:active;sourceComponent:Component{T3JournalSettings{bridge:root.bridge;query:root.query}}}
      Loader{Layout.fillWidth:true;active:!root.query.trim()&&root.page==="licenses";visible:active;sourceComponent:Component{T3Licenses{bridge:root.bridge}}}
      Loader{Layout.fillWidth:true;Layout.preferredHeight:Math.max(Style.space(280),settingsScroll.height-Style.space(170));active:!root.query.trim()&&root.page==="archived";visible:active;sourceComponent:Component{T3Archive{bridge:root.bridge;targets:root.scope.targets}}}
      Loader{Layout.fillWidth:true;active:!root.query.trim()&&root.page==="connections";visible:active;sourceComponent:Component{T3Connections{bridge:root.bridge;environment:root.environment}}}
      Loader{Layout.fillWidth:true;active:!root.query.trim()&&root.page==="providers";visible:active;sourceComponent:Component{T3ProviderInstances{bridge:root.bridge;environment:root.environment}}}
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="providers";Layout.fillWidth:true;title:"Model options";icon:"star"
       body:Component{Column{width:parent.width;Repeater{model:(root.bridge.catalogs[root.environment] || {}).models || []
        Item{required property var modelData;required property int index;width:parent.width;implicitHeight:Style.space(52)
         Rectangle{visible:index>0;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.065)}
         RowLayout{anchors.fill:parent;anchors.margins:Style.space(12)
          Text{Layout.fillWidth:true;text:modelData.displayName || modelData.model;color:root.fg;elide:Text.ElideRight;font.pixelSize:Style.font.caption}
          T3IconButton{readonly property bool favorite:((root.bridge.catalogs[root.environment] || {}).favorites || []).some(function(f){return f.provider===modelData.instanceId&&f.model===modelData.model});name:favorite?"star-filled":"star";tint:favorite?Color.accent:root.dim;tooltipText:T3I18n.tr("Model options");onClicked:root.bridge.toggleModelFavoriteIn(root.environment,modelData.instanceId,modelData.model)}
         }
        }
       }}}
      }
      Loader{Layout.fillWidth:true;active:!root.query.trim()&&root.page==="shortcuts";visible:active;sourceComponent:Component{T3Keybindings{bridge:root.bridge;targets:root.scope.targets}}}
     }
    }
   }
  }
 }
}
