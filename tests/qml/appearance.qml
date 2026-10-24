import QtQuick
import QtQuick.Controls as Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import "UnderTest/ui/qml" as Test
// A theme switch reaches the detached app without Omarchy's shell IPC, and
// tooltips use ot3's style: the theme's tooltip with tooltips.json overrides.
ShellRoot{
 id:root;property int phase:0
 readonly property string home:Quickshell.env("HOME")
 readonly property string config:Quickshell.env("XDG_CONFIG_HOME")
 function check(ok,msg){if(!ok){console.error("T3_APPEARANCE_QML_FAIL: "+msg);Qt.exit(1)}}
 function sh(script){shell.command=["sh","-c",script];shell.running=true}
 Process{id:shell}
 FloatingWindow{visible:true;implicitWidth:200;implicitHeight:120
  Item{id:anchor;width:40;height:20;y:80;Controls.ToolTip{id:tip;text:"Dictate"}}}
 Timer{interval:150;running:true;repeat:true;onTriggered:{
  var p=Test.T3Palette
  if(phase===0){if(String(Color.accent)!=="#111111"||p.pick("blue","")!=="#1111aa")return
   check(tip.themeBorderSpec!==undefined,"Controls.ToolTip comes from ot3's style");phase=1
  }else if(phase===1){if(tip.contentItem.radius!==0)return;check(String(tip.contentItem.borderSpec.color)==="#ff0000","custom tooltip border: "+tip.contentItem.borderSpec.color)
   // What omarchy-theme-set does: replace the whole theme folder, then write theme.name.
   var dir=root.home+"/.local/state/omarchy/current"
   root.sh("rm -rf '"+dir+"/theme' && mkdir -p '"+dir+"/next' && printf '%s\\n' 'accent = \"#222222\"' 'blue = \"#2222bb\"' > '"+dir+"/next/colors.toml' && mv '"+dir+"/next' '"+dir+"/theme' && echo second > '"+dir+"/theme.name'");phase=2
  }else if(phase===2){if(String(Color.accent)!=="#222222"||p.pick("blue","")!=="#2222bb")return
   root.sh("printf '%s' '{\"shape\":\"rounded\"}' > '"+root.config+"/omarchy/tooltips.json.tmp' && mv '"+root.config+"/omarchy/tooltips.json.tmp' '"+root.config+"/omarchy/tooltips.json'");phase=3
  }else if(phase===3){if(tip.contentItem.radius!==Style.space(8))return
   console.log("T3_APPEARANCE_QML_PASS: theme switch reloads Color and the palette; tooltips follow the style and tooltips.json");Qt.quit()}
 }}
}
