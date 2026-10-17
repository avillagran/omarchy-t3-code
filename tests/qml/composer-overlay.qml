import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("COMPOSER_OVERLAY_QML_FAIL: "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView,p=panel.composerView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"Overlay"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<40;i++)rows.push({id:"m"+i,role:"assistant",text:"Message "+i+"\n"+"Text scrolls behind the prompt surface.\n".repeat(3)});b.transcript=rows;panel.open();phase=1
  }else if(phase===1){if(b.draft.loading||panel.restoringConversation)return;b.patchDraft(b.selectedKey,{text:"Prompt stays above the history",loading:false});phase=2
  }else if(phase===2){var origin=c.mapToItem(p,0,0);check(origin.y+c.height>p.height,"history viewport extends behind prompt "+JSON.stringify({y:origin.y,height:c.height,prompt:p.height,footer:c.footerItem.height}));check(c.footerItem.height>=p.height,"trailing inset reserves prompt space");c.positionViewAtEnd();phase=3
  }else if(phase===3){var last=c.itemAtIndex(c.count-1);check(!!last,"last row materialized");var bottom=last.mapToItem(p,0,last.height).y;check(bottom<=1,"last message stays above prompt at live edge: "+bottom);panel.detachConversation();c.positionViewAtIndex(10,ListView.Beginning);phase=4
  }else if(phase===4){check(!c.atYEnd,"reading old history");var overlaps=false;for(var i=0;i<c.count;i++){var row=c.itemAtIndex(i);if(!row)continue;var top=row.mapToItem(p,0,0).y;if(top<p.height&&top+row.height>0)overlaps=true}check(overlaps,"history rows traverse beneath the prompt");keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3-composer-overlay.png")});phase=5
  }else if(phase===5){panel.close();console.log("COMPOSER_OVERLAY_QML_PASS: history under prompt, reserved end inset, live-edge legibility");Qt.quit()}
 }}
}
