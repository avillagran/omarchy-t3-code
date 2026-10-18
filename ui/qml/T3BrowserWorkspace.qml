import QtQuick 6.11
import QtQml.Models
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeBrowserPreferences.js" as Original
Rectangle {
 id:root;objectName:"t3BrowserWorkspace";color:Color.popups.background
 required property var bridge
 signal closed()
 signal preferencesRequested()
 property string owner:""
 property string addressDraft:""
 readonly property var state:bridge.previewState || {}
 readonly property var snapshot:state.snapshot || null
 readonly property var tabs:Object.keys(state.sessions || {}).map(function(id){return state.sessions[id]})
 readonly property var operation:(bridge.previewOperations || {})[JSON.stringify([bridge.selectedEnvironment,bridge.selectedThread])] || {}
 readonly property bool locked:["pending","uncertain"].indexOf(operation.state)>=0
 readonly property bool writable:!!bridge.previewWatch.connected&&!!bridge.previewWatch.canOperate&&state.listLoaded===true&&!locked
 readonly property string url:snapshot&&snapshot.navStatus._tag!=="Idle"?snapshot.navStatus.url:""
 readonly property bool emptyPage:!snapshot||snapshot.navStatus._tag==="Idle"
 readonly property var recentUrls:(state.recentlySeenUrls||[]).slice(0,8)
 readonly property bool loading:!!snapshot&&snapshot.navStatus._tag==="Loading"
 readonly property var profiles:Original.nativeBrowser.resolveBrowserProfiles((bridge.clientSettings.data || {}).browserProfiles || [])
 readonly property string defaultProfile:{var data=bridge.clientSettings.data||{},selected=profiles.find(function(p){return p.id===data.browserDefaultProfileId&&p.kind!=="incognito"});return selected?selected.id:"default"}
 readonly property string profileId:snapshot&&snapshot.profileId || defaultProfile
 readonly property string profileName:{var p=profiles.find(function(p){return p.id===root.profileId});return p?(Original.nativeBrowser.isBuiltInBrowserProfileId(p.id)?T3I18n.tr(p.name):p.name):T3I18n.tr("Removed profile")}
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 readonly property color divider:Qt.rgba(fg.r,fg.g,fg.b,0.10)
 function title(tab){if(tab.navStatus._tag==="Idle")return T3I18n.tr("Browser");if(tab.navStatus.title.trim())return tab.navStatus.title;var match=/^https?:\/\/([^/]+)/.exec(tab.navStatus.url);return match?match[1].replace(/^.*@/,""):T3I18n.tr("Browser")}
 function openTab(profile,address){if(!writable)return false;var input={profileId:profile || defaultProfile,viewport:(bridge.clientSettings.data || {}).browserDefaultViewport || {_tag:"fill"}};if(address)input.address=address;return bridge.previewAction("open",input)}
 function submit(value){var next=String(value).trim();if(!next||!writable)return false;var accepted=snapshot?bridge.previewAction("navigate",{tabId:snapshot.tabId,address:next}):openTab(defaultProfile,next);if(accepted){addressInput.focus=false;body.forceActiveFocus()}return accepted}
 function focusUrl(){addressInput.forceActiveFocus();addressInput.selectAll()}
 function refresh(){if(snapshot&&writable)bridge.previewAction("refresh",{tabId:snapshot.tabId})}
 Component.onCompleted:owner=bridge.mountBrowserWorkspace()
 Component.onDestruction:if(owner)bridge.unmountBrowserWorkspace(owner)
 Rectangle{anchors.left:parent.left;anchors.top:parent.top;anchors.bottom:parent.bottom;width:1;color:root.divider}
 ColumnLayout {
  anchors.fill:parent;spacing:0
  RowLayout {
   objectName:"t3BrowserTabRow"
   Layout.fillWidth:true;Layout.minimumHeight:Style.space(40);Layout.preferredHeight:Style.space(40);Layout.maximumHeight:Style.space(40);Layout.leftMargin:Style.space(8);Layout.rightMargin:Style.space(6);spacing:Style.space(2)
   T3Icon{name:"globe";tint:root.dim}
   Text{visible:!root.tabs.length;text:T3I18n.tr("Browser");color:root.fg;font.pixelSize:Style.font.caption}
   ListView {
    id:tabStrip;objectName:"t3BrowserTabs";Layout.fillWidth:true;Layout.fillHeight:true;orientation:ListView.Horizontal;clip:true;spacing:Style.space(3);model:root.tabs
    delegate:Rectangle {
     required property var modelData;required property int index
     objectName:"t3BrowserTab_"+modelData.tabId;
     width:Math.min(Style.space(190),Math.max(Style.space(110),tabStrip.width/Math.min(3,root.tabs.length)));height:Style.space(32);y:(tabStrip.height-height)/2;radius:Style.space(5)
     color:modelData.tabId===root.state.activeTabId?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07):"transparent"
     border.width:modelData.tabId===root.state.activeTabId?1:0;border.color:root.divider
     Accessible.role:Accessible.PageTab;Accessible.name:root.title(modelData);Accessible.selected:modelData.tabId===root.state.activeTabId;activeFocusOnTab:true
     Keys.onReturnPressed:root.bridge.activatePreviewTab(modelData.tabId)
     RowLayout{anchors.fill:parent;anchors.leftMargin:Style.space(8);spacing:Style.space(3);Text{Layout.fillWidth:true;text:root.title(modelData);textFormat:Text.PlainText;elide:Text.ElideRight;color:root.fg;font.pixelSize:Style.font.caption}T3IconButton{objectName:"t3BrowserCloseTab_"+modelData.tabId;name:"x";implicitWidth:Style.space(22);tooltipText:T3I18n.tr("Close tab");enabled:root.writable;onClicked:root.bridge.previewAction("close",{tabId:modelData.tabId})}}
     MouseArea{anchors.fill:parent;anchors.rightMargin:Style.space(24);onClicked:root.bridge.activatePreviewTab(modelData.tabId)}
    }
   }
   T3IconButton{objectName:"t3BrowserNewTab";name:"plus";tooltipText:T3I18n.tr("New tab");enabled:root.writable;onClicked:root.openTab(root.defaultProfile,"")}
   T3IconButton{objectName:"t3BrowserProfileMenu";name:"chevron-down";tooltipText:T3I18n.tr("Browser profiles");enabled:root.writable;onClicked:profileMenu.popup()}
   T3IconButton{objectName:"t3BrowserClosePanel";name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.closed()}
  }
  Rectangle{Layout.fillWidth:true;height:1;color:root.divider}
  RowLayout {
   objectName:"t3BrowserChromeRow"
   Layout.fillWidth:true;Layout.minimumHeight:Style.space(40);Layout.preferredHeight:Style.space(40);Layout.maximumHeight:Style.space(40);Layout.leftMargin:Style.space(7);Layout.rightMargin:Style.space(7);spacing:Style.space(2)
   T3IconButton{objectName:"t3BrowserBack";name:"arrow-left";implicitWidth:Style.space(24);tooltipText:T3I18n.tr("Back");enabled:root.writable&&!!root.snapshot&&root.snapshot.canGoBack&&root.bridge.browserSurface.interactive;onClicked:root.bridge.browserControl("back")}
   T3IconButton{objectName:"t3BrowserForward";name:"arrow-right";implicitWidth:Style.space(24);tooltipText:T3I18n.tr("Forward");enabled:root.writable&&!!root.snapshot&&root.snapshot.canGoForward&&root.bridge.browserSurface.interactive;onClicked:root.bridge.browserControl("forward")}
   T3IconButton{objectName:"t3BrowserRefresh";name:"refresh-cw";implicitWidth:Style.space(24);tooltipText:T3I18n.tr(root.loading?"Loading…":"Refresh");tint:root.loading?Color.accent:root.dim;enabled:root.writable&&!!root.snapshot;onClicked:root.refresh()}
   Rectangle{visible:!!root.snapshot&&root.profileId!=="default";implicitWidth:Math.min(Style.space(90),profileText.implicitWidth+Style.space(12));implicitHeight:Style.space(22);radius:Style.space(4);border.width:1;border.color:root.divider;color:"transparent";Text{id:profileText;anchors.fill:parent;anchors.margins:Style.space(4);text:root.profileName;elide:Text.ElideRight;textFormat:Text.PlainText;color:root.dim;font.pixelSize:Style.font.caption}}
   Controls.TextField {
    id:addressInput;objectName:"t3BrowserAddress";maximumLength:2048;Layout.fillWidth:true;implicitHeight:Style.space(28);text:activeFocus?root.addressDraft:root.url;placeholderText:T3I18n.tr("Search or enter URL");enabled:root.writable;selectByMouse:true;color:root.fg;placeholderTextColor:root.dim;selectionColor:Color.accent;selectedTextColor:root.fg;font.pixelSize:Style.font.caption
    onActiveFocusChanged:if(activeFocus){root.addressDraft=root.url;Qt.callLater(selectAll)}
    onTextEdited:root.addressDraft=text
    onAccepted:root.submit(text)
    Keys.onEscapePressed:function(e){root.addressDraft=root.url;focus=false;e.accepted=true}
    background:Rectangle{radius:Style.space(5);color:addressInput.activeFocus?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent";border.width:addressInput.activeFocus?1:0;border.color:Color.accent}
   }
   T3IconButton{objectName:"t3BrowserOpenExternal";name:"external-link";implicitWidth:Style.space(24);tooltipText:T3I18n.tr("Open in system browser");enabled:!!root.url;onClicked:root.bridge.openReference(root.url)}
   T3IconButton{objectName:"t3BrowserMore";name:"ellipsis-vertical";implicitWidth:Style.space(24);tooltipText:T3I18n.tr("Preview menu");onClicked:moreMenu.popup()}
  }
  Rectangle{Layout.fillWidth:true;height:1;color:root.divider}
  RowLayout{
   visible:!!root.operation.error||!!root.bridge.previewWatch.error;Layout.fillWidth:true;Layout.margins:Style.space(8)
   Text{Layout.fillWidth:true;text:T3I18n.error(root.operation.error||root.bridge.previewWatch.error);color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   Controls.Button{visible:root.operation.state==="uncertain";text:T3I18n.tr("Refresh status");onClicked:root.bridge.acknowledgePreviewOperation()}
  }
  Item {
   id:body;Layout.fillWidth:true;Layout.fillHeight:true
   T3BrowserView{anchors.fill:parent;bridge:root.bridge;visible:!!root.snapshot&&root.snapshot.navStatus._tag!=="Idle"}
   ColumnLayout {
    anchors.centerIn:parent;width:Math.min(parent.width-Style.space(40),Style.space(420));spacing:Style.space(24)
    visible:root.state.listLoaded===true&&root.emptyPage&&!root.recentUrls.length&&root.bridge.previewWatch.canOperate
    Item{Layout.alignment:Qt.AlignHCenter;implicitWidth:Style.space(36);implicitHeight:Style.space(36);Layout.bottomMargin:Style.space(24)
     Repeater{model:[-10,10];delegate:Rectangle{required property int modelData;width:parent.width*0.84;height:parent.height*0.84;anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;rotation:modelData;radius:Style.space(6);color:root.color;border.width:1;border.color:root.divider}}
     Rectangle{anchors.fill:parent;radius:Style.space(6);color:root.color;border.width:1;border.color:root.divider;T3Icon{anchors.centerIn:parent;name:"globe";implicitWidth:Style.space(18);tint:root.dim}}
    }
    Text{Layout.fillWidth:true;text:T3I18n.tr("No preview yet");color:root.fg;font.pixelSize:Style.space(20);font.weight:Font.DemiBold;horizontalAlignment:Text.AlignHCenter}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Search or enter URL");color:root.dim;font.pixelSize:Style.space(14);horizontalAlignment:Text.AlignHCenter;Layout.topMargin:Style.space(4)}

   }
   Controls.ScrollView{
    objectName:"t3BrowserRecentUrls";anchors.fill:parent;anchors.margins:Style.space(20);visible:root.emptyPage&&root.recentUrls.length>0&&root.bridge.previewWatch.canOperate;clip:true
    ColumnLayout{width:parent.width;spacing:Style.space(12)
     Text{text:T3I18n.tr("Recently used");color:root.dim;font.pixelSize:Style.space(14)}
     Repeater{model:root.recentUrls;delegate:Controls.Button{required property string modelData;required property int index;objectName:"t3BrowserRecent_"+index;Layout.fillWidth:true;text:modelData;enabled:root.writable;onClicked:root.submit(modelData);contentItem:Text{text:parent.text;color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}background:Rectangle{color:parent.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent";radius:Style.space(6);border.width:1;border.color:root.divider}}}
    }
   }
   Text{anchors.centerIn:parent;width:parent.width-Style.space(40);visible:root.state.listLoaded===true&&!root.bridge.previewWatch.canOperate;text:T3I18n.tr("Pair this client again with preview access to control browser previews.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;horizontalAlignment:Text.AlignHCenter}
   WorkIndicator{anchors.centerIn:parent;visible:root.bridge.previewWatch.busy;active:visible}
  }
 }
 Controls.Menu {
  id:profileMenu;objectName:"t3BrowserProfilesMenu";parent:root;x:Math.max(0,root.width-width-8);y:Style.space(40);width:Style.space(240);palette.window:root.color;palette.base:root.color;palette.text:root.fg;palette.windowText:root.fg;palette.buttonText:root.fg;palette.highlight:Color.accent;palette.highlightedText:root.fg
  background:Rectangle{color:root.color;border.width:1;border.color:root.divider;radius:Style.space(6)}
  Instantiator{model:root.profiles;delegate:Controls.MenuItem{required property var modelData;text:Original.nativeBrowser.isBuiltInBrowserProfileId(modelData.id)?T3I18n.tr(modelData.name):modelData.name;enabled:root.writable;onTriggered:root.openTab(modelData.id,"")}
   onObjectAdded:function(index,object){profileMenu.insertItem(index,object)};onObjectRemoved:function(index,object){profileMenu.removeItem(object)}}
 }
 Controls.Menu {
  id:moreMenu;objectName:"t3BrowserMoreMenu";parent:root;x:Math.max(0,root.width-width-8);y:Style.space(80);width:Style.space(240);palette.window:root.color;palette.base:root.color;palette.text:root.fg;palette.windowText:root.fg;palette.buttonText:root.fg;palette.highlight:Color.accent;palette.highlightedText:root.fg
  background:Rectangle{color:root.color;border.width:1;border.color:root.divider;radius:Style.space(6)}
  Controls.MenuItem{text:T3I18n.tr("Hard reload");enabled:root.writable&&root.bridge.browserSurface.interactive;onTriggered:root.bridge.browserControl("hard-reload")}
  Controls.MenuSeparator{}
  Controls.MenuItem{text:T3I18n.tr("Zoom in");enabled:root.bridge.browserSurface.interactive;onTriggered:root.bridge.browserControl("zoom-in")}
  Controls.MenuItem{text:T3I18n.tr("Zoom out");enabled:root.bridge.browserSurface.interactive;onTriggered:root.bridge.browserControl("zoom-out")}
  Controls.MenuItem{text:T3I18n.tr("Reset zoom")+" · "+Math.round((root.bridge.browserSurface.zoomFactor||1)*100)+"%";enabled:root.bridge.browserSurface.interactive;onTriggered:root.bridge.browserControl("zoom-reset")}
  Controls.MenuSeparator{}
  Controls.MenuItem{text:T3I18n.tr("Preferences");onTriggered:root.preferencesRequested()}
 }
}
