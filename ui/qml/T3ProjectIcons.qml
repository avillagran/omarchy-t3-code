pragma Singleton
import QtQuick 6.11
import Quickshell
import Quickshell.Io
Singleton {
 id:root
 property var icons:({})
 FileView {
  path:Qt.resolvedUrl("../assets/lucide-project-icons.json").toString().replace(/^file:\/\//,"")
  printErrors:false
  onLoaded:{try{root.icons=JSON.parse(text())}catch(e){root.icons=({})}}
 }
}
