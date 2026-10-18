import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Port of ArchivedThreadsPanel: native archive query, scoped projects, native commands.
Item {
 id:root
 required property var bridge
 required property var targets
 readonly property var view:bridge.archiveView
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 readonly property var sessions:view.sessions || ({})
 readonly property bool loading:Object.keys(sessions).some(function(k){return root.sessions[k].busy})
 readonly property var errors:Object.keys(sessions).filter(function(k){return !!root.sessions[k].error}).map(function(k){return {environment:k,error:root.sessions[k].error}})
 readonly property var operations:Object.keys(bridge.archiveActions).map(function(k){return root.bridge.archiveActions[k]}).filter(function(r){var s=root.sessions[r.environment];return s&&s.data&&s.data.originKey===r.originKey&&(s.scope.projectIds===null||s.scope.projectIds.indexOf(r.projectId)>=0)&&r.state!=="complete"})
 property var selected:null
 property var pendingDelete:null
 property double clockNow:Date.now()
 property int rowCount:0
 implicitHeight:Style.space(440)
 function rowFor(key){for(var i=0;i<rows.count;i++){var r=rows.get(i);if(r.rowKey===key)return JSON.parse(r.payload)}return null}
 function allowed(row){var s=row&&sessions[row.environment];return !!row&&!!s&&!s.busy&&!s.error&&!!s.data&&row.viewId===s.data.viewId&&!!row.writable&&!bridge.archiveBlocking(bridge.archiveActions[bridge.archiveKey(row)])}
 function restore(row){if(allowed(row))bridge.requestArchiveAction(row,"unarchive",false)}
 function remove(row){if(!allowed(row))return;selected=null;menu.close();if(((bridge.clientSettings || {}).data || {}).confirmThreadDelete===false)bridge.requestArchiveAction(row,"delete",true);else{pendingDelete=JSON.parse(JSON.stringify(row));confirmation.open()}}
 function confirmDelete(){var row=pendingDelete;confirmation.close();pendingDelete=null;if(allowed(row))bridge.requestArchiveAction(row,"delete",true)}
 function context(key,item){var r=rowFor(key);if(!r)return;selected=r;menu.popup(item,0,item.height)}
 // Keep the virtual list and scroll position stable across read/busy/action receipts.
 function rebuild(){
  if(!rows)return
  var next=[]
  Object.keys(sessions).sort().forEach(function(environment){var s=root.sessions[environment],d=s.data;if(!d)return;(d.groups || []).forEach(function(g){var groupKey=environment+"/"+d.originKey+"/"+g.project.id;(g.threads || []).forEach(function(t){var r=Object.assign({},t,{originKey:d.originKey,viewId:d.viewId,writable:!!d.writable,project:g.project});next.push({rowKey:root.bridge.archiveKey(r),groupKey:groupKey,groupTitle:g.project.title || "",groupIcon:JSON.stringify(g.project),environment:environment,payload:JSON.stringify(r)})})})})
  for(var i=0;i<next.length;i++){var key=next[i].rowKey,j=i;while(j<rows.count&&rows.get(j).rowKey!==key)j++;if(j===rows.count)rows.insert(i,next[i]);else{if(j!==i)rows.move(j,i,1);rows.set(i,next[i])}}
  if(rows.count>next.length)rows.remove(next.length,rows.count-next.length)
  rowCount=rows.count
 }
 function relative(iso){var ms=new Date(iso).getTime();if(!isFinite(ms))return "—";var m=Math.max(0,Math.floor((clockNow-ms)/60000));return m<1?T3I18n.tr("now"):T3I18n.tr("{time} ago",{time:m<60?m+"m":m<1440?Math.floor(m/60)+"h":Math.floor(m/1440)+"d"})}
 onSessionsChanged:rebuild()
 onTargetsChanged:{selected=null;pendingDelete=null;if(menu)menu.close();if(confirmation)confirmation.close();if(bridge)bridge.requestArchive(targets,false)}
 Component.onCompleted:{bridge.requestArchive(targets,false);rebuild()}
 Component.onDestruction:bridge.dismissArchive()
 Timer{interval:3000;running:root.visible&&root.operations.some(function(r){return r.state==="accepted"||r.state==="pending"})&&!root.loading&&!Object.keys(root.bridge.archiveCommands).length;repeat:true;onTriggered:root.bridge.requestArchive(root.targets,true)}
 Timer{interval:60000;running:root.visible;repeat:true;onTriggered:root.clockNow=Date.now()}
 ListModel{id:rows}
 ColumnLayout {
  anchors.fill:parent;spacing:Style.space(10)
  Text{visible:!!root.view.error;Layout.fillWidth:true;text:T3I18n.error(root.view.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Repeater{model:root.errors
   RowLayout{required property var modelData;Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:modelData.environment+": "+T3I18n.error(modelData.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    T3PickerButton{primary:false;text:T3I18n.tr("Retry");enabled:!root.loading;onClicked:root.bridge.requestArchive(root.targets,true)}
   }
  }
  Repeater{model:root.operations
   ColumnLayout{required property var modelData;Layout.fillWidth:true;spacing:Style.space(4)
    Text{Layout.fillWidth:true;text:modelData.title+" · "+T3I18n.tr(modelData.state==="failed"?"Failed":modelData.state==="uncertain"?"Outcome unknown. Check native T3 before retrying.":"Operation pending in native T3.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Text{visible:!!modelData.error;Layout.fillWidth:true;text:T3I18n.error(modelData.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    RowLayout{visible:root.bridge.archiveBlocking(modelData)
     readonly property bool busy:Object.keys(root.bridge.archiveCommands).some(function(k){return root.bridge.archiveCommands[k].actionId===modelData.actionId})
     T3PickerButton{objectName:"archiveCheck_"+modelData.threadId;primary:false;text:T3I18n.tr("Check status");enabled:!parent.busy;onClicked:root.bridge.checkArchive(modelData,false)}
     T3PickerButton{objectName:"archiveRetry_"+modelData.threadId;primary:false;text:T3I18n.tr("Retry");visible:modelData.state==="uncertain";enabled:!parent.busy;onClicked:root.bridge.checkArchive(modelData,true)}
    }
   }
  }
  RowLayout{visible:!root.rowCount;Layout.fillWidth:true;Layout.topMargin:Style.space(12)
   T3Icon{name:"archive";tint:root.dim;implicitWidth:Style.space(16)}
   WorkIndicator{visible:root.loading;active:visible;implicitWidth:Style.space(16)}
   ColumnLayout{Layout.fillWidth:true;spacing:Style.space(6)
    Text{Layout.fillWidth:true;text:T3I18n.tr(root.loading?"Loading archived threads":root.errors.length||root.view.error?"Could not load archived threads":"No archived threads");color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
    Text{Layout.fillWidth:true;text:root.loading?T3I18n.tr("Checking connected environments."):root.view.error?T3I18n.error(root.view.error):root.errors.length?T3I18n.error(root.errors[0].error):T3I18n.tr("Archived threads will appear here.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   }
  }
  ListView {
   id:list;objectName:"t3ArchivedThreadList";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:rows;reuseItems:true;activeFocusOnTab:true;boundsBehavior:Flickable.StopAtBounds
   section.property:"groupKey";section.criteria:ViewSection.FullString
   section.delegate:Item{
    required property string section
    readonly property var first:{for(var i=0;i<rows.count;i++)if(rows.get(i).groupKey===section)return rows.get(i);return null}
    width:list.width;height:Style.space(48)
    RowLayout{anchors.left:parent.left;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;spacing:Style.space(8);anchors.leftMargin:Style.space(12);anchors.rightMargin:Style.space(12)
     T3ProjectIcon{project:parent.parent.first?JSON.parse(parent.parent.first.groupIcon):({});projectKey:parent.parent.section;monogram:project.title?project.title.slice(0,2).toUpperCase():"PR"}
     Text{Layout.fillWidth:true;text:parent.parent.first?parent.parent.first.groupTitle:"";color:root.dim;font.pixelSize:Style.font.caption;font.bold:true;elide:Text.ElideRight;textFormat:Text.PlainText}
     Text{visible:Object.keys(root.sessions).length>1;text:parent.parent.first?parent.parent.first.environment:"";color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    }
   }
   delegate:Item{
    id:row;objectName:"archiveRow_"+entry.id;required property string rowKey;required property string payload;required property int index
    readonly property var entry:JSON.parse(payload)
    width:list.width;height:Math.max(Style.space(68),details.implicitHeight+Style.space(26))
    Rectangle{anchors.fill:parent;radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.022);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06)}
    TapHandler{acceptedButtons:Qt.RightButton;onTapped:root.context(row.rowKey,row)}
    Keys.onPressed:function(event){if(event.key===Qt.Key_Menu||event.key===Qt.Key_F10&&event.modifiers===Qt.ShiftModifier){root.context(row.rowKey,row);event.accepted=true}}
    RowLayout{anchors.fill:parent;anchors.margins:Style.space(13);spacing:Style.space(12)
     ColumnLayout{id:details;Layout.fillWidth:true;spacing:Style.space(5)
      Text{Layout.fillWidth:true;text:row.entry.title;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:T3I18n.tr("Archived {archived} · Created {created}",{archived:root.relative(row.entry.archivedAt || row.entry.createdAt),created:root.relative(row.entry.createdAt)});color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
     }
     T3PickerButton{objectName:"archiveUnarchive_"+row.entry.id;primary:false;text:T3I18n.tr("Unarchive");implicitHeight:Style.space(28);leftPadding:Style.space(10);rightPadding:leftPadding;enabled:root.allowed(row.entry);onClicked:root.restore(row.entry);contentItem:RowLayout{spacing:Style.space(6);T3Icon{name:"archive";tint:root.fg;implicitWidth:Style.space(14)}Text{text:parent.parent.text;color:root.fg;font.pixelSize:Style.font.caption}} background:Rectangle{radius:Style.space(5);color:parent.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06):"transparent";border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}}
    }
   }
   T3Wheel{flickable:list}
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
  }
 }
 Controls.Menu{
  id:menu;objectName:"t3ArchiveMenu"
  palette.window:Color.popups.background;palette.text:root.fg;width:Style.space(220)
  background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:root.dim}
  Controls.MenuItem{objectName:"archiveMenuUnarchive";text:T3I18n.tr("Unarchive");enabled:root.allowed(root.selected);onTriggered:root.restore(root.selected)}
  Controls.MenuItem{objectName:"archiveMenuDelete";text:T3I18n.tr("Delete");enabled:root.allowed(root.selected);palette.text:Color.urgent;onTriggered:root.remove(root.selected)}
 }
 Controls.Popup{
  id:confirmation;objectName:"t3ArchiveDeleteDialog";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(430),parent?parent.width-Style.space(24):Style.space(430));padding:Style.space(20);modal:true;focus:true
  onClosed:root.pendingDelete=null
  background:Rectangle{color:Color.popups.background;radius:Style.space(10);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16)}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:T3I18n.tr('Delete thread "{title}"?',{title:root.pendingDelete?root.pendingDelete.title:""});color:root.fg;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:T3I18n.tr("This permanently clears conversation history for this thread.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout{Layout.alignment:Qt.AlignRight
    T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:confirmation.close()}
    T3PickerButton{objectName:"archiveDeleteConfirm";text:T3I18n.tr("Delete");enabled:root.allowed(root.pendingDelete);onClicked:root.confirmDelete()}
   }
  }
 }
}
