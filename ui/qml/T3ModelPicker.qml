import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import qs.Commons

Controls.Button {
 id:root
 required property var bridge
 property var selection:({})
 property string emptyText:T3I18n.tr("Loading models…")
 property alias popup:menu
 readonly property var models:bridge.catalog.models || []
 readonly property var providers:bridge.catalog.providers || fallbackProviders()
 readonly property var favorites:bridge.catalog.favorites || []
 readonly property int currentIndex:models.findIndex(function(m){return m.instanceId===root.selection.instanceId&&m.model===root.selection.model})
 readonly property var selectedModel:currentIndex>=0?models[currentIndex]:({})
 property string provider:""
 property bool legacyExpanded:false
 property string query:""
 signal picked()
 Accessible.name:selectedModel.shortName || selectedModel.displayName || selection.model || root.emptyText
 implicitHeight:Style.space(30)
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 function fallbackProviders(){var ids={},result=[];models.forEach(function(m){if(!ids[m.instanceId]){ids[m.instanceId]=true;result.push({instanceId:m.instanceId,name:m.providerName || m.instanceId,driverKind:m.driverKind,ready:true})}});return result}
 function favorite(m){return favorites.some(function(f){return f.provider===m.instanceId&&f.model===m.model})}
 function pending(m){return !!(bridge.favoritePending || {})[bridge.selectedEnvironment+"/"+JSON.stringify([m.instanceId,m.model])]}
 function score(m){var text=[m.displayName || m.name,m.model,m.providerName,m.subProvider].join(" ").toLowerCase(),words=query.trim().toLowerCase().split(/\s+/),sum=0;for(var i=0;i<words.length;i++){var w=words[i],at=text.indexOf(w);if(at>=0){sum+=500-at;continue}var p=0;for(var j=0;j<text.length&&p<w.length;j++)if(text[j]===w[p])p++;if(p<w.length)return -1;sum+=100}return sum}
 readonly property var rows:{
   var searching=!!query.trim(),out=models.map(function(m,i){return {model:m,index:i,score:searching?root.score(m):0,favorite:root.favorite(m)}}).filter(function(r){return r.model.available!==false && (root.provider==="favorites"?r.favorite:searching || r.model.instanceId===root.provider) && (!searching || r.score>=0) && (searching || root.provider==="favorites" || root.legacyExpanded || !r.model.isLegacy)})
   out.sort(function(a,b){return b.score-a.score || (root.provider==="favorites"?0:Number(b.favorite)-Number(a.favorite)) || a.index-b.index});return out
 }
 readonly property var rail:[{instanceId:"favorites",name:T3I18n.tr("Favorites"),ready:true}].concat(providers)
 onRowsChanged:if(menu.visible){results.currentIndex=Math.max(0,rows.findIndex(function(r){return r.model.instanceId===root.selection.instanceId&&r.model.model===root.selection.model}));Qt.callLater(function(){results.positionViewAtIndex(results.currentIndex,ListView.Contain)})}
 function selectProvider(id){provider=id;railView.currentIndex=rail.findIndex(function(p){return p.instanceId===id});legacyExpanded=models.some(function(m){return m.instanceId===id&&m.model===root.selection.model&&m.isLegacy});results.positionViewAtBeginning()}
 // ModelPickerContent's adjacent provider behavior over the existing normalized
 // rail: favorites always participates, unavailable providers are skipped.
 function moveProvider(step){var available=rail.filter(function(p){return p.ready!==false}),index=available.findIndex(function(p){return p.instanceId===root.provider});query="";selectProvider(available[index<0?(step>0?0:available.length-1):(index+step+available.length)%available.length].instanceId)}
 function moveResult(step){if(!rows.length)return;results.currentIndex=(results.currentIndex+step+rows.length)%rows.length;results.positionViewAtIndex(results.currentIndex,ListView.Contain)}
 function selectResult(index){var row=rows[index];if(!row || row.model.available===false)return;bridge.chooseModel(row.index);menu.close();picked()}
 contentItem:RowLayout {
  spacing:Style.space(6)
  T3ProviderIcon{provider:root.selectedModel.instanceId?root.selectedModel:root.selection;implicitWidth:Style.space(16);opacity:root.enabled?1:0.45}
  Text{Layout.fillWidth:true;text:root.selectedModel.shortName || root.selectedModel.displayName || root.selectedModel.name || root.selection.model || root.emptyText;color:root.fg;elide:Text.ElideRight;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;opacity:root.enabled?1:0.45}
  T3Icon{name:"chevron-down";implicitWidth:Style.space(12);opacity:root.enabled?1:0.45}
 }
 background:Rectangle{radius:Style.space(7);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,root.hovered?0.09:0.035);border.width:root.activeFocus?1:0;border.color:Color.accent}
 Controls.ToolTip.visible:hovered
 Controls.ToolTip.delay:400
 Controls.ToolTip.text:(selectedModel.displayName || selection.model || "")+(selectedModel.providerName?" · "+selectedModel.providerName:"")
 onClicked:menu.open()
 Controls.Popup {
  id:menu;objectName:"t3ModelPickerPopup"
  x:0;y:-height-Style.space(6);width:Style.space(360);height:Style.space(346);padding:0;margins:Style.space(8)
  modal:true;focus:true;closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  background:Rectangle{radius:Style.space(9);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16)}
  onOpened:{root.query="";var active=root.providers.find(function(p){return p.instanceId===root.selection.instanceId});root.selectProvider(root.favorites.length&&(!active || active.ready!==false)?"favorites":root.selection.instanceId || (root.providers[0] || {}).instanceId || "favorites");root.bridge.refreshCatalog();Qt.callLater(function(){search.forceActiveFocus()})}
  onClosed:root.forceActiveFocus()
  contentItem:Item {
   Rectangle{anchors.fill:parent;color:Color.popups.background;radius:Style.space(9)}
   RowLayout {anchors.fill:parent
   spacing:0
   ListView {
    id:railView;objectName:"t3ModelProviderRail"
    Layout.preferredWidth:Style.space(44);Layout.fillHeight:true;clip:true;spacing:Style.space(3)
    topMargin:Style.space(5);bottomMargin:Style.space(5);model:root.rail
    currentIndex:0
    T3Wheel {flickable:railView}
    Keys.onUpPressed:{currentIndex=(currentIndex+count-1)%count;root.selectProvider(root.rail[currentIndex].instanceId)}
    Keys.onDownPressed:{currentIndex=(currentIndex+1)%count;root.selectProvider(root.rail[currentIndex].instanceId)}
    Keys.onRightPressed:search.forceActiveFocus()
    Keys.onReturnPressed:search.forceActiveFocus()
    delegate:Controls.AbstractButton {
     required property var modelData
     width:railView.width;height:Style.space(38);enabled:modelData.ready!==false
     Accessible.name:modelData.name || modelData.instanceId
     onClicked:{root.selectProvider(modelData.instanceId);search.forceActiveFocus()}
     background:Rectangle{anchors.margins:Style.space(3);color:root.provider===modelData.instanceId?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.09):parent.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent";radius:Style.space(5)}
     Rectangle{x:0;anchors.verticalCenter:parent.verticalCenter;width:Style.space(2);height:Style.space(20);radius:1;visible:root.provider===modelData.instanceId;color:Color.accent}
     contentItem:Item{
      T3Icon{anchors.centerIn:parent;width:Style.space(20);height:width;visible:modelData.instanceId==="favorites";name:"star";tint:root.fg}
      T3ProviderIcon{anchors.centerIn:parent;width:Style.space(20);height:width;visible:modelData.instanceId!=="favorites";provider:modelData;tint:parent.enabled?root.fg:root.dim}
      Rectangle{anchors.right:parent.right;anchors.bottom:parent.bottom;width:Style.space(15);height:Style.space(11);radius:Style.space(2);color:Color.popups.background
       visible:modelData.instanceId!=="favorites"&&root.providers.filter(function(p){return p.driverKind===modelData.driverKind}).length>1
       Text{anchors.centerIn:parent;text:(modelData.name || modelData.instanceId).split(/\s+/).pop().slice(0,2).toUpperCase();font.pixelSize:Style.space(7);color:root.fg;textFormat:Text.PlainText}
      }
     }
     Controls.ToolTip.visible:hovered;Controls.ToolTip.text:(modelData.name || modelData.instanceId)+(enabled?"":" · "+T3I18n.tr("Unavailable"));Controls.ToolTip.delay:250
    }
   }
   Rectangle{Layout.preferredWidth:1;Layout.fillHeight:true;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.09)}
   ColumnLayout {
    Layout.fillWidth:true;Layout.fillHeight:true;spacing:0
    RowLayout{
     Layout.fillWidth:true;Layout.leftMargin:Style.space(10);Layout.rightMargin:Style.space(8)
     T3Icon{name:"search";implicitWidth:Style.space(14);tint:root.dim}
     Controls.TextField {
      id:search;objectName:"t3ModelSearch";Layout.fillWidth:true;implicitHeight:Style.space(40)
      text:root.query;onTextChanged:if(text!==root.query)root.query=text
      placeholderText:T3I18n.tr("Search models…");font.pixelSize:Style.font.caption;color:root.fg;placeholderTextColor:root.dim;selectByMouse:true;background:Item{}
      Keys.onDownPressed:root.moveResult(1)
      Keys.onUpPressed:root.moveResult(-1)
      Keys.onReturnPressed:root.selectResult(results.currentIndex)
      Keys.onEscapePressed:menu.close()
      Keys.onLeftPressed:function(event){event.accepted=!text.length;if(event.accepted)railView.forceActiveFocus()}
     }
    }
    Rectangle{Layout.fillWidth:true;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.09)}
    Text{visible:!!root.bridge.favoriteError || !!root.bridge.catalog.error;Layout.fillWidth:true;Layout.margins:Style.space(8);text:root.bridge.favoriteError?T3I18n.tr("Favorites could not be saved"):T3I18n.error(root.bridge.catalog.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    ListView{
     id:results;objectName:"t3ModelResults";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:root.rows;spacing:Style.space(2)
     T3Wheel {flickable:results}
     Controls.ScrollBar.vertical:Controls.ScrollBar{}
     Keys.onReturnPressed:root.selectResult(currentIndex)
     Keys.onLeftPressed:railView.forceActiveFocus()
     delegate:Rectangle{
      required property var modelData;required property int index
      width:results.width;height:Style.space(52);radius:Style.space(5)
      readonly property var entry:modelData.model
      color:results.currentIndex===index?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08):"transparent"
      Column{
       x:Style.space(10);anchors.verticalCenter:parent.verticalCenter;width:parent.width-Style.space(75);spacing:Style.space(3)
       RowLayout{width:parent.width
        Text{Layout.fillWidth:true;text:entry.displayName || entry.name;color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
        Text{visible:entry.badge==="new";text:T3I18n.tr("New");color:Color.accent;font.pixelSize:Style.font.caption}
       }
       Row{spacing:Style.space(4);visible:root.provider==="favorites" || !!root.query.trim() || !!entry.subProvider
        T3ProviderIcon{provider:entry;implicitWidth:Style.space(12);implicitHeight:implicitWidth;tint:root.dim}
        Text{width:Math.max(0,parent.parent.width-Style.space(16));text:(entry.providerName || entry.instanceId)+(entry.subProvider?" · "+entry.subProvider:"");color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
       }
      }
      T3Icon{anchors.right:star.left;anchors.verticalCenter:parent.verticalCenter;visible:entry.instanceId===root.selection.instanceId&&entry.model===root.selection.model;name:"check";implicitWidth:Style.space(14)}
      MouseArea{anchors{left:parent.left;top:parent.top;bottom:parent.bottom;right:star.left} hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onPositionChanged:results.currentIndex=index;onClicked:root.selectResult(index)}
      T3IconButton{
       id:star;objectName:"t3ModelFavorite_"+index
       anchors.right:parent.right;anchors.rightMargin:Style.space(4);anchors.verticalCenter:parent.verticalCenter
       name:modelData.favorite?"star-filled":"star";tint:modelData.favorite?T3Palette.waiting:root.dim
       enabled:root.bridge.catalog.favoritesReady===true&&!root.pending(entry)
       tooltipText:root.bridge.catalog.favoritesReady!==true?T3I18n.tr("Open T3 to sync favorites"):modelData.favorite?T3I18n.tr("Remove from favorites"):T3I18n.tr("Add to favorites")
       onClicked:root.bridge.toggleModelFavorite(entry.instanceId,entry.model)
      }
     }
     Text{anchors.centerIn:parent;width:parent.width-Style.space(24);horizontalAlignment:Text.AlignHCenter;wrapMode:Text.Wrap;color:root.dim;font.pixelSize:Style.font.caption;text:root.provider==="favorites"&&!root.query.trim()?T3I18n.tr("No favorites yet"):T3I18n.tr("No results");visible:!results.count}
    }
    Controls.Button{
     Layout.fillWidth:true;flat:true;visible:root.provider!=="favorites"&&!root.query.trim()&&root.models.some(function(m){return m.instanceId===root.provider&&m.isLegacy})
     text:T3I18n.tr("Legacy models")+" "+(root.legacyExpanded?"⌃":"⌄");onClicked:root.legacyExpanded=!root.legacyExpanded
    }
   }
  }
  }
 }
}
