import QtQuick
import Quickshell
import QtTest
import "UnderTest" as Test
ShellRoot{
 id:root;property int phase:0;property real savedWidth:0
 property var quotas:[{provider:"codex",label:"Codex",weekly:{availablePercent:75,windowSeconds:604800},onPace:true,expectedAvailable:50}]
 function check(ok,msg){if(!ok){console.error("T3_LAYOUT_QML_FAIL: "+msg);Qt.exit(1)}}
 Connections{target:Test.T3Backend;function onProvidersChanged(){if(root.phase>0 && Test.T3Backend.providers!==root.quotas)Test.T3Backend.providers=root.quotas}}
 TestCase{id:keys;when:false}
 Loader{id:loader;active:false;sourceComponent:Component{Test.T3Panel{bridge:Test.T3Backend}}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend;var p=loader.item
  if(phase===0){if(!b.viewStateLoaded)return;b.providers=root.quotas;loader.active=true;phase=1
  }else if(phase===1){p.open();phase=2
  }else if(phase===2){var usage=keys.findChild(p,"t3HeaderUsage");check(p.usageCollapsed&&!usage.visible,"usage hidden by default");check(!usage.showHeader,"usage has no section title");keys.mouseClick(keys.findChild(p,"t3UsageToggle"),14,14);phase=3
  }else if(phase===3){b.providers=root.quotas;var usage=keys.findChild(p,"t3HeaderUsage");check(!p.usageCollapsed&&usage.visible,"header button expands usage "+JSON.stringify({collapsed:p.usageCollapsed,visible:usage.visible,providers:usage.visibleProviders.length,opened:p.opened,expanded:b.viewState.usageExpanded}));var header=keys.findChild(p,"t3ChatHeader");check(usage.mapToItem(p,0,0).y>=header.mapToItem(p,0,header.height).y,"usage sits below project title");check(usage.mapToItem(p,0,usage.height).y<=p.conversationView.mapToItem(p,0,0).y,"usage sits above transcript");check(usage.width>p.sidebarView.width,"usage uses wider chat area");keys.findChild(p,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/omarchy-t3-wide-usage.png")});var grip=keys.findChild(p,"t3SidebarResizer");keys.mousePress(grip,grip.width/2,grip.height/2);keys.mouseMove(grip,grip.width/2+80,grip.height/2,20);keys.mouseRelease(grip,grip.width/2,grip.height/2);phase=4
  }else if(phase===4){check(p.sidebarWidth>320,"native pointer drag resizes sidebar");savedWidth=p.sidebarWidth;check(b.viewState.sidebarWidth===savedWidth,"drag saves logical width");p.close();loader.active=false;phase=5
  }else if(phase===5){loader.active=true;phase=6
  }else if(phase===6){p.open();phase=7
  }else if(phase===7){check(p.sidebarWidth===savedWidth,"sidebar width survives panel unload");check(!p.usageCollapsed,"explicit usage expansion survives unload");var button=keys.findChild(p,"t3UsageToggle");button.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=8
  }else if(phase===8){check(p.usageCollapsed,"keyboard toggles usage closed");p.resizeSidebar(999);check(p.sidebarWidth===600,"sidebar maximum clamp");p.resizeSidebar(100);check(p.sidebarWidth===260,"sidebar minimum clamp");p.close();console.log("T3_LAYOUT_QML_PASS: hidden usage, header placement, click/keyboard, pointer sidebar drag, persistence and limits");Qt.quit()}
 }}
}
