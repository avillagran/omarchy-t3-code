import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's usage page: spend and tokens for a period, per day and per model, from the
// agents' own transcripts on the selected environment.
Controls.Popup {
 id:root;objectName:"t3UsagePage"
 required property var bridge
 property string period:"week"
 property string metric:"cost"
 readonly property var view:bridge.usage || ({})
 readonly property var summary:view.data || null
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 function money(v){v=Number(v||0);return "$"+(v>=1000?Math.round(v).toLocaleString(Qt.locale(),"f",0):v.toFixed(2))}
 function count(v){v=Number(v||0);if(v<1000)return String(Math.round(v));if(v<1e6)return (v/1e3).toFixed(v<1e4?1:0).replace(/\.0$/,"")+"k";if(v<1e9)return (v/1e6).toFixed(v<1e7?1:0).replace(/\.0$/,"")+"M";return (v/1e9).toFixed(1).replace(/\.0$/,"")+"B"}
 function value(row){return metric==="cost"?Number(row.cost||0):Number(row.tokens||0)}
 function show(){open();load()}
 function load(){bridge.requestUsage(period)}
 function setPeriod(p){if(p===period)return;period=p;load()}
 modal:true;focus:true;padding:Style.space(18)
 x:Math.round((parent.width-width)/2);y:Math.round((parent.height-height)/2)
 width:Math.min(Style.space(760),parent.width-Style.space(40));height:Math.min(Style.space(620),parent.height-Style.space(40))
 background:Rectangle{radius:Style.space(12);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
 ColumnLayout{
  anchors.fill:parent;spacing:Style.space(12)
  RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Usage");color:root.fg;font.pixelSize:Style.font.title||18;font.weight:Font.DemiBold}
   Repeater{model:[["day","Day"],["week","Week"],["month","Month"],["quarter","Quarter"]]
    T3PickerButton{required property var modelData;objectName:"usagePeriod_"+modelData[0];implicitHeight:Style.space(28);primary:root.period===modelData[0];text:T3I18n.tr(modelData[1]);onClicked:root.setPeriod(modelData[0])}}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
  }
  Text{visible:!!root.view.error;Layout.fillWidth:true;text:T3I18n.error(root.view.error||"");color:Color.urgent;wrapMode:Text.Wrap}
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.busy&&!root.summary;active:visible}
  GridLayout{
   Layout.fillWidth:true;visible:!!root.summary;columns:4;columnSpacing:Style.space(10)
   Repeater{model:root.summary?[["Cost",root.money(root.summary.cost)],["Tokens",root.count(root.summary.tokens)],["Sessions",String(root.summary.sessions)],["Cache savings",root.money(root.summary.cacheSavings)]]:[]
    Rectangle{required property var modelData;Layout.fillWidth:true;implicitHeight:Style.space(62);radius:Style.space(8);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04)
     Column{anchors.left:parent.left;anchors.leftMargin:Style.space(12);anchors.verticalCenter:parent.verticalCenter;spacing:Style.space(2)
      Text{text:T3I18n.tr(modelData[0]);color:root.muted;font.pixelSize:Style.font.caption}
      Text{objectName:"usageKpi_"+modelData[0];text:modelData[1];color:root.fg;font.pixelSize:Style.font.body+4;font.weight:Font.DemiBold}}}}
  }
  RowLayout{Layout.fillWidth:true;visible:!!root.summary;spacing:Style.space(6)
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.metric==="cost"?"Cost per day":"Tokens per day");color:root.muted;font.pixelSize:Style.font.caption}
   T3PickerButton{objectName:"usageMetricCost";implicitHeight:Style.space(24);primary:root.metric==="cost";text:T3I18n.tr("Cost");onClicked:root.metric="cost"}
   T3PickerButton{objectName:"usageMetricTokens";implicitHeight:Style.space(24);primary:root.metric==="tokens";text:T3I18n.tr("Tokens");onClicked:root.metric="tokens"}
  }
  // One bar per day, scaled to the busiest day.
  Item{
   id:chart;objectName:"usageChart";Layout.fillWidth:true;Layout.preferredHeight:Style.space(150);visible:!!root.summary
   readonly property var days:root.summary?root.summary.days:[]
   readonly property real peak:Math.max.apply(null,[0].concat(days.map(function(d){return root.value(d)})))
   Row{anchors.fill:parent;spacing:Math.max(1,Math.min(Style.space(6),chart.width/Math.max(1,chart.days.length)*0.2))
    Repeater{model:chart.days
     Item{required property var modelData;width:(chart.width-parent.spacing*(chart.days.length-1))/Math.max(1,chart.days.length);height:chart.height
      Rectangle{anchors.bottom:parent.bottom;anchors.bottomMargin:Style.space(16);width:parent.width;radius:Math.min(Style.space(3),width/2);color:barHover.hovered?Qt.lighter(Color.accent,1.2):Color.accent;height:chart.peak>0?Math.max(root.value(modelData)>0?2:0,(parent.height-Style.space(16))*root.value(modelData)/chart.peak):0}
      Text{visible:chart.days.length<=14;anchors.bottom:parent.bottom;anchors.horizontalCenter:parent.horizontalCenter;text:String(modelData.day).slice(8);color:root.muted;font.pixelSize:Style.font.caption-1}
      HoverHandler{id:barHover}
      Controls.ToolTip.visible:barHover.hovered;Controls.ToolTip.text:modelData.day+" · "+root.money(modelData.cost)+" · "+root.count(modelData.tokens)+" "+T3I18n.tr("tokens")
     }}}
  }
  Text{visible:!!root.summary;text:T3I18n.tr("Models");color:root.muted;font.pixelSize:Style.font.caption}
  ListView{
   id:models;objectName:"usageModels";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;visible:!!root.summary;model:root.summary?root.summary.models:[]
   T3Wheel{flickable:models}
   delegate:RowLayout{required property var modelData;width:models.width;height:Style.space(28);spacing:Style.space(8)
    T3Icon{name:modelData.provider==="claude"?"claude":modelData.provider==="codex"?"openai":"acp";implicitWidth:Style.space(14);tint:root.muted}
    Text{Layout.fillWidth:true;text:modelData.model;color:root.fg;elide:Text.ElideMiddle;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
    Rectangle{Layout.preferredWidth:Style.space(160)*(root.summary&&root.summary.models.length?root.value(modelData)/Math.max(1e-9,root.value(root.summary.models.reduce(function(a,b){return root.value(a)>=root.value(b)?a:b}))):0);Layout.preferredHeight:Style.space(6);radius:3;color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.6)}
    Text{Layout.preferredWidth:Style.space(80);horizontalAlignment:Text.AlignRight;text:root.metric==="cost"?root.money(modelData.cost):root.count(modelData.tokens);color:root.fg;font.pixelSize:Style.font.caption}
   }
  }
  T3PickerButton{objectName:"usageRefreshPrices";visible:!!root.summary&&(root.summary.unpricedRecords>0||root.summary.pricing!=="fresh");primary:false;implicitHeight:Style.space(26);enabled:!root.view.busy;text:T3I18n.tr("Refresh prices");onClicked:root.bridge.requestUsage(root.period,true)}
  Text{Layout.fillWidth:true;visible:!!root.summary&&(root.summary.unpricedRecords>0||root.summary.pricing!=="fresh");text:(root.summary&&root.summary.unpricedRecords>0?T3I18n.tr("{count} records have no price.",{count:root.summary.unpricedRecords})+" ":"")+(root.summary&&root.summary.pricing==="unavailable"?T3I18n.tr("Model prices are unavailable."):"");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 }
}
