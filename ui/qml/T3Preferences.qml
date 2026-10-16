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
 property string environment:bridge.selectedEnvironment || "local"
 readonly property var result:bridge.settingsResult
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property bool writable:result.environment===environment&&!result.busy&&!result.error
 color:Color.popups.background
 function load(){bridge.requestSettings(environment)}
 Component.onCompleted:load()
 onEnvironmentChanged:load()
 ColumnLayout {
  anchors.fill:parent;anchors.margins:Style.space(18);spacing:Style.space(16)
  RowLayout {Layout.fillWidth:true
   T3IconButton{name:"chevron-left";tooltipText:T3I18n.tr("Back");onClicked:root.closed()}
   Text{text:T3I18n.tr("Preferences");font.pixelSize:Style.font.body;font.bold:true;color:root.fg;Layout.fillWidth:true}
   T3Select{objectName:"preferencesEnvironment";enabled:!root.result.busy;Layout.preferredWidth:Style.space(160);model:root.environments;textRole:"name";currentIndex:Math.max(0,model.findIndex(function(e){return e.id===root.environment}));onActivated:function(i){root.environment=model[i].id}}
   T3IconButton{name:"refresh-cw";enabled:!root.result.busy;tooltipText:T3I18n.tr("Refresh");onClicked:root.load()}
  }
  RowLayout {Layout.fillWidth:true;Layout.fillHeight:true;spacing:Style.space(20)
   ColumnLayout {Layout.preferredWidth:Style.space(145);Layout.alignment:Qt.AlignTop;spacing:Style.space(4)
    Repeater {model:[{id:"general",name:"General"},{id:"appearance",name:"Appearance"},{id:"connections",name:"Connections"},{id:"providers",name:"Providers"},{id:"shortcuts",name:"Keyboard shortcuts"}]
     Controls.Button{required property var modelData;objectName:"preferencesTab_"+modelData.id;Layout.fillWidth:true;text:T3I18n.tr(modelData.name);checkable:true;checked:root.page===modelData.id;onClicked:root.page=modelData.id;font.pixelSize:Style.font.caption}
    }
    Item{Layout.fillHeight:true}
   }
   Controls.ScrollView {id:settingsScroll;Layout.fillWidth:true;Layout.fillHeight:true;contentWidth:availableWidth
    T3Wheel{flickable:settingsScroll.contentItem}
    ColumnLayout {width:settingsScroll.availableWidth;spacing:Style.space(14)
     Text{visible:!!root.result.error;Layout.fillWidth:true;text:T3I18n.error(root.result.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
     WorkIndicator{visible:root.result.busy;active:visible}
     ColumnLayout {visible:root.page==="general";Layout.fillWidth:true;spacing:Style.space(10)
      Repeater{model:root.result.fields || []
       RowLayout{required property var modelData;Layout.fillWidth:true;spacing:Style.space(12)
        Text{Layout.fillWidth:true;text:T3I18n.tr(modelData.label);color:root.fg;wrapMode:Text.Wrap;font.pixelSize:Style.font.body}
        Controls.CheckBox{objectName:"preference_"+modelData.key;visible:modelData.kind==="bool";enabled:root.writable;checked:modelData.value===true;onToggled:root.bridge.requestSettings(root.environment,modelData.key,checked)}
        T3Select{objectName:"preferenceChoice_"+modelData.key;visible:modelData.kind==="choice";enabled:root.writable;Layout.preferredWidth:Style.space(155);translateLabels:true;model:modelData.choices || [];textRole:"name";currentIndex:model.findIndex(function(c){return c.id===modelData.value});onActivated:function(i){root.bridge.requestSettings(root.environment,modelData.key,model[i].id)}}
       }
      }
     }
     ColumnLayout {visible:root.page==="appearance";Layout.fillWidth:true
      Text{Layout.fillWidth:true;text:T3I18n.tr("Omarchy theme");color:root.fg;font.pixelSize:Style.font.body}
      T3Select{model:T3I18n.languages;textRole:"name";currentIndex:Math.max(0,model.findIndex(function(l){return l.id===T3I18n.choice}));onActivated:function(i){root.bridge.rememberView({language:model[i].id})}}
      Controls.CheckBox{objectName:"preferenceUsage";text:T3I18n.tr("Show provider usage");checked:!!root.bridge.viewState.usageExpanded;onToggled:root.usageRequested(checked)}
      Controls.CheckBox{objectName:"preferenceGrouping";text:T3I18n.tr("Group working, snoozed and settled threads");checked:!!root.bridge.viewState.sidebarCompact;onToggled:root.groupingRequested(checked)}
     }
     ColumnLayout {visible:root.page==="connections";Layout.fillWidth:true
      Repeater{model:root.bridge.state.environments || []
       RowLayout {required property var modelData;Layout.fillWidth:true
        T3Icon{name:modelData.id==="local"?"laptop":"server";tint:modelData.state==="ready"?T3Palette.done:Color.urgent}
        Text{Layout.fillWidth:true;text:modelData.name;color:root.fg;font.pixelSize:Style.font.body}
        Text{text:modelData.state==="ready"?T3I18n.tr("Done"):T3I18n.error(modelData.error || modelData.state);color:root.dim;font.pixelSize:Style.font.caption}
       }
      }
      Controls.Button{text:T3I18n.tr("Refresh");onClicked:root.bridge.refresh()}
     }
     ColumnLayout {visible:root.page==="providers";Layout.fillWidth:true
      Repeater{model:(root.bridge.catalogs[root.environment] || {}).providers || []
       RowLayout {required property var modelData;Layout.fillWidth:true
        T3ProviderIcon{provider:modelData;tint:root.fg}
        Text{Layout.fillWidth:true;text:modelData.name || modelData.instanceId;color:root.fg;font.pixelSize:Style.font.body}
        Text{text:modelData.ready?T3I18n.tr("Done"):T3I18n.tr("Offline");color:root.dim;font.pixelSize:Style.font.caption}
       }
      }
      Text{Layout.fillWidth:true;text:T3I18n.tr("Model options");color:root.fg;font.pixelSize:Style.font.body}
      Repeater{model:(root.bridge.catalogs[root.environment] || {}).models || []
       RowLayout{required property var modelData;Layout.fillWidth:true
        Text{Layout.fillWidth:true;text:modelData.displayName || modelData.model;color:root.dim;elide:Text.ElideRight;font.pixelSize:Style.font.caption}
        T3IconButton{readonly property bool favorite:((root.bridge.catalogs[root.environment] || {}).favorites || []).some(function(f){return f.provider===modelData.instanceId&&f.model===modelData.model});name:favorite?"star-filled":"star";tint:favorite?Color.accent:root.dim;tooltipText:T3I18n.tr("Model options");onClicked:root.bridge.toggleModelFavoriteIn(root.environment,modelData.instanceId,modelData.model)}
       }
      }
     }
     Text{visible:root.page==="shortcuts";Layout.fillWidth:true;text:root.shortcuts;color:root.fg;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.body}
     Item{Layout.fillWidth:true;Layout.preferredHeight:Style.space(12)}
    }
   }
  }
 }
}
