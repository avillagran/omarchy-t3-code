import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeBrowserPreferences.js" as Original
Column {
 id:root
 required property var bridge
 readonly property var client:bridge.clientSettings || ({})
 readonly property var values:client.data || ({})
 readonly property bool writable:!!values.revision&&!client.busy&&!client.error
 readonly property var rules:Original.nativeBrowser
 readonly property var viewport:values.browserDefaultViewport || ({_tag:"fill"})
 readonly property var presented:viewport._tag==="fill"?rules.RESPONSIVE_SEED_SIZE:viewport
 readonly property var presets:[{id:"fill",name:"Fill panel"},{id:"responsive",name:"Responsive"}].concat(rules.PREVIEW_VIEWPORT_PRESETS.map(function(p){return {id:p.id,name:p.label+" · "+p.detail}}))
 function save(key,value){if(!writable)return;var patch={};patch[key]=value;bridge.requestClientSettings(patch)}
 function selectViewport(id){if(id==="fill"){save("browserDefaultViewport",{_tag:"fill"});return}if(id==="responsive"){save("browserDefaultViewport",{_tag:"freeform",width:presented.width,height:presented.height});return}var p=rules.PREVIEW_VIEWPORT_PRESETS.find(function(p){return p.id===id});if(p)save("browserDefaultViewport",{_tag:"preset",presetId:p.id,width:p.width,height:p.height})}
 function commit(axis,text){var value=Number(text);if(!writable||!rules.isValidDimension(value))return;var next={width:presented.width,height:presented.height};next[axis]=value;if(next.width*next.height>rules.PREVIEW_VIEWPORT_MAX_AREA)return;if(viewport._tag!=="fill"&&next.width===viewport.width&&next.height===viewport.height)return;save("browserDefaultViewport",Object.assign({_tag:"freeform"},next))}
 function rotate(){if(writable&&viewport._tag!=="fill")save("browserDefaultViewport",rules.rotateViewport(viewport))}
 readonly property var fields:[
  {key:"browserDefaultZoomFactor",label:"Default browser zoom",description:"Page zoom applied to new browser tabs.",kind:"choice",resetValue:1,choices:rules.PREVIEW_ZOOM_LEVELS.map(function(z){return {id:z,name:Math.round(z*100)+"%"}})},
  {key:"browserDefaultAppearance",label:"Default browser appearance",description:"The color scheme pages are told to prefer. System follows your OS setting.",kind:"choice",resetValue:"system",choices:[{id:"system",name:"System"},{id:"light",name:"Light"},{id:"dark",name:"Dark"}]},
  {key:"browserRecordingShowKeyPresses",label:"Show key presses in recordings",description:"Show pressed keys and shortcuts in new recordings. Password fields are excluded.",kind:"bool"},
  {key:"browserRecordingShowMousePresses",label:"Show mouse presses in recordings",description:"Highlight mouse presses and held buttons in new recordings.",kind:"bool"},
  {key:"browserRecordingFrameRate",label:"Browser recording frame rate",description:"Maximum recording rate. 30 fps saves CPU and storage; 60 fps is smoother.",kind:"choice",resetValue:30,choices:[{id:30,name:"30 fps"},{id:60,name:"60 fps"}]},
  {key:"browserLinkTarget",label:"Open links in",description:"Where links in the chat and terminal open. Hold ⌘ or Ctrl while clicking a link to open it in your default browser either way.",kind:"choice",resetValue:"system",choices:[{id:"system",name:"Your default browser"},{id:"app",name:"T3 Code"}]},
  {key:"browserAutoShowFloatingPreview",label:"Auto-show floating preview",description:"Show the floating preview when an agent opens a browser or device unless the agent says otherwise.",kind:"bool",resetValue:true}
 ]
 T3PreferenceField{width:parent.width;divided:true;field:({key:"browserDefaultViewport",label:"Default browser viewport",description:"Tab size for you and agents. Fill fits the panel; other sizes show the device toolbar.",kind:"choice",value:root.rules.viewportSelectValue(root.viewport),resetValue:"fill",choices:root.presets});writable:root.writable;onChanged:function(v){root.selectViewport(v)}}
 RowLayout{visible:root.viewport._tag!=="fill";width:parent.width;spacing:Style.space(8)
  Item{Layout.fillWidth:true}
  Repeater{model:["width","height"]
   Controls.TextField{required property string modelData;objectName:"browserViewport_"+modelData;Layout.preferredWidth:Style.space(85);Layout.bottomMargin:Style.space(14);enabled:root.writable;text:String(root.presented[modelData]);Accessible.name:T3I18n.tr(modelData==="width"?"Width":"Height");validator:IntValidator{bottom:0;top:9999}selectByMouse:true;color:Color.popups.text;font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignHCenter;onEditingFinished:{root.commit(modelData,text);text=String(root.presented[modelData])}background:Rectangle{radius:Style.space(5);color:"transparent";border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.15)}}
  }
  T3IconButton{objectName:"browserViewportRotate";Layout.rightMargin:Style.space(16);Layout.bottomMargin:Style.space(14);name:"refresh-cw";tooltipText:T3I18n.tr("Rotate");enabled:root.writable;onClicked:root.rotate()}
 }
 Repeater{model:root.fields
  T3PreferenceField{required property var modelData;width:parent.width;divided:true;field:Object.assign({},modelData,{value:root.values[modelData.key]});writable:root.writable;onChanged:function(v){root.save(modelData.key,v)}}
 }
 Text{width:parent.width;visible:!!root.client.error;text:T3I18n.error(root.client.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
}
