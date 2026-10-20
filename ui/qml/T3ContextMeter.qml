import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons

// How full the thread's context window is, from T3's ContextWindowMeter.
Item {
 id:root;objectName:"t3ContextMeter"
 property var usage:null
 readonly property real percent:usage&&usage.usedPercentage!==null&&usage.usedPercentage!==undefined?usage.usedPercentage:-1
 readonly property color tone:percent>=90?Color.urgent:percent>=75?Qt.color("#d9a33a"):Color.accent
 visible:!!usage
 implicitWidth:Style.space(22);implicitHeight:Style.space(22)
 Accessible.role:Accessible.ProgressBar;Accessible.name:T3I18n.tr("Context window")
 function tokens(v){if(v===null||v===undefined||!isFinite(v))return "0";if(v<1000)return String(Math.round(v));if(v<10000)return (v/1000).toFixed(1).replace(/\.0$/,"")+"k";if(v<999500)return Math.round(v/1000)+"k";return (v/1000000).toFixed(1).replace(/\.0$/,"")+"m"}
 readonly property string summary:{
  if(!usage)return ""
  var lines=[T3I18n.tr("Context window")]
  lines.push(usage.maxTokens?T3I18n.tr("{used} of {max} tokens · {percent}%",{used:tokens(usage.usedTokens),max:tokens(usage.maxTokens),percent:Math.round(percent)}):T3I18n.tr("{used} tokens",{used:tokens(usage.usedTokens)}))
  if(usage.totalProcessedTokens)lines.push(T3I18n.tr("Total processed: {count}",{count:tokens(usage.totalProcessedTokens)}))
  if(usage.cost!==null&&usage.cost!==undefined)lines.push(T3I18n.tr("Cost: ${cost}",{cost:Number(usage.cost).toFixed(2)}))
  if(usage.compactsAutomatically)lines.push(T3I18n.tr("Compacts automatically when full."))
  return lines.join("\n")
 }
 Canvas {
  id:ring;anchors.fill:parent;anchors.margins:Style.space(3)
  property real value:Math.max(0,root.percent)/100
  property color fill:root.tone
  onValueChanged:requestPaint();onFillChanged:requestPaint()
  onPaint:{var c=getContext("2d"),r=Math.min(width,height)/2-1.5,cx=width/2,cy=height/2;c.reset();c.lineWidth=2.5;c.strokeStyle=Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.18);c.beginPath();c.arc(cx,cy,r,0,2*Math.PI);c.stroke();if(value>0){c.strokeStyle=fill;c.beginPath();c.arc(cx,cy,r,-Math.PI/2,-Math.PI/2+2*Math.PI*value);c.stroke()}}
 }
 Text{anchors.centerIn:parent;visible:root.percent<0;text:"·";color:Color.popups.text;opacity:0.6;font.pixelSize:Style.font.caption}
 HoverHandler{id:hover}
 Controls.ToolTip.visible:hover.hovered;Controls.ToolTip.delay:300;Controls.ToolTip.text:root.summary
}
