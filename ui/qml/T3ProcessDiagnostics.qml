import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's process diagnostics: what the server is running for agents and tools, with
// CPU and memory. A stuck process can be interrupted or killed.
Controls.Popup {
 id:root;objectName:"t3ProcessDiagnostics"
 required property var bridge
 readonly property var view:bridge.processDiagnostics || ({})
 readonly property var report:view.report || null
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 property string confirmKill:""
 function size(b){b=Number(b||0);if(b<1048576)return Math.round(b/1024)+" KB";if(b<1073741824)return (b/1048576).toFixed(0)+" MB";return (b/1073741824).toFixed(1)+" GB"}
 function show(){confirmKill="";open();bridge.processDiagnosticsAction({action:"read"})}
 Timer{interval:3000;repeat:true;running:root.opened&&!root.view.busy;onTriggered:root.bridge.processDiagnosticsAction({action:"read"})}
 modal:true;focus:true;padding:Style.space(18)
 x:Math.round((parent.width-width)/2);y:Math.round((parent.height-height)/2)
 width:Math.min(Style.space(760),parent.width-Style.space(40));height:Math.min(Style.space(560),parent.height-Style.space(40))
 background:Rectangle{radius:Style.space(12);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
 ColumnLayout{
  anchors.fill:parent;spacing:Style.space(10)
  RowLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Processes");color:root.fg;font.pixelSize:Style.font.body+4;font.weight:Font.DemiBold}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
  }
  Text{visible:!!root.report;Layout.fillWidth:true;color:root.muted;font.pixelSize:Style.font.caption;text:root.report?T3I18n.tr("{count} processes · {cpu}% CPU · {memory}",{count:root.report.count,cpu:Number(root.report.cpu||0).toFixed(1),memory:root.size(root.report.rss)}):""}
  // CPU (line) and memory (bars) over the last half hour.
  Canvas{
   id:history;objectName:"processHistory";Layout.fillWidth:true;Layout.preferredHeight:Style.space(70);visible:!!root.report&&(root.report.history||[]).length>1
   property var points:root.report?root.report.history||[]:[]
   onPointsChanged:requestPaint()
   onPaint:{var c=getContext("2d");c.reset();var n=points.length;if(n<2)return;var maxCpu=Math.max(10,Math.max.apply(null,points.map(function(p){return p.cpu||0}))),maxRss=Math.max(1,Math.max.apply(null,points.map(function(p){return p.rss||0}))),w=width/n
    c.fillStyle=Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.18);for(var i=0;i<n;i++){var h=(height-4)*(points[i].rss||0)/maxRss;c.fillRect(i*w+1,height-h,Math.max(1,w-2),h)}
    c.strokeStyle=Color.accent;c.lineWidth=1.5;c.beginPath();for(var j=0;j<n;j++){var x=j*w+w/2,y=height-2-(height-6)*(points[j].cpu||0)/maxCpu;if(j===0)c.moveTo(x,y);else c.lineTo(x,y)}c.stroke()}
  }
  Text{visible:history.visible;Layout.fillWidth:true;text:T3I18n.tr("Last 30 minutes: CPU (line) and memory (bars)");color:root.muted;font.pixelSize:Style.font.caption}
  Text{visible:!!root.view.error||!!(root.report&&root.report.error);Layout.fillWidth:true;wrapMode:Text.Wrap;color:Color.urgent;font.pixelSize:Style.font.caption;text:root.view.error?T3I18n.error(root.view.error):root.report&&root.report.error?root.report.error:""}
  ListView{
   id:list;objectName:"processList";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:root.report?root.report.processes:[]
   T3Wheel{flickable:list}
   delegate:RowLayout{
    required property var modelData;width:list.width;height:Style.space(30);spacing:Style.space(8)
    Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(12)*Math.min(4,modelData.depth||0);text:modelData.command;color:root.fg;font.family:"monospace";font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
    Text{Layout.preferredWidth:Style.space(52);horizontalAlignment:Text.AlignRight;text:Number(modelData.cpu||0).toFixed(1)+"%";color:root.muted;font.pixelSize:Style.font.caption}
    Text{Layout.preferredWidth:Style.space(64);horizontalAlignment:Text.AlignRight;text:root.size(modelData.rss);color:root.muted;font.pixelSize:Style.font.caption}
    Text{Layout.preferredWidth:Style.space(64);horizontalAlignment:Text.AlignRight;text:String(modelData.elapsed||"");color:root.muted;font.pixelSize:Style.font.caption}
    T3IconButton{name:"square";tooltipText:T3I18n.tr("Interrupt");enabled:!root.view.busy;onClicked:root.bridge.processDiagnosticsAction({action:"signal",signal:"SIGINT",pid:modelData.pid,startTimeMs:modelData.startTimeMs})}
    T3IconButton{name:"trash-2";tint:root.confirmKill===String(modelData.pid)?Color.urgent:root.muted;tooltipText:T3I18n.tr(root.confirmKill===String(modelData.pid)?"Click again to kill":"Kill");enabled:!root.view.busy;onClicked:{if(root.confirmKill!==String(modelData.pid)){root.confirmKill=String(modelData.pid);return}root.confirmKill="";root.bridge.processDiagnosticsAction({action:"signal",signal:"SIGKILL",pid:modelData.pid,startTimeMs:modelData.startTimeMs})}}
   }
  }
 }
}
