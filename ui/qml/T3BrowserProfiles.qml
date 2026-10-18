import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeBrowserPreferences.js" as Original
ColumnLayout {
 id:root
 objectName:"t3BrowserProfiles"
 required property var bridge
 required property var targets
 readonly property var client:bridge.clientSettings || ({})
 readonly property var values:client.data || ({})
 readonly property var operation:bridge.browserProfileOperation || ({})
 readonly property var view:bridge.browserProfileView || ({})
 readonly property int maxProfiles:Original.nativeBrowser.BROWSER_PROFILE_MAX_COUNT
 readonly property var userProfiles:values.browserProfiles || []
 readonly property var profiles:Original.nativeBrowser.resolveBrowserProfiles(userProfiles).filter(function(p){return p.kind!=="incognito"})
 readonly property var importOperation:bridge.browserImportOperation||({})
 readonly property var importCatalog:bridge.browserImportCatalog||({sources:[]})
 readonly property string primaryImportEnvironment:{var state=bridge.state||{},id=typeof state.primaryEnvironmentId==="string"?state.primaryEnvironmentId:state.primaryEnvironmentId===null?"":(bridge.primaryEnvironmentId||"");return (state.environments||[]).some(function(e){return e.id===id&&e.state==="ready"})?id:""}
 readonly property var importSources:(importCatalog.sources||[]).filter(function(s){return s.unavailable!=="notInstalled"&&s.unavailable!=="unsupportedPlatform"})
 readonly property bool pending:operation.state==="pending"||importOperation.state==="pending"
 readonly property bool writable:!!values.revision&&!client.busy&&!client.error&&!pending&&operation.state!=="uncertain"&&importOperation.state!=="uncertain"
 readonly property bool cleanupAvailable:writable&&!view.busy&&!view.error&&!!view.data&&!!view.data.available
 readonly property string defaultId:(profiles.find(function(p){return p.id===root.values.browserDefaultProfileId}) || {id:"default"}).id
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 property var confirmation:null
 property string observedRevision:""
 spacing:Style.space(10)
 function profileLabel(profile){return profile.id==="default"?T3I18n.tr("Default"):profile.name}
 function outcomeLabel(state){return T3I18n.tr(state==="complete"?"Completed":state==="failed"?"Failed":state==="pending"?"Pending":state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":state)}
 function refresh(){bridge.requestBrowserProfiles(targets,true)}
 function create(){if(!writable||userProfiles.length>=maxProfiles)return;var names=profiles.map(function(p){return p.name}),name="New profile",index=2;while(names.indexOf(name)>=0)name="New profile "+index++;bridge.requestClientSettings({browserProfiles:userProfiles.concat([{id:"profile-"+bridge.uuid(),name:name,kind:"persistent"}])})}
 function rename(id,name){if(!writable||Original.nativeBrowser.isBuiltInBrowserProfileId(id))return;name=name.trim().slice(0,Original.nativeBrowser.BROWSER_PROFILE_NAME_MAX_LENGTH);if(!name)return;var current=userProfiles.find(function(p){return p.id===id});if(!current||current.name===name)return;bridge.requestClientSettings({browserProfiles:userProfiles.map(function(p){return p.id===id?Object.assign({},p,{name:name}):p})})}
 function setDefault(id){if(writable&&profiles.some(function(p){return p.id===id}))bridge.requestClientSettings({browserDefaultProfileId:id})}
 function ask(action,profile){if(!cleanupAvailable||action==="remove"&&profile.id==="default")return;confirmation={action:action,profile:JSON.parse(JSON.stringify(profile))};confirmationPopup.open()}
 function confirm(){if(confirmation&&cleanupAvailable)bridge.browserProfileAction(confirmation.action,confirmation.profile)}
 onTargetsChanged:{confirmation=null;confirmationPopup.close();bridge.requestBrowserProfiles(targets,false)}
 Component.onCompleted:bridge.requestBrowserProfiles(targets,false)
 Component.onDestruction:bridge.dismissBrowserProfiles()
 Connections{target:root.bridge
  function onClientSettingsChanged(){var c=root.client,revision=(c.data || {}).revision || "";if(!c.busy&&!c.error&&revision&&revision!==root.observedRevision){root.observedRevision=revision;if(!root.pending)root.refresh()}}
  function onBrowserProfileOperationChanged(){if(root.operation.state==="complete"&&root.confirmation&&root.operation.profileId===root.confirmation.profile.id){confirmationPopup.close();root.confirmation=null}}
 }
 RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
  ColumnLayout{Layout.fillWidth:true
   Text{text:T3I18n.tr("Browser profiles");color:root.fg;font.pixelSize:Style.font.body}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Profiles separate cookies and logins. Incognito data is cleared when the app closes.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3PickerButton{objectName:"browserProfileAdd";primary:false;text:T3I18n.tr("Add profile");enabled:root.writable;onClicked:addMenu.popup()
   Controls.Menu{id:addMenu;objectName:"browserProfileAddMenu";onOpened:if(typeof root.bridge.requestBrowserImportSources==="function")root.bridge.requestBrowserImportSources();palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
    background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2)}
    Controls.MenuItem{objectName:"browserProfileBlank";text:T3I18n.tr("Blank profile");enabled:root.writable&&root.userProfiles.length<root.maxProfiles;onTriggered:root.create()}
    Controls.MenuItem{text:T3I18n.tr("You've reached the profile limit");visible:root.userProfiles.length>=root.maxProfiles;height:visible?implicitHeight:0;enabled:false}
    Controls.MenuSeparator{}
    Controls.MenuItem{text:T3I18n.tr("Import from");enabled:false}
    Controls.MenuItem{text:T3I18n.tr("Looking for browsers…");visible:!!root.importCatalog.busy&&!root.importCatalog.sources.length;height:visible?implicitHeight:0;enabled:false}
    Controls.MenuItem{text:T3I18n.tr("No supported browsers found");visible:!root.importCatalog.busy&&!root.importSources.length;height:visible?implicitHeight:0;enabled:false}
    Instantiator{model:root.importSources;delegate:Controls.MenuItem{required property var modelData;objectName:"browserImportMenu_"+modelData.id;text:modelData.name;enabled:root.writable&&!!root.primaryImportEnvironment;onTriggered:if(typeof root.bridge.openBrowserImport==="function")root.bridge.openBrowserImport(modelData)} onObjectAdded:function(index,object){addMenu.insertItem(6+index,object)};onObjectRemoved:function(index,object){addMenu.removeItem(object)}}
    Controls.MenuItem{text:T3I18n.tr("Connect to an environment to import cookies");visible:!root.primaryImportEnvironment;height:visible?implicitHeight:0;enabled:false}
   }
  }
 }
 Rectangle{Layout.fillWidth:true;Layout.leftMargin:Style.space(14);Layout.rightMargin:Style.space(14);implicitHeight:profileRows.implicitHeight;color:"transparent";radius:Style.space(6);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)
  Column{id:profileRows;width:parent.width;spacing:0
 Repeater{model:root.profiles
  Rectangle{required property var modelData;required property int index;width:parent.width;implicitHeight:profileRow.implicitHeight+Style.space(16);color:"transparent"
   Rectangle{visible:index>0;anchors.top:parent.top;width:parent.width;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
   RowLayout{id:profileRow;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(8);spacing:Style.space(12)
    Controls.TextField{objectName:"browserProfileName_"+modelData.id;Layout.fillWidth:true;text:root.profileLabel(modelData);enabled:root.writable&&!Original.nativeBrowser.isBuiltInBrowserProfileId(modelData.id);maximumLength:Original.nativeBrowser.BROWSER_PROFILE_NAME_MAX_LENGTH;selectByMouse:true;color:root.fg;font.pixelSize:Style.font.caption;onEditingFinished:root.rename(modelData.id,text);background:Rectangle{color:"transparent";radius:Style.space(4);border.width:parent.activeFocus?1:0;border.color:Color.accent}}
    Text{visible:root.defaultId===modelData.id;text:T3I18n.tr("Default");color:root.dim;font.pixelSize:Style.font.caption}
    T3IconButton{objectName:"browserProfileMenu_"+modelData.id;name:"ellipsis-vertical";tooltipText:T3I18n.tr("Profile actions");onClicked:profileMenu.popup()
     Controls.Menu{id:profileMenu;palette.window:Color.popups.background;palette.text:root.fg;palette.windowText:root.fg
      background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2)}
      Controls.MenuItem{text:T3I18n.tr("Set as default");enabled:root.writable&&root.defaultId!==modelData.id;onTriggered:root.setDefault(modelData.id)}
      Controls.MenuItem{text:T3I18n.tr("Clear cookies and cache");enabled:root.cleanupAvailable;onTriggered:root.ask("clear",modelData)}
      Controls.MenuItem{text:T3I18n.tr("Remove profile and data");visible:modelData.id!=="default";enabled:root.cleanupAvailable&&visible;height:visible?implicitHeight:0;onTriggered:root.ask("remove",modelData)}
     }
    }
   }
  }
 }
  }
 }
 RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
  WorkIndicator{visible:root.view.busy||root.pending;active:visible;implicitWidth:Style.space(16)}
  Text{Layout.fillWidth:true;visible:!!root.view.error||!root.view.busy&&!!root.view.data&&!root.view.data.available;text:root.view.error?T3I18n.error(root.view.error):T3I18n.tr("Connect to an environment to clear profile data");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  T3PickerButton{primary:false;text:T3I18n.tr("Reload");enabled:!root.view.busy&&!root.pending;onClicked:root.refresh()}
 }
 ColumnLayout{visible:!!root.operation.state&&root.operation.state!=="complete";Layout.fillWidth:true;Layout.margins:Style.space(14)
  Text{Layout.fillWidth:true;text:(root.operation.name || "")+" · "+T3I18n.tr(root.pending?"Operation pending in native T3.":root.operation.state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":"Failed")+(root.operation.error?" · "+T3I18n.error(root.operation.error):"");color:root.pending?root.dim:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Repeater{model:root.operation.results || [];Text{required property var modelData;Layout.fillWidth:true;text:modelData.environment+" · "+root.outcomeLabel(modelData.state)+(modelData.error?" · "+T3I18n.error(modelData.error):"");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}}
  T3PickerButton{visible:!root.pending;primary:false;text:T3I18n.tr("Dismiss");onClicked:root.bridge.acknowledgeBrowserProfile()}
 }
 Controls.Popup{id:confirmationPopup;objectName:"browserProfileConfirmation";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(500),parent?parent.width-Style.space(32):Style.space(500));padding:Style.space(20);modal:true;focus:true;closePolicy:root.pending?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2)}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{objectName:"browserProfileConfirmationTitle";Layout.fillWidth:true;text:T3I18n.tr(root.confirmation&&root.confirmation.action==="remove"?"Remove profile and data":"Clear cookies and cache")+" · "+(root.confirmation?root.profileLabel(root.confirmation.profile):"");color:root.fg;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:T3I18n.tr("You are signed out of its sites. Server browser tabs open in this profile close now.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{visible:!!root.operation.error;Layout.fillWidth:true;text:T3I18n.error(root.operation.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   RowLayout{Layout.alignment:Qt.AlignRight
    T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.pending;onClicked:confirmationPopup.close()}
    T3PickerButton{objectName:"browserProfileConfirm";text:T3I18n.tr(root.confirmation&&root.confirmation.action==="remove"?"Remove profile":"Clear cookies and cache");enabled:root.cleanupAvailable;onClicked:root.confirm()}
   }
  }
 }
 T3BrowserImportWizard{bridge:root.bridge}

}
