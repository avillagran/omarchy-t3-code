import QtQuick
// Trusted frame around an agent-written QML preview. The component is placed
// in `stage` from C++ (in its own context); errors are listed below it.
Rectangle {
 id:root
 color:omarchy.background
 property string title:"QML errors"
 property var messages:[]
 property bool failed:false
 // Inherited by the component: Controls follow the Omarchy theme.
 palette{window:omarchy.background;windowText:omarchy.foreground;base:omarchy.surface;alternateBase:omarchy.background;text:omarchy.foreground;button:omarchy.surface;buttonText:omarchy.foreground;highlight:omarchy.accent;highlightedText:omarchy.background;placeholderText:omarchy.muted;toolTipBase:omarchy.surface;toolTipText:omarchy.foreground;link:omarchy.accent;mid:omarchy.muted;light:Qt.lighter(omarchy.surface,1.3);dark:Qt.darker(omarchy.background,1.3);brightText:omarchy.foreground}
 Item{id:stage;objectName:"stage";anchors{left:parent.left;right:parent.right;top:parent.top;bottom:panel.top}clip:true}
 Rectangle{
  id:panel;anchors{left:parent.left;right:parent.right;bottom:parent.bottom}
  visible:root.messages.length>0
  height:!visible?0:root.failed?root.height:Math.min(list.implicitHeight+24,root.height/2)
  color:Qt.rgba(omarchy.red.r,omarchy.red.g,omarchy.red.b,root.failed?0.08:0.14)
  Column{
   id:list;anchors{left:parent.left;right:parent.right;top:parent.top;margins:12}spacing:6
   Text{text:root.title;color:omarchy.red;font.bold:true;font.pixelSize:14}
   Repeater{model:root.messages;Text{required property var modelData;width:list.width;text:modelData;color:omarchy.foreground;font.family:"monospace";font.pixelSize:12;wrapMode:Text.Wrap;textFormat:Text.PlainText}}
  }
 }
}
