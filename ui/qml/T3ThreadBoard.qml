import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/SidebarModel.js" as Sidebar

Rectangle {
  id:root
  objectName:"t3ThreadBoard"
  required property var bridge
  property var threads:[]
  signal closed()
  signal threadRequested(var thread)
  readonly property color fg:Color.popups.text
  readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.55)
  color:Color.popups.background
  activeFocusOnTab:true
  Keys.onEscapePressed:closed()
  function bucket(t){
    var state=Sidebar.state(t)
    if(state==="waiting"||state==="failed")return "attention"
    if(state==="working")return "working"
    if(t.settled||state==="done")return "done"
    return "pending"
  }
  function rows(key){return threads.filter(function(t){return t.relationshipToParent!=="subagent"&&root.bucket(t)===key}).slice().sort(function(a,b){return (b.repliedAt||b.updatedAt||0)-(a.repliedAt||a.updatedAt||0)})}
  ColumnLayout {
    anchors.fill:parent;anchors.margins:Style.space(20);spacing:Style.space(16)
    RowLayout {
      Layout.fillWidth:true
      Text{text:T3I18n.tr("Task board");color:root.fg;font.pixelSize:Style.font.body; font.weight:Font.DemiBold}
      Item{Layout.fillWidth:true}
      T3PickerButton{objectName:"t3FocusBoardConversation";text:T3I18n.tr("Conversation");primary:false;onClicked:root.closed()}
    }
    GridLayout {
      id:grid;Layout.fillWidth:true;Layout.fillHeight:true;columns:width>Style.space(900)?4:width>Style.space(480)?2:1
      columnSpacing:Style.space(14);rowSpacing:Style.space(14)
      Repeater {
        model:[{key:"pending",label:"Pending",tint:root.dim},{key:"working",label:"Working",tint:T3Palette.working},{key:"attention",label:"Waiting",tint:T3Palette.waiting},{key:"done",label:"Done",tint:T3Palette.done}]
        ColumnLayout {
          id:column
          required property var modelData
          readonly property var rows:root.rows(modelData.key)
          objectName:"t3BoardColumn_"+modelData.key
          Layout.fillWidth:true;Layout.fillHeight:true;Layout.minimumHeight:Style.space(110)
          spacing:Style.space(10)
          RowLayout {
            Layout.fillWidth:true;spacing:Style.space(6)
            Rectangle{width:Style.space(6);height:width;radius:width/2;color:column.modelData.tint}
            Text{text:T3I18n.tr(column.modelData.label);color:root.dim;font.pixelSize:Style.font.caption}
            Text{text:String(column.rows.length);color:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption}
          }
          ListView {
            id:list;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;spacing:Style.space(8);reuseItems:true
            model:column.rows
            T3Wheel{flickable:list}
            Controls.ScrollBar.vertical:Controls.ScrollBar{}
            Text{anchors.centerIn:parent;visible:!list.count;text:"—";color:root.dim;font.pixelSize:Style.font.caption}
            delegate:Controls.AbstractButton {
              id:card
              required property var modelData
              objectName:"t3BoardThread_"+modelData.id
              width:ListView.view.width;height:Style.space(90);padding:Style.space(12)
              Accessible.name:[modelData.title,modelData.project,modelData.environmentName].filter(Boolean).join(" · ")
              onClicked:root.threadRequested(modelData)
              background:Rectangle{radius:Style.space(10);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,card.hovered?0.065:0.025);border.width:1;border.color:card.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)}
              contentItem:ColumnLayout {
                spacing:Style.space(7)
                Text{Layout.fillWidth:true;text:card.modelData.title || T3I18n.tr("New chat");color:root.fg;font.pixelSize:Style.font.body;maximumLineCount:2;wrapMode:Text.Wrap;elide:Text.ElideRight;textFormat:Text.PlainText}
                RowLayout {
                  Layout.fillWidth:true
                  Text{Layout.fillWidth:true;text:card.modelData.project || "";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
                  Text{text:card.modelData.environmentName || ((root.bridge.state.environments || []).find(function(e){return e.id===card.modelData.environmentId}) || {}).name || card.modelData.environmentId;color:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption-Style.space(1);textFormat:Text.PlainText;Layout.maximumWidth:parent.width*0.45;elide:Text.ElideRight}
                }
              }
            }
          }
        }
      }
    }
  }
}
