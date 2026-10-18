import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
import qs.Ui

ColumnLayout {
  id: root
  required property var providers
  property var notices: []
  property var external: []
  property bool collapsed: true
  property bool showHeader: true
  signal refreshRequested()
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r,fg.g,fg.b,0.65)
  readonly property var visibleProviders: providers.filter(function(p){return root.windows(p).length>0})
  readonly property string issueText:notices.concat(external.map(function(e){return e.label+" · "+e.url})).join("\n")
  function windows(p){if(p.windows)return p.windows;return [p.weekly,p.shortWindow].filter(function(w){return !!w}).map(function(w,i){return Object.assign({label:i===0?"Weekly":"Session",segments:[],expectedAvailable:p.expectedAvailable},w)})}
  visible: (visibleProviders.length>0 || issueText.length>0) && (showHeader || !collapsed)
  spacing: Style.space(3)
  RowLayout {
    Layout.fillWidth:true
    visible:root.showHeader
    Text{Layout.fillWidth:true;text:T3I18n.tr("Provider usage");color:root.dim;font.pixelSize:Style.font.caption;font.bold:true}
    T3IconButton{visible:!!root.issueText;glyph:"!";tint:Color.urgent;tooltipText:root.issueText}
    T3IconButton{visible:!root.collapsed;name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:root.refreshRequested()}
    T3IconButton{name:root.collapsed?"chevron-down":"chevron-up";tooltipText:T3I18n.tr("Toggle usage");onClicked:root.collapsed=!root.collapsed}
  }
  // A failed-only observation occupies one compact line, with details on hover.
  RowLayout {
    visible:!root.showHeader && !root.collapsed && !root.visibleProviders.length
    Layout.fillWidth:true
    Item{Layout.fillWidth:true}
    T3IconButton{objectName:"quotaUnavailable";implicitWidth:Style.space(20);visible:!!root.issueText;glyph:"!";tint:Color.urgent;tooltipText:root.issueText}
    T3IconButton{implicitWidth:Style.space(20);name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:root.refreshRequested()}
  }
  Flickable {
    id: scroll
    visible:!root.collapsed && root.visibleProviders.length>0
    Layout.fillWidth:true
    Layout.preferredHeight:visible?Math.min(content.height,Style.space(200)):0
    contentHeight:content.height;contentWidth:width;clip:true
    boundsBehavior:Flickable.StopAtBounds
    T3Wheel{flickable:scroll}
    Column {
      id:content;width:scroll.width;spacing:Style.space(3)
      Repeater {
        model:root.visibleProviders
        Rectangle {
          id:group
          required property var modelData
          required property int index
          property bool expanded:false
          width:content.width;height:groupContent.implicitHeight+Style.space(8)
          color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025)
          radius:Style.space(4);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.09)
          ColumnLayout {
            id:groupContent
            anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(4)
            spacing:Style.space(3)
            RowLayout {
              Layout.fillWidth:true;spacing:Style.space(6)
              Text{Layout.preferredWidth:Style.space(92);text:group.modelData.label+((group.modelData.accounts || []).length>1?" · "+group.modelData.accounts.length:"");color:root.fg;font.bold:true;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}
              ColumnLayout {
                Layout.fillWidth:true;spacing:Style.space(2)
                Repeater {
                  model:root.windows(group.modelData)
                  RowLayout {
                    id:windowRow
                    required property var modelData
                    objectName:"quotaWindow_"+group.modelData.provider+"_"+(modelData.id || modelData.label)
                    Layout.fillWidth:true;spacing:Style.space(5)
                    Text{Layout.preferredWidth:Style.space(72);text:windowRow.modelData.label;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight}
                    Row {
                      id:segments
                      Layout.fillWidth:true;height:Style.space(6);spacing:Math.min(Style.space(2),width/(Math.max(1,items.length)*4))
                      readonly property var items:windowRow.modelData.segments && windowRow.modelData.segments.length?windowRow.modelData.segments:[{availablePercent:windowRow.modelData.availablePercent,expectedAvailable:windowRow.modelData.expectedAvailable}]
                      Repeater {
                        model:segments.items
                        Rectangle {
                          id:segment
                          required property var modelData
                          width:Math.max(0,(segments.width-segments.spacing*(segments.items.length-1))/segments.items.length)
                          readonly property var expected: modelData?modelData.expectedAvailable:null
                          readonly property bool overBudget:!!modelData && typeof expected==="number" && isFinite(expected) && Number(modelData.availablePercent)<expected
                          height:segments.height;radius:2;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,modelData?0.12:0.035)
                          Rectangle{objectName:"quotaFill_"+group.modelData.provider+"_"+(windowRow.modelData.id || windowRow.modelData.label);visible:!!segment.modelData;width:segment.width*Math.max(0,Math.min(100,segment.modelData?segment.modelData.availablePercent:0))/100;height:segment.height;radius:2;color:segment.overBudget?Color.urgent:Color.accent}
                          Rectangle {
                            objectName:"quotaOptimal_"+group.modelData.provider+"_"+(windowRow.modelData.id || windowRow.modelData.label)
                            readonly property var expected:segment.modelData?segment.modelData.expectedAvailable:null
                            visible:typeof expected==="number" && isFinite(expected) && expected>=0
                            x:Math.max(0,Math.min(segment.width-width,segment.width*Number(expected || 0)/100-width/2))
                            y:-Style.space(2);width:Math.max(2,Style.space(1));height:segment.height+Style.space(4);radius:1;color:root.fg
                          }
                        }
                      }
                    }
                    Text{Layout.preferredWidth:Style.space(34);horizontalAlignment:Text.AlignRight;text:Math.round(windowRow.modelData.availablePercent)+"%";color:root.fg;font.pixelSize:Style.font.caption}
                    Text{Layout.preferredWidth:Style.space(112);text:windowRow.modelData.nextReset || "";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
                  }
                }
              }
              RowLayout {
                Layout.preferredWidth:Style.space(60);spacing:0
                T3IconButton{objectName:"quotaAccounts_"+group.modelData.provider;implicitWidth:Style.space(20);visible:(group.modelData.accounts || []).length>0;name:group.expanded?"chevron-up":"chevron-down";tooltipText:T3I18n.tr("Accounts");onClicked:group.expanded=!group.expanded}
                T3IconButton{objectName:group.index===0?"quotaWarning":"";implicitWidth:Style.space(20);opacity:group.index===0 && !root.showHeader && !!root.issueText?1:0;enabled:opacity>0;glyph:"!";tint:Color.urgent;tooltipText:root.issueText}
                T3IconButton{objectName:group.index===0?"quotaRefresh":"";implicitWidth:Style.space(20);opacity:group.index===0 && !root.showHeader?1:0;enabled:opacity>0;name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");onClicked:root.refreshRequested()}
              }
            }
            Repeater {
              model:group.expanded?(group.modelData.accounts || []):[]
              ColumnLayout {
                id:account
                required property var modelData
                Layout.fillWidth:true;spacing:Style.space(2)
                Text{Layout.fillWidth:true;text:account.modelData.label+(account.modelData.plan?" · "+account.modelData.plan:"");color:root.fg;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;wrapMode:Text.Wrap}
                Text{Layout.fillWidth:true;text:account.modelData.hosts;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;wrapMode:Text.Wrap}
                Repeater{model:account.modelData.windows;Text{required property var modelData;Layout.fillWidth:true;text:modelData.label+": "+modelData.availablePercent+"%"+(modelData.reset?" · "+modelData.reset:"");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;wrapMode:Text.Wrap}}
              }
            }
          }
        }
      }
    }
  }
}
