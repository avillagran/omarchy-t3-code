import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property real anchorY:0
 function check(ok,msg){if(!ok){console.error("T3_HISTORY_FIND_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 function row(i){return {id:"m"+i,role:i%2?"assistant":"user",text:"Message "+i+"\n"+(i===20?"needle needle":i===25?"another needle":"a paragraph of text").repeat(8)}}
 Timer{interval:180;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={threads:[{key:"fixture/chat",id:"chat",environmentId:"fixture",title:"Chat"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","chat")
   var recent=[];for(var i=10;i<40;i++)recent.push(row(i));b.recentTranscript=recent;b.mergeTranscript();b.historyCursor="opaque";b.hasEarlier=true;panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;panel.jumpTimelineItem({id:"m20"});phase=2
  }else if(phase===2){
   if(panel.restoringConversation)return
   var index=b.transcript.findIndex(function(r){return r.id==="m20"}),item=c.itemAtIndex(index);check(!!item,"anchor delegate exists");anchorY=item.y-c.contentY
   b.historyOperation={requestId:"page",key:b.selectedKey,serial:b.requestedSerial,busy:true};var page=[];for(var j=0;j<12;j++)page.push(row(j))
   b.action({op:"history",requestId:"page",result:{timeline:page,historyCursor:"next",hasMoreHistory:true}});phase=3
  }else if(phase===3){
   if(panel.restoringConversation)return
   check(b.transcript.length===40&&b.transcript[0].id==="m0","older page prepends and overlap deduplicates")
   var index=b.transcript.findIndex(function(r){return r.id==="m20"});check(Math.abs(c.itemAtIndex(index).y-c.contentY-anchorY)<3,"prepend preserves visible row pixel position")
   b.recentTranscript=b.recentTranscript.map(function(r){return r.id==="m20"?Object.assign({},r,{text:"needle needle"}):r});b.messagesAboutToChange();b.mergeTranscript();b.messagesUpdated()
   check(b.transcript[0].id==="m0"&&b.historyCursor==="next","live refresh preserves loaded history and opaque cursor")
   panel.openFind();panel.findQuery="needle";phase=4
  }else if(phase===4){
   if(panel.restoringConversation)return
   check(panel.findMatches.length===10,"all occurrences in loaded messages found")
   check(panel.findMatches[panel.findIndex].id==="m20","find jumps to first message")
   var item=c.itemAtIndex(b.transcript.findIndex(function(r){return r.id==="m20"}));check(item&&item.textItem.selectedText.toLowerCase()==="needle","matching displayed text selected "+JSON.stringify({item:!!item,selected:item?item.textItem.selectedText:"",index:panel.findIndex}))
   panel.navigateFind(-1);check(panel.findIndex===9,"previous match wraps")
   panel.findQuery="no-such-value";phase=5
  }else if(phase===5){
   check(panel.findMatches.length===0&&panel.findIndex===-1,"empty search result")
   b.historyOperation={requestId:"stale",key:"fixture/other",serial:b.requestedSerial,busy:false};b.action({op:"history",requestId:"stale",result:{timeline:[row(-1)]}});check(b.transcript[0].id==="m0","late page for other thread ignored")
   b.resetHistory();check(!b.olderTranscript.length&&!b.hasEarlier,"thread reset releases history")
   panel.close();console.log("T3_HISTORY_FIND_QML_PASS: page prepend, overlap, viewport anchor, live retention, scoped results, search selection and wrap");Qt.quit()
  }
 }}
}
