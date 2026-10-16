import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property var menu:null;property bool captured:false
 function check(ok,msg){if(!ok){console.error("THREAD_MENU_PANEL_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){var row=panel.sidebarView.itemAtIndex(0);if(!row)return;keys.mouseClick(row,100,40,Qt.RightButton);menu=keys.findChild(row,"t3ThreadContextMenu");phase=2
  }else if(phase===2){check(menu.visible,"menu visible in real panel");check(menu.width>100&&menu.height>100,"menu must render usable content, not just a line");check(keys.findChild(panel.sidebarView.itemAtIndex(0),"threadRename").width>100,"menu item receives usable text width");menu.contentItem.grabToImage(function(image){root.captured=image.saveToFile("/tmp/t3-thread-menu-panel.png")});phase=3
  }else if(phase===3){if(!captured)return;menu.close();panel.close();console.log("THREAD_MENU_PANEL_QML_PASS");Qt.quit()}
 }}
}
