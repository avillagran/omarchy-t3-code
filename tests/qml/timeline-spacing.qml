import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false
 function check(ok,msg){if(!ok){console.error("TIMELINE_SPACING_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return
   fixture={threads:[{key:"fixture/spacing",id:"spacing",environmentId:"fixture",title:"Native spacing"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","spacing")
   b.transcript=[{id:"u",role:"user",text:"Check the project and run the tests.",createdAt:1},{id:"a",role:"assistant",text:"I will inspect the project and run the checks.",rendered:"<p>I will inspect the project and run the checks.</p>",createdAt:2},{id:"cmd1",role:"activity",activity:{command:"rg --files src",active:false,count:1},createdAt:3},{id:"cmd2",role:"activity",activity:{command:"cargo test --locked",active:false,count:1},createdAt:4},{id:"done",role:"assistant",text:"All checks passed.\n\n```sh\ncargo test --locked\n```",rendered:'<p>All checks passed.</p><table width="100%"><tr><td>sh</td><td align="right"><a href="ot3-code-copy://0">Copy code</a></td></tr></table><pre><code>cargo test --locked\n</code></pre>',createdAt:5}];panel.setWindowMode("floating");panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return
   var user=c.itemAtIndex(0),comment=c.itemAtIndex(1),work=c.itemAtIndex(2),next=c.itemAtIndex(3),done=c.itemAtIndex(4);if(!user||!comment||!work||!next||!done)return
   check(work.bottomGap===0&&work.verticalInset===0,"native work rows have no vertical padding")
   var body=keys.findChild(work,"t3MessageBody");check(Math.abs(work.height-body.implicitHeight)<1,"no hidden per-command whitespace")
   check(Math.abs(next.y-work.y-work.height)<1,"consecutive commands are contiguous: "+JSON.stringify({y:work.y,h:work.height,next:next.y,spacing:c.spacing}))
   check(comment.bottomGap===Style.space(8)&&done.bottomGap===Style.space(16),"native commentary and terminal message gaps")
   check(!keys.findChild(comment,"copyChatMessage").visible,"commentary has no terminal metadata row")
   check(comment.height<=comment.textItem.implicitHeight+Style.space(12)+1,"commentary reserves no hidden footer")
   check(user.verticalInset===Style.space(12)&&user.bottomGap===Style.space(16),"user bubble padding and external gap are distinct")
   check(Math.abs(comment.textItem.mapToItem(comment,0,0).y-Style.space(2))<1,"assistant content starts without visible author heading")
   check(keys.findChild(done,"copyChatMessage").height===Style.space(20),"compact copy control remains available")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-timeline-spacing.png")});phase=2
  }else if(phase===2){if(!captured)return;panel.close();console.log("TIMELINE_SPACING_QML_PASS: contiguous commands, native row gaps, bubble inset and compact actions");Qt.quit()}
 }}
}
