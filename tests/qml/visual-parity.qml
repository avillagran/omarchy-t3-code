import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false
 function check(ok,msg){if(!ok){console.error("VISUAL_PARITY_QML_FAIL: "+msg);Qt.exit(1)}}
 function thread(id,extra){return Object.assign({id:id,key:"fixture/"+id,environmentId:"fixture",projectId:"p",projectKey:"fixture/p",project:"Project",projectMonogram:"PR",title:id,provider:"codex",branch:"main",status:"completed",updatedAt:Date.now()},extra || {})}
 function byId(list,id){for(var i=0;i<list.count;i++){var row=list.itemAtIndex(i);if(row&&row.thread.id===id)return row}return null}
 TestCase{id:keys;parent:panel.normalWindow.contentItem;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Test.T3ProjectIcon{id:lucideIcon;project:({key:"fixture/p",projectIcon:{kind:"lucide",name:"flask-conical",color:"green"}});projectKey:"fixture/p"}
 Test.T3ProjectIcon{id:emojiIcon;project:({projectIcon:{kind:"emoji",emoji:"🚀"}})}
 Test.T3ProjectIcon{id:monoIcon;project:({projectIcon:{kind:"monogram",text:"AB",color:"blue"}})}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return
   b.rememberView({threadRowSize:"large"})
   fixture={threads:[thread("selected",{pinned:true}),thread("read"),thread("working",{status:"running",working:true}),thread("snoozed",{snoozed:true,snoozedUntil:new Date(Date.now()+7200000).toISOString()}),thread("settled",{settled:true})],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"Project",projectIcon:{kind:"emoji",emoji:"🚀"}}],environments:[]};b.state=fixture;b.rememberView({sidebarCompact:true,workingExpanded:true,snoozedExpanded:true,settledExpanded:true,windowWidth:1400,windowHeight:900});b.selectThread("fixture","selected");b.patchDraft("fixture/selected",{text:"Current draft"});b.patchDraft("fixture/working",{text:"Other draft"})
   b.transcript=[{id:"short",role:"user",text:"Short question"},{id:"long",role:"user",text:"A longer question that wraps naturally within the native eighty percent bubble. ".repeat(8)},{id:"rich",role:"assistant",text:"# Heading\n\nParagraph\n\n- Item\n\n```sh\necho hello\n```",rendered:'<h1>Heading</h1><p>Paragraph with <a href="https://github.com/a/b">GitHub</a> and <code>inline code</code>.</p><ul><li>First item</li><li>Second item</li></ul><blockquote><p>A quotation</p></blockquote><table><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody><tr><td>Example</td><td>42</td></tr></tbody></table><table width="100%"><tr><td>sh</td><td align="right"><a href="ot3-code-copy://0">Copy code</a></td></tr></table><pre><code>echo hello\n</code></pre>'}];panel.setWindowMode("floating");panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation||!lucideIcon.svg)return
   var selected=byId(keys.findChild(panel,"pinnedThreadList"),"selected"),read=byId(panel.sidebarView,"read"),working=byId(keys.findChild(panel,"workingThreadList"),"working"),sleep=byId(keys.findChild(panel,"snoozedThreadList"),"snoozed"),settled=byId(keys.findChild(panel,"settledThreadList"),"settled")
   if(!selected||!read||!working||!sleep||!settled)return
   check(selected.height===Style.space(82)&&working.height===Style.space(82),"active/working retain native full cards")
   check(sleep.height===Style.space(36)&&settled.height===Style.space(36),"snoozed/settled use native slim rows")
   check(!selected.showDraft&&working.showDraft,"draft marker only on other threads")
   check(!read.stateLabel&&panel.status(thread("unread",{pending:true,reason:"review"}))===Test.T3I18n.tr("Done"),"Done only denotes unread completion")
   check(panel.pinnedRows[0].id==="selected"&&!keys.findChild(selected,"threadPinIndicator"),"pinned threads sit in the Pinned group, with no pin icon")
   check(!keys.findChild(settled,"threadCardTitle").visible&&keys.findChild(settled,"threadProjectOrTitle").text==="settled","slim row has one title")
   check(emojiIcon.label==="🚀"&&monoIcon.label==="AB"&&lucideIcon.svg.indexOf("<svg")>=0,"native project overrides")
   c.positionViewAtBeginning();phase=2
  }else if(phase===2){if(panel.restoringConversation)return
   var short=c.itemAtIndex(0),long=c.itemAtIndex(1);if(!short||!long)return
   var small=keys.findChild(short,"t3MessageBubble"),large=keys.findChild(long,"t3MessageBubble"),actions=keys.findChild(short,"t3MessageActions")
   check(short.laneWidth<=panel.chatContentMaxWidth&&panel.composerView.width<=panel.chatContentMaxWidth,"shared comfortable maximum width");var lane=keys.findChild(short,"t3MessageLane");check(Math.abs(lane.mapToItem(panel.composerView,0,0).x)<3,"composer and timeline lanes align")
   check(small.width<short.laneWidth*0.8&&large.width<=long.laneWidth*0.8+1,"user bubbles shrink and cap at 80 percent")
   check(actions.mapToItem(small,0,0).y>=small.height,"metadata outside user background")
   c.positionViewAtEnd();phase=3
  }else if(phase===3){var row=c.itemAtIndex(2);if(!row)return
   check(panel.renderMessage(b.transcript[2]).indexOf("line-height:150%")>=0&&panel.renderMessage(b.transcript[2]).indexOf('class="ot3-code-toolbar"')>=0,"explicit typography and compact code toolbar")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-visual-parity.png")});phase=4
  }else if(phase===4){if(!captured)return;b.search("read");panel.searchInput.forceActiveFocus();phase=5
  }else if(phase===5){keys.keyClick(Qt.Key_Escape);check(panel.opened&&!b.query,"Escape clears search without closing chat");b.search("working");phase=6
  }else if(phase===6){check(panel.searchInput.text==="working","search text binding survives clear");keys.mouseClick(keys.findChild(panel,"clearThreadSearch"),10,10);check(!b.query&&panel.opened,"clear search button preserves panel");panel.close();console.log("VISUAL_PARITY_QML_PASS: lane, bubbles, external metadata, Markdown, card/slim rows, read status, drafts, pins, project overrides and search clear");Qt.quit()}
 }}
}
