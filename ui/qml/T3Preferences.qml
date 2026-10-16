import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var bridge
 property string shortcuts:""
 signal closed()
 signal usageRequested(bool value)
 signal groupingRequested(bool value)
 property string page:"general"
 property var edits:({})
 property string savingKey:""
 property string savingRequest:""
 function save(key,value){if(!writable)return;bridge.requestSettings(environment,key,value);if(result.busy){savingKey=key;savingRequest=result.requestId}}
 Connections {target:root.bridge
  function onSettingsResultChanged(){var r=root.result;if(r.busy||!root.savingRequest||r.requestId!==root.savingRequest)return;if(!r.error){var next=Object.assign({},root.edits);delete next[root.savingKey];root.edits=next}root.savingKey="";root.savingRequest=""}
 }
 function stage(key,value){var next=Object.assign({},edits);next[key]=value;edits=next}
 property string environment:bridge.selectedEnvironment || "local"
 readonly property var result:bridge.settingsResult
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property bool writable:result.environment===environment&&!result.busy&&!result.error
 color:Color.popups.background
 function load(){bridge.requestSettings(environment)}
 Component.onCompleted:load()
 onEnvironmentChanged:{edits={};savingKey="";savingRequest="";load()}

 property string query:""
 readonly property bool compact:width<Style.space(780)
 readonly property var pages:[{id:"general",name:"General",icon:"settings"},{id:"appearance",name:"Appearance",icon:"layers"},{id:"connections",name:"Connections",icon:"server"},{id:"providers",name:"Providers",icon:"acp"},{id:"source-control",name:"Source control",icon:"git-branch"},{id:"shortcuts",name:"Keyboard shortcuts",icon:"keyboard"}]
 readonly property var currentPage:pages.find(function(p){return p.id===root.page}) || pages[0]
 readonly property var sections:[
  {page:"general",title:"Updates",icon:"refresh-cw",keys:["enableProviderUpdateChecks","continueThreadsAfterServerUpdate"]},
  {page:"general",title:"Threads",icon:"list-todo",keys:["sidebarAutoSettleAfterDays","sidebarAutoSettleKeepPullRequests","sidebarAutoSettleOnMerge","snoozeLimitedThreads","autoResumeLimitedThreads"]},
  {page:"general",title:"Background activity",icon:"clock",keys:["responseStreamingMode","backgroundActivityProfile"]},
  {page:"source-control",title:"Branch naming",icon:"git-branch",keys:["branchNamingMode","branchNamePrefix","branchNameInstructions"]},
  {page:"source-control",title:"Worktrees",icon:"folder-git-2",keys:["defaultAutoPull","newWorktreesStartFromOrigin","worktreeSubmodules"]},
  {page:"source-control",title:"Writing style",icon:"square-pen",keys:["sourceControlWritingStyle.mode","sourceControlWritingStyle.customInstructions","sourceControlWritingStyle.followChangeRequestTemplates"]},
  {page:"source-control",title:"Pull requests",icon:"git-pull-request",keys:["pullRequestMergeMethod","removeAgentCreditsOnMerge","reviewGuidesEnabled","reviewGuideInstructions"]}]
 function matches(text){return !query.trim() || (T3I18n.tr(text).toLocaleLowerCase().indexOf(query.trim().toLocaleLowerCase())>=0 || String(text).toLocaleLowerCase().indexOf(query.trim().toLocaleLowerCase())>=0)}
 readonly property var groups:sections.filter(function(s){return root.query.trim() || s.page===root.page}).map(function(s){return Object.assign({},s,{fields:(root.result.fields || []).filter(function(f){return s.keys.indexOf(f.key)>=0&&(root.matches(s.title)||root.matches(f.label))})})}).filter(function(s){return s.fields.length>0})
 onQueryChanged:if(settingsScroll.contentItem)settingsScroll.contentItem.contentY=0
 onPageChanged:if(settingsScroll.contentItem)settingsScroll.contentItem.contentY=0
 RowLayout {
  anchors.fill:parent;spacing:0
  Rectangle {objectName:"preferencesNavigation";Layout.preferredWidth:Style.space(root.compact?185:220);Layout.fillHeight:true;color:Qt.darker(Color.popups.background,1.07)
   Rectangle{anchors.right:parent.right;width:1;height:parent.height;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)}
   ColumnLayout {anchors.fill:parent;anchors.margins:Style.space(12);spacing:Style.space(6)
    RowLayout{Layout.fillWidth:true;Layout.preferredHeight:Style.space(45);Layout.leftMargin:Style.space(8);spacing:Style.space(10)
     T3Icon{name:"settings";tint:root.dim;implicitWidth:Style.space(18)}
     Text{text:T3I18n.tr("Preferences");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold;Layout.fillWidth:true}
    }
    Controls.AbstractButton {id:back;objectName:"preferencesBack";Layout.fillWidth:true;implicitHeight:Style.space(34);onClicked:root.closed();hoverEnabled:true
     background:Rectangle{radius:Style.space(6);color:back.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent";border.width:back.activeFocus?1:0;border.color:Color.accent}
     contentItem:RowLayout{spacing:Style.space(8);T3Icon{name:"chevron-left";tint:root.dim;implicitWidth:Style.space(14)}Text{text:T3I18n.tr("Back to chat");font.pixelSize:Style.font.caption;color:root.dim;Layout.fillWidth:true}}
     leftPadding:Style.space(10);rightPadding:Style.space(10);HoverHandler{cursorShape:Qt.PointingHandCursor}
    }
    Item{Layout.preferredHeight:Style.space(12)}
    Repeater {model:root.pages
     Controls.AbstractButton {id:nav;required property var modelData;objectName:"preferencesTab_"+modelData.id;Layout.fillWidth:true;implicitHeight:Style.space(36);hoverEnabled:true;checked:root.page===modelData.id&&!root.query.trim();Accessible.name:T3I18n.tr(modelData.name);onClicked:{root.query="";root.page=modelData.id}
      leftPadding:Style.space(10);rightPadding:Style.space(8)
      background:Rectangle{radius:Style.space(6);color:nav.checked?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08):nav.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent";border.width:nav.activeFocus?1:0;border.color:Color.accent}
      contentItem:RowLayout{spacing:Style.space(10);T3Icon{name:nav.modelData.icon;tint:nav.checked?root.fg:root.dim;implicitWidth:Style.space(16)}Text{text:T3I18n.tr(nav.modelData.name);color:nav.checked?root.fg:root.dim;font.pixelSize:Style.font.caption;font.weight:nav.checked?Font.Medium:Font.Normal;Layout.fillWidth:true;elide:Text.ElideRight}}
      HoverHandler{cursorShape:Qt.PointingHandCursor}
     }
    }
    Item{Layout.fillHeight:true}
    RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(8);spacing:Style.space(8)
     T3Icon{name:root.environment==="local"?"laptop":"server";tint:Color.accent;implicitWidth:Style.space(14)}
     Text{text:(root.environments.find(function(e){return e.id===root.environment}) || {}).name || root.environment;color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;Layout.fillWidth:true}
    }
   }
  }
  ColumnLayout {Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
   Rectangle{Layout.fillWidth:true;Layout.preferredHeight:Style.space(58);color:"transparent"
    Rectangle{anchors.bottom:parent.bottom;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07)}
    RowLayout{anchors.fill:parent;anchors.leftMargin:Style.space(24);anchors.rightMargin:Style.space(20);spacing:Style.space(10)
     Text{text:T3I18n.tr("Preferences");color:root.dim;font.pixelSize:Style.font.caption}
     Text{text:"/";color:root.dim;font.pixelSize:Style.font.caption}
     Text{text:T3I18n.tr(root.currentPage.name);color:root.fg;font.pixelSize:Style.font.caption;Layout.fillWidth:true;elide:Text.ElideRight}
     T3Select{objectName:"preferencesEnvironment";enabled:!root.result.busy;Layout.preferredWidth:Style.space(root.compact?135:170);model:root.environments;textRole:"name";popupAbove:false;currentIndex:Math.max(0,model.findIndex(function(e){return e.id===root.environment}));onActivated:function(i){root.environment=model[i].id}}
     T3IconButton{name:"refresh-cw";enabled:!root.result.busy;tooltipText:T3I18n.tr("Refresh");onClicked:root.load()}
    }
   }
   Controls.ScrollView {id:settingsScroll;objectName:"preferencesScroll";Layout.fillWidth:true;Layout.fillHeight:true;contentWidth:availableWidth;clip:true
    T3Wheel{flickable:settingsScroll.contentItem}
    Item {width:settingsScroll.availableWidth;implicitHeight:content.implicitHeight+Style.space(64)
     ColumnLayout {id:content;objectName:"preferencesContent";anchors.top:parent.top;anchors.topMargin:Style.space(30);anchors.horizontalCenter:parent.horizontalCenter;width:Math.min(Style.space(820),parent.width-Style.space(root.compact?32:64));spacing:Style.space(24)
      RowLayout{Layout.fillWidth:true;spacing:Style.space(16)
       Text{text:T3I18n.tr(root.query.trim()?"Search settings":root.currentPage.name);color:root.fg;font.pixelSize:Style.space(21);font.weight:Font.DemiBold;Layout.fillWidth:true;elide:Text.ElideRight}
       Controls.TextField{id:search;objectName:"preferencesSearch";Layout.preferredWidth:Math.min(Style.space(210),content.width*0.42);implicitHeight:Style.space(32);text:root.query;onTextEdited:root.query=text;placeholderText:T3I18n.tr("Search settings");placeholderTextColor:root.dim;color:root.fg;font.pixelSize:Style.font.caption;leftPadding:Style.space(30);rightPadding:Style.space(8)
        background:Rectangle{radius:Style.space(7);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:search.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
        T3Icon{anchors.left:parent.left;anchors.leftMargin:Style.space(9);anchors.verticalCenter:parent.verticalCenter;name:"search";tint:root.dim;implicitWidth:Style.space(14)}
       }
      }
      Text{visible:!!root.result.error;Layout.fillWidth:true;text:T3I18n.error(root.result.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      WorkIndicator{visible:root.result.busy;active:visible}
      Repeater {model:root.groups
       T3PreferenceSection{required property var modelData;Layout.fillWidth:true;title:modelData.title;icon:modelData.icon
        body:Component{Column{width:parent.width;Repeater{model:modelData.fields
         T3PreferenceField{required property var modelData;required property int index;width:parent.width;field:modelData;divided:index>0;writable:root.writable;edit:root.edits[modelData.key];onStaged:function(value){root.stage(modelData.key,value)};onChanged:function(value){root.save(modelData.key,value)}}
        }}}
       }
      }
      Text{visible:!!root.query.trim()&&!root.groups.length;text:T3I18n.tr("No settings found");color:root.dim;font.pixelSize:Style.font.body;Layout.fillWidth:true;horizontalAlignment:Text.AlignHCenter;Layout.topMargin:Style.space(32)}
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="appearance";Layout.fillWidth:true;title:"Omarchy theme";icon:"layers"
       body:Component{Column{width:parent.width
        T3PreferenceField{width:parent.width;field:({key:"language",label:"Language",kind:"choice",value:T3I18n.choice,choices:T3I18n.languages.map(function(l){return {id:l.id,name:l.name}})});writable:true;onChanged:function(value){root.bridge.rememberView({language:value})}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceUsage";field:({key:"usage",label:"Show provider usage",kind:"bool",value:!!root.bridge.viewState.usageExpanded});writable:true;onChanged:function(value){root.usageRequested(value)}}
        T3PreferenceField{width:parent.width;divided:true;controlName:"preferenceGrouping";field:({key:"grouping",label:"Group working, snoozed and settled threads",kind:"bool",value:!!root.bridge.viewState.sidebarCompact});writable:true;onChanged:function(value){root.groupingRequested(value)}}
       }}
      }
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="connections";Layout.fillWidth:true;title:"Connections";icon:"server"
       body:Component{Column{width:parent.width;Repeater{model:root.bridge.state.environments || []
        Item{required property var modelData;required property int index;width:parent.width;implicitHeight:Style.space(68)
         Rectangle{visible:index>0;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.065)}
         RowLayout{anchors.fill:parent;anchors.margins:Style.space(16);spacing:Style.space(12)
          T3Icon{name:modelData.id==="local"?"laptop":"server";tint:root.dim;implicitWidth:Style.space(18)}
          Text{Layout.fillWidth:true;text:modelData.name;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight}
          Rectangle{width:Style.space(6);height:width;radius:width/2;color:modelData.state==="ready"?T3Palette.done:Color.urgent}
          Text{text:modelData.state==="ready"?T3I18n.tr("Done"):T3I18n.error(modelData.error || modelData.state);color:root.dim;font.pixelSize:Style.font.caption;Layout.maximumWidth:parent.width*0.45;elide:Text.ElideRight}
         }
        }
       }}}
      }
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="providers";Layout.fillWidth:true;title:"Providers";icon:"acp"
       body:Component{Column{width:parent.width;Repeater{model:(root.bridge.catalogs[root.environment] || {}).providers || []
        Item{required property var modelData;required property int index;width:parent.width;implicitHeight:Style.space(62)
         Rectangle{visible:index>0;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.065)}
         RowLayout{anchors.fill:parent;anchors.margins:Style.space(16);spacing:Style.space(12)
          T3ProviderIcon{provider:modelData;tint:root.fg;implicitWidth:Style.space(19)}
          Text{Layout.fillWidth:true;text:modelData.name || modelData.instanceId;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight}
          Text{text:modelData.ready?T3I18n.tr("Done"):T3I18n.tr("Offline");color:modelData.ready?T3Palette.done:root.dim;font.pixelSize:Style.font.caption}
         }
        }
       }}}
      }
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
      T3PreferenceSection{visible:!root.query.trim()&&root.page==="shortcuts";Layout.fillWidth:true;title:"Keyboard shortcuts";icon:"keyboard"
       body:Component{Column{width:parent.width;Repeater{model:root.shortcuts.split("\n")
        Item{required property string modelData;required property int index;width:parent.width;implicitHeight:shortcutRow.implicitHeight+Style.space(24)
         Rectangle{visible:index>0;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.065)}
         RowLayout{id:shortcutRow;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(12);spacing:Style.space(16)
          Text{Layout.fillWidth:true;text:modelData.split(" · ").slice(1).join(" · ");color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
          Text{text:modelData.split(" · ")[0];color:root.dim;font.pixelSize:Style.font.caption;font.family:"FiraCode Nerd Font";Layout.maximumWidth:parent.width*0.6;wrapMode:Text.Wrap}
         }
        }
       }}}
      }
     }
    }
   }
  }
 }
}
