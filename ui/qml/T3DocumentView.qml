import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/MarkdownStyle.js" as MarkdownStyle
Item {
 id:root;objectName:"t3DocumentView"
 required property var bridge
 signal closed()
 property string sessionId:""
 property string fallbackTitle:""
 property bool inlineMode:false
 property bool chromeVisible:true
 property bool ownsWheel:!inlineMode
 property var keyFilter:null
 signal frameReady()
 readonly property real surfaceHeight:surface.height
 readonly property var doc:inlineMode?(bridge.inlineDocuments[sessionId] || {}):bridge.documentOpen
 property bool sourceMode:false
 property real zoom:1
 readonly property bool html:!!doc.data&&doc.data.html===true
 readonly property bool markdown:!!doc.data&&doc.data.markdown===true
 readonly property bool previewable:html||markdown
 onPreviewableChanged:{sourceMode=!previewable;Qt.callLater(resize)}
 function openLink(link){var path=String(link);if(!/^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(path)){var title=String(doc.data.title || "");path=title.slice(0,title.lastIndexOf("/")+1)+path}bridge.openReference(path)}
 function send(event){if(inlineMode&&!sessionId)return;if(sessionId)bridge.send({op:"document-input",requestId:sessionId,event:event});else bridge.documentInput(event)}
 function resize(){if(html&&!sourceMode)send({type:"resize",width:Math.round(surface.width),height:Math.round(surface.height)})}
 ColumnLayout{
  anchors.fill:parent;spacing:Style.space(8)
  RowLayout{
   visible:root.chromeVisible
   Layout.fillWidth:true
   T3Icon{name:"file";tint:Color.accent}
   Text{Layout.fillWidth:true;text:root.doc.data?root.doc.data.title:root.fallbackTitle;color:Color.popups.text;textFormat:Text.PlainText;elide:Text.ElideMiddle;font.pixelSize:Style.font.body}
   Controls.Button{text:T3I18n.tr(root.sourceMode?"Preview":"Source");objectName:"t3DocumentMode";visible:root.previewable&&!root.inlineMode;onClicked:{root.sourceMode=!root.sourceMode;resizeDelay.restart()} palette.button:Color.popups.background;palette.buttonText:Color.popups.text}
   T3IconButton{name:"minus";tooltipText:"−";visible:root.html&&!root.sourceMode&&!root.inlineMode;enabled:root.zoom>0.5;onClicked:{root.zoom=Math.max(0.5,root.zoom-0.25);root.send({type:"zoom",value:root.zoom})}}
   Controls.Button{text:Math.round(root.zoom*100)+"%";visible:root.html&&!root.sourceMode&&!root.inlineMode;onClicked:{root.zoom=1;root.send({type:"zoom",value:1})} palette.button:Color.popups.background;palette.buttonText:Color.popups.text}
   T3IconButton{name:"plus";tooltipText:"+";visible:root.html&&!root.sourceMode&&!root.inlineMode;enabled:root.zoom<2;onClicked:{root.zoom=Math.min(2,root.zoom+0.25);root.send({type:"zoom",value:root.zoom})}}
   T3IconButton{objectName:"t3DocumentClose";name:root.inlineMode?"maximize-2":"x";tooltipText:T3I18n.tr(root.inlineMode?"Preview":"Close");onClicked:root.closed()}
  }
  Text{Layout.fillWidth:true;visible:!!root.doc.error;text:T3I18n.error(root.doc.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:root.doc.busy || root.html&&!root.sourceMode&&!root.doc.error&&(!root.doc.frameUrl || root.doc.data.internalReview&&!root.doc.rendererReady);active:visible}
  Item{
   id:surface;Layout.fillWidth:true;Layout.fillHeight:true;clip:true
   onWidthChanged:resizeDelay.restart();onHeightChanged:resizeDelay.restart()
   Image{
    id:frame;objectName:"t3EmbeddedDocumentFrame";anchors.fill:parent;visible:(root.html&&!root.sourceMode)||!!root.doc.data&&!!root.doc.data.imageData
    source:root.doc.data&&root.doc.data.imageData?root.doc.data.imageData:root.doc.frameUrl || "";cache:false;fillMode:Image.PreserveAspectFit;asynchronous:true;retainWhileLoading:true
    function point(x,y){var w=paintedWidth || width,h=paintedHeight || height;return {x:Math.max(0,(x-(width-w)/2)*(root.doc.frameWidth || w)/w),y:Math.max(0,(y-(height-h)/2)*(root.doc.frameHeight || h)/h)}}
    activeFocusOnTab:true
    onStatusChanged:if(status===Image.Ready&&visible){root.frameReady();if(!root.inlineMode)forceActiveFocus()}
    Keys.onPressed:function(e){if(root.keyFilter&&root.keyFilter(e))return;if(e.key===Qt.Key_Escape){root.closed();e.accepted=true;return}root.send({type:"key",key:e.key,text:e.text,modifiers:e.modifiers});e.accepted=true}
    Keys.onReleased:function(e){root.send({type:"keyup",key:e.key,text:e.text,modifiers:e.modifiers});e.accepted=true}
    MouseArea{
     id:mouseInput
     anchors.fill:parent;enabled:root.html&&frame.paintedWidth>0&&frame.paintedHeight>0;acceptedButtons:Qt.LeftButton;hoverEnabled:true
     property var lastMove:({})
     function input(type,m){var p=frame.point(m.x,m.y);root.send({type:type,x:p.x,y:p.y,button:m.button,buttons:m.buttons,modifiers:m.modifiers})}
     onPressed:function(m){frame.forceActiveFocus();input("press",m)}
     onReleased:function(m){input("release",m)}
     onPositionChanged:function(m){if(pressed)input("move",m);else{var p=frame.point(m.x,m.y);lastMove={type:"move",x:p.x,y:p.y,button:0,buttons:0,modifiers:m.modifiers};if(!moveDelay.running)moveDelay.start()}}
     Timer{id:moveDelay;interval:40;onTriggered:root.send(mouseInput.lastMove)}
     onDoubleClicked:function(m){input("double",m)}
     onWheel:function(e){if(!root.ownsWheel&&!frame.activeFocus){e.accepted=false;return}var p=frame.point(e.x,e.y);root.send({type:"wheel",x:p.x,y:p.y,ax:e.angleDelta.x,ay:e.angleDelta.y,px:e.pixelDelta.x,py:e.pixelDelta.y,modifiers:e.modifiers});e.accepted=true}
    }
   }
   Controls.ScrollView{
    id:markdownView;anchors.fill:parent;visible:root.markdown&&!root.sourceMode;clip:true
    T3Wheel{objectName:"t3DocumentMarkdownWheel";flickable:markdownView.contentItem;policy:root.bridge.scrollPolicy}
    Controls.TextArea{
     objectName:"t3DocumentMarkdown";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.RichText
     text:root.markdown?MarkdownStyle.render(root.doc.data.rendered || "",Style.font.body,Style.spaceReal(1),String(Color.popups.text),String(Color.popups.text),String(Color.popups.background),String(Color.popups.text)):""
     color:Color.popups.text;selectionColor:Color.accent;font.pixelSize:Style.font.body
     onLinkActivated:function(link){root.openLink(link)}
     background:Rectangle{color:Color.popups.background}
    }
   }
   Controls.ScrollView{
    id:sourceView;anchors.fill:parent;visible:!(root.doc.data&&root.doc.data.imageData)&&(!root.previewable || root.sourceMode);clip:true
    T3Wheel{objectName:"t3DocumentSourceWheel";flickable:sourceView.contentItem;policy:root.bridge.scrollPolicy}
    Controls.TextArea{objectName:"t3DocumentSource";Keys.onPressed:function(e){if(e.modifiers===Qt.MetaModifier&&e.key===Qt.Key_A){selectAll();e.accepted=true}else if(e.modifiers===Qt.MetaModifier&&e.key===Qt.Key_C){copy();e.accepted=true}}text:root.doc.data&&!root.doc.data.internalReview?root.doc.data.contents:"";textFormat:TextEdit.PlainText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;color:Color.popups.text;font.family:"monospace";font.pixelSize:Style.font.body;background:Rectangle{color:Color.popups.background}}
   }
  }
 }
 Timer{id:resizeDelay;interval:160;onTriggered:root.resize()}
 onVisibleChanged:if(visible)resizeDelay.restart()
}
