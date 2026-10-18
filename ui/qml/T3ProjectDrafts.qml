import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
// QML port of native ComposerStashMenu: the editor and chat instantiate this
// same component, backed by the same project-scoped collection.
Rectangle {
 id:root
 required property var bridge
 signal closeRequested()
 readonly property var state:bridge.projectDrafts || ({entries:[],busy:false,error:""})
 readonly property var entries:typeof bridge.projectDraftScope==="function"&&state.scope===bridge.projectDraftScope()?(state.entries || []):[]
 readonly property bool locked:!!state.busy || !!bridge.draft.busy || !!bridge.draft.attempt || !!bridge.draft.queueEdit || !!bridge.draft.loading || !!bridge.draft.loadError
 readonly property color fg:Color.popups.text
 implicitHeight:header.implicitHeight+list.height+Style.space(24)+(errorText.visible?errorText.implicitHeight+Style.space(8):0)
 color:Qt.rgba(fg.r,fg.g,fg.b,0.025);radius:Style.space(8)
 border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.1)
 function focusList(){list.forceActiveFocus()}
 function refresh(){bridge.requestProjectDrafts("list")}
 function pick(index){var entry=entries[index];if(entry&&!locked&&!entry.locked)bridge.requestProjectDrafts("restore",entry)}
 function snippet(entry){var text=String(entry.prompt || "").trim().replace(/\s+/g," ");return text?text.slice(0,90)+(text.length>90?"…":""):T3I18n.tr("Attachments")+" ("+entry.attachmentCount+")"}
 ColumnLayout {
  anchors.fill:parent;anchors.margins:Style.space(10);spacing:Style.space(4)
  RowLayout {
   id:header;Layout.fillWidth:true
   T3Icon{name:"bookmark";tint:Color.accent}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Project drafts")+" ("+root.entries.length+")";color:root.fg;font.pixelSize:Style.font.caption;font.bold:true}
   WorkIndicator{visible:root.state.busy;active:visible&&root.visible;implicitWidth:Style.space(16)}
   T3IconButton{objectName:"t3StashPrompt";name:"bookmark-plus";tooltipText:T3I18n.tr("Save to project drafts");enabled:!root.locked&&(!!root.bridge.draft.text.trim()||root.bridge.draft.attachments.length>0);onClicked:root.bridge.requestProjectDrafts("stash")}
   T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.state.busy&&(!root.locked||!!root.state.command);onClicked:root.state.error&&root.state.command?root.bridge.retryProjectDrafts():root.refresh()}
   T3IconButton{objectName:"t3CloseProjectDrafts";name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.closeRequested()}
  }
  ListView {
   id:list;objectName:"t3ProjectDraftList";Layout.fillWidth:true
   height:Math.min(Style.space(170),Math.max(Style.space(36),contentHeight));Layout.preferredHeight:height
   clip:true;model:root.entries;currentIndex:count?0:-1;activeFocusOnTab:true
   Keys.onDownPressed:if(count)currentIndex=(currentIndex+1)%count
   Keys.onUpPressed:if(count)currentIndex=(currentIndex+count-1)%count
   Keys.onReturnPressed:root.pick(currentIndex)
   Keys.onEscapePressed:root.closeRequested()
   delegate:RowLayout {
    required property var modelData;required property int index
    width:list.width;height:Style.space(44);spacing:Style.space(4)
    Controls.ItemDelegate {
     Layout.fillWidth:true;Layout.fillHeight:true;hoverEnabled:true;enabled:!root.locked&&!modelData.locked
     objectName:"projectDraft_"+index
     onClicked:{list.currentIndex=index;root.pick(index)}
     background:Rectangle{radius:Style.space(5);color:parent.hovered||list.currentIndex===index?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06):"transparent"}
     contentItem:RowLayout {
      T3Icon{name:"file-text";tint:root.fg;opacity:0.6;implicitWidth:Style.space(14)}
      Text{Layout.fillWidth:true;text:root.snippet(modelData);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
      Text{visible:modelData.attachmentCount>0;text:"📎 "+modelData.attachmentCount;color:root.fg;opacity:0.6;font.pixelSize:Style.font.caption}
      T3Icon{visible:!!modelData.locked||modelData.sourceKey===root.bridge.selectedKey;name:modelData.locked?"lock":"check";tint:modelData.locked?root.fg:Color.accent;implicitWidth:Style.space(14)}
     }
    }
    T3IconButton{objectName:"deleteProjectDraft_"+index;name:"x";tooltipText:T3I18n.tr("Delete draft");enabled:!root.locked&&!modelData.locked;onClicked:root.bridge.requestProjectDrafts("delete",modelData)}
   }
   Text{anchors.centerIn:parent;visible:!list.count&&!root.state.busy;text:T3I18n.tr("No project drafts");color:root.fg;opacity:0.55;font.pixelSize:Style.font.caption}
   T3Wheel{flickable:list}
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
  }
  Text{id:errorText;Layout.fillWidth:true;visible:!!root.state.error;text:root.state.error==="draft_stash_full"?T3I18n.tr("Project drafts are full. Delete one before saving another."):root.state.error==="draft_changed"?T3I18n.tr("Draft changed. Refresh the list and try again."):T3I18n.error(root.state.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 }
}
