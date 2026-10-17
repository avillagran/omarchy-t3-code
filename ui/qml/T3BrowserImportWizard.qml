import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeBrowserImport.js" as Original
Controls.Popup{
 id:root
 objectName:"browserImportWizard"
 required property var bridge
 readonly property var wizard:bridge.browserImportWizard||({})
 readonly property var view:bridge.browserImportView||({})
 readonly property var profileData:view.data||({})
 readonly property var operation:bridge.browserImportOperation||({})
 readonly property var source:wizard.source||({name:"",profiles:[]})
 readonly property var step:wizard.step||({step:"configure"})
 readonly property string phase:step.step||"configure"
 readonly property bool pending:phase==="importing"||operation.state==="pending"
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 readonly property var targets:(profileData.canCreateProfile?[{kind:"new",id:"new",name:"New profile",detail:"Created for these cookies"}]:[]).concat((profileData.profiles||[]).map(function(p){return {kind:"existing",id:p.id,name:p.name,detail:"Existing profile"}}))
 readonly property bool targetMissing:!!wizard.target&&(wizard.target.kind==="new"?!profileData.canCreateProfile:!(profileData.profiles||[]).some(function(p){return p.id===root.wizard.target.profileId}))
 function tr(s,args){return T3I18n.tr(s,args)}
 function count(n){return tr(n===1?"{count} cookie":"{count} cookies",{count:Number(n||0).toLocaleString()})}
 readonly property string titleText:phase==="quit"?tr("Quit {source} to import",{source:source.name}):phase==="checking"?tr("Checking {source}",{source:source.name}):phase==="importing"?tr("Importing cookies"):phase==="done"?(step.imported>0?tr("Imported {count}",{count:count(step.imported)}):step.skipped>0?tr("Skipped {count}",{count:count(step.skipped)}):tr("No cookies found")):phase==="blocked"?tr("Couldn't import from {source}",{source:source.name}):tr("Import from {source}",{source:source.name})
 readonly property string description:phase==="quit"?tr("{source} is open, so its cookies can't be read yet. Quit it, then continue.",{source:source.name}):phase==="checking"?tr("Checking whether the browser has closed."):phase==="importing"?tr("This may take a moment."):phase==="blocked"?(step.reason==="browser_cookie_import_unknown"?tr("Outcome unknown. Check native T3 before retrying."):tr(Original.nativeImport.failureCopy[step.reason]||Original.nativeImport.failureCopy.readFailed)):phase==="done"?(step.imported>0?tr("Added to {profile} for {environment}.",{profile:step.targetName,environment:wizard.environmentName})+(step.skipped>0?" "+tr("{count} skipped.",{count:count(step.skipped)}):""):step.skipped>0?tr("No cookies were imported for {environment}.",{environment:wizard.environmentName}):tr("There were no cookies to import for {environment}.",{environment:wizard.environmentName})):tr("Choose which cookies to import for {environment}.",{environment:wizard.environmentName})
 visible:!!wizard.open
 parent:Controls.Overlay.overlay
 anchors.centerIn:parent
 width:Math.min(Style.space(512),parent?parent.width-Style.space(32):Style.space(512))
 padding:Style.space(20);modal:true;focus:true
 closePolicy:pending?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
 onClosed:if(wizard.open&&!pending)bridge.closeBrowserImport(false)
 background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.2)}
 component Tile:Controls.AbstractButton{
  id:tile
  required property string label
  property string detail:""
  property bool selected:false
  focusPolicy:Qt.StrongFocus
  Accessible.name:label+(detail?" · "+detail:"")
  Accessible.checkable:true
  Accessible.checked:selected
  implicitHeight:tileContent.implicitHeight+Style.space(16)
  padding:Style.space(10)
  background:Rectangle{radius:Style.space(8);color:tile.selected?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.08):tile.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent";border.width:1;border.color:tile.selected||tile.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16)}
  contentItem:RowLayout{id:tileContent;spacing:Style.space(10)
   ColumnLayout{Layout.fillWidth:true;spacing:Style.space(3)
    Text{Layout.fillWidth:true;text:tile.label;color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText;elide:Text.ElideRight}
    Text{Layout.fillWidth:true;visible:!!tile.detail;text:tile.detail;color:root.muted;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}
   }
   Rectangle{implicitWidth:Style.space(16);implicitHeight:width;radius:width/2;color:tile.selected?Color.accent:"transparent";border.width:1;border.color:tile.selected?Color.accent:root.muted;Text{anchors.centerIn:parent;visible:tile.selected;text:"✓";font.pixelSize:Style.font.caption;color:Color.popups.background}}
  }
 }
 contentItem:Rectangle{
  color:Color.popups.background;implicitHeight:body.implicitHeight;implicitWidth:body.implicitWidth
  ColumnLayout{id:body;anchors.fill:parent
  spacing:Style.space(16)
  RowLayout{Layout.fillWidth:true
   Text{objectName:"browserImportTitle";Layout.fillWidth:true;text:root.titleText;color:root.fg;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   T3IconButton{objectName:"browserImportClose";visible:!root.pending;name:"x";tooltipText:root.tr("Close");onClicked:root.bridge.closeBrowserImport(false)}
  }
  Text{Layout.fillWidth:true;text:root.description;color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Controls.ScrollView{visible:root.phase==="configure";Layout.fillWidth:true;Layout.preferredHeight:Math.min(configuration.implicitHeight,Style.space(360));clip:true;contentWidth:availableWidth
   GridLayout{id:configuration;objectName:"browserImportChoices";width:parent.width;columns:width>=Style.space(420)?3:1;columnSpacing:Style.space(16);rowSpacing:Style.space(12)
    ColumnLayout{Layout.fillWidth:true;Layout.preferredWidth:1;Layout.minimumWidth:0;Layout.alignment:Qt.AlignTop;spacing:Style.space(8)
     Text{text:root.tr("From");font.capitalization:Font.AllUppercase;color:root.muted;font.pixelSize:Style.font.caption}
     Repeater{model:root.source.profiles||[];Tile{required property var modelData;required property int index;objectName:"browserImportSource_"+index;Layout.fillWidth:true;label:modelData.name;detail:modelData.cookieCount===undefined?"":modelData.cookieCount===0?root.tr("no cookies"):root.count(modelData.cookieCount);selected:root.wizard.directory===modelData.directory;onClicked:root.bridge.chooseBrowserImportSource(modelData.directory)}}
    }
    Text{Layout.alignment:Qt.AlignCenter;text:configuration.columns===3?"→":"↓";color:root.muted;font.pixelSize:Style.font.body}
    ColumnLayout{Layout.fillWidth:true;Layout.preferredWidth:1;Layout.minimumWidth:0;Layout.alignment:Qt.AlignTop;spacing:Style.space(8)
     Text{text:root.tr("Into");font.capitalization:Font.AllUppercase;color:root.muted;font.pixelSize:Style.font.caption}
     Repeater{model:root.targets;Tile{required property var modelData;objectName:"browserImportTarget_"+modelData.id;Layout.fillWidth:true;label:modelData.kind==="new"?root.tr(modelData.name):modelData.name;detail:root.tr(modelData.detail);selected:!!root.wizard.target&&root.wizard.target.kind===modelData.kind&&(modelData.kind==="new"||root.wizard.target.profileId===modelData.id);onClicked:root.bridge.chooseBrowserImportTarget(modelData.kind,modelData.id)}}
    }
   }
  }
  Text{visible:root.phase==="configure"&&root.targetMissing;Layout.fillWidth:true;text:root.tr(root.wizard.target&&root.wizard.target.kind==="new"?"You've reached the profile limit. Choose an existing profile to import into.":"That profile is no longer available. Choose where to import these cookies.");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  RowLayout{visible:root.phase==="importing"||root.phase==="checking"||root.view.busy;Layout.fillWidth:true;WorkIndicator{active:parent.visible;implicitWidth:Style.space(20)}Text{text:root.tr(root.phase==="importing"?"Importing…":"Checking…");color:root.muted;font.pixelSize:Style.font.body}}
  ColumnLayout{visible:root.phase==="done"&&(root.step.skippedDomains||[]).length>0;Layout.fillWidth:true;Text{text:root.tr("Skipped");color:root.muted;font.pixelSize:Style.font.caption}Text{Layout.fillWidth:true;text:Original.nativeImport.formatSkippedDomains(root.step.skippedDomains||[]);color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}}
  RowLayout{visible:root.phase!=="importing"&&root.phase!=="checking";Layout.fillWidth:true
   Item{Layout.fillWidth:true}
   T3PickerButton{objectName:"browserImportCancel";visible:root.phase!=="done";primary:false;text:root.tr(root.operation.state==="uncertain"?"Dismiss":root.phase==="blocked"?"Close":"Cancel");onClicked:root.bridge.closeBrowserImport(root.operation.state==="uncertain")}
   T3PickerButton{objectName:"browserImportContinue";visible:root.phase!=="blocked"||root.operation.state!=="uncertain"&&Original.nativeImport.isRetryableReason(root.step.reason);primary:true;text:root.tr(root.phase==="quit"?"I've quit it":root.phase==="done"?"Done":root.phase==="blocked"?"Try again":"Import");enabled:root.bridge.deviceWorkerReady&&!root.view.busy&&(root.phase!=="configure"||!root.view.error&&!!root.wizard.directory&&!root.targetMissing&&!!root.profileData.viewId);onClicked:{if(root.phase==="quit")root.bridge.refreshBrowserImport(false);else if(root.phase==="blocked")root.bridge.refreshBrowserImport(true);else if(root.phase==="done")root.bridge.closeBrowserImport(false);else root.bridge.runBrowserImport()}}
  }
 }
 }
}
