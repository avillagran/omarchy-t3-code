import QtQuick
import QtQml.Models
import "../js/NativeKeybindings.js" as Native
import "../js/ShortcutRouting.js" as Routing
import "../js/QtKeybindings.js" as QtKeys
Item {
 id:root
 property bool active:false
 property var primary:Native.nativeKeys.DEFAULT_RESOLVED_KEYBINDINGS
 property var scripts:Native.nativeKeys.DEFAULT_RESOLVED_KEYBINDINGS
 property var context:({})
 property var localOverrides:[]
 property var supports:function(command){return false}
 signal triggered(string command)
 readonly property var claims:Routing.effective(primary,scripts,context)
 readonly property var routes:{
  var seen={},out=[]
  claims.forEach(function(r){if(Routing.reserved(r.shortcut)||localOverrides.indexOf(r.sequence)>=0||!r.sequences.length||!root.supports(r.command))return;var identity=Routing.qtIdentity(r.shortcut);if(seen[identity])return;seen[identity]=true;out.push(r)})
  return out
 }
 function claimsSequence(sequence){var shortcut=QtKeys.fromQtSequence(sequence);if(!shortcut||localOverrides.indexOf(sequence)>=0)return false;var key=Routing.qtIdentity(shortcut);return claims.some(function(r){return r.sequences.length&&Routing.qtIdentity(r.shortcut)===key&&!Routing.reserved(r.shortcut)})}
 Instantiator {model:root.routes
  delegate:Shortcut{
   required property var modelData
   objectName:"nativeShortcut_"+modelData.command
   sequences:modelData.sequences
   autoRepeat:["thread.previous","thread.next"].indexOf(modelData.command)>=0
   enabled:root.active
   context:Qt.WindowShortcut
   onActivated:root.triggered(modelData.command)
  }
 }
}
