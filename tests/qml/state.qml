import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
  id: root;property var fixtureState: null
  property var oldChat: []
  property var expectedChat: null
  property var expectedSidebar: null
  property int phase: 0
  function check(ok,msg){if(!ok){console.error("T3_STATE_QML_FAIL: "+msg);Qt.exit(1)}}
  function messages(prefix,count){var out=[];for(var i=0;i<count;i++)out.push({id:prefix+i,role:i%2?"assistant":"user",text:"Message "+i+"\n"+"Reading position must survive a new panel and refreshed messages.\n".repeat(1+i%6),createdAt:1000});return out}
  function samePosition(view,rows,p,label){var index=view.indexAt(1,view.contentY+1);var item=view.itemAtIndex(index);root.check(index>=0&&item,label+" visible anchor");root.check((rows[index].key||rows[index].id)===p.id,label+" anchor restored");root.check(Math.abs(view.contentY-item.y-p.offset)<3,label+" intra-row offset restored")}
  Loader {id: loader;active:false;sourceComponent:Component{Test.T3Panel{bridge:Test.T3Backend}}}
 Connections { target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixtureState)Test.T3Backend.state=root.fixtureState} function onQueryChanged(){if(root.fixtureState&&Test.T3Backend.query)Test.T3Backend.query=""} }
  Timer {
    interval:200;running:true;repeat:true
    onTriggered:{
      var b=Test.T3Backend;var panel=loader.item
      if(panel && [2,4,7,8,9,10].indexOf(phase)>=0 && panel.restoringConversation)return
      if(phase===0){if(!b.viewStateLoaded)return;
        check(b.viewState.quotaCollapsed === true,"preferences restored from Rust state file at startup");check(b.selectedKey === "fixture/restored","selection restored from disk at startup");
        b.viewState={positions:{"fixture/one":{id:"one-4",offset:2,atEnd:false}}};b.sessionPositions=({});var rows=[];for(var i=0;i<40;i++)rows.push({id:"thread"+i,key:"fixture/thread"+i,title:"Thread "+i,project:"Project",environmentId:"fixture",projectKey:"fixture/p",status:"idle"});
        b.state={threads:rows,projects:[],environments:[],working:0,pending:0};root.fixtureState=b.state;b.selectThread("fixture","one");root.oldChat=messages("one-",40);b.transcript=root.oldChat;loader.active=true;phase=1
      }else if(phase===1){panel.open();panel.usageCollapsed=true;b.rememberView({language:"en"});panel.settledExpanded=true;phase=2
      }else if(phase===2){check(panel.conversationView.atYEnd,"fresh session ignores stale disk conversation anchors and opens at end");var c=panel.conversationView;var s=panel.sidebarView;panel.detachConversation();c.positionViewAtIndex(12,ListView.Beginning);c.movementEnded();s.positionViewAtIndex(9,ListView.Beginning);phase=3
      }else if(phase===3){panel.conversationView.contentY+=7;panel.sidebarView.contentY+=5;panel.captureState();expectedChat=b.viewState.positions["fixture/one"];expectedSidebar=b.viewState.positions["@sidebar"];check(!!expectedChat&&!!expectedSidebar,"positions captured "+JSON.stringify({positions:b.viewState.positions,cVisible:panel.conversationView.visible,cHeight:panel.conversationView.height,cCount:panel.conversationView.count,cY:panel.conversationView.contentY,sVisible:panel.sidebarView.visible,sHeight:panel.sidebarView.height,sY:panel.sidebarView.contentY,sIndex:panel.sidebarView.indexAt(1,panel.sidebarView.contentY+1),sWidth:panel.sidebarView.width,sCount:panel.sidebarView.count,sRestoring:panel.restoringSidebar,restoring:panel.restoringConversation,key:b.selectedKey}));check(!expectedChat.atEnd,"reading old messages");
        b.messagesAboutToChange();root.oldChat=root.oldChat.concat(messages("new-",1));b.transcript=root.oldChat;b.messagesUpdated();phase=4
      }else if(phase===4){samePosition(panel.conversationView,b.transcript,expectedChat,"message refresh");panel.close();loader.active=false;phase=5
      }else if(phase===5){loader.active=true;phase=6
      }else if(phase===6){panel.open();phase=7
      }else if(phase===7){check(panel.usageCollapsed,"usage remains collapsed after unload");check(b.viewState.language==="en"&&panel.settledExpanded,"other preferences survive unload");check(b.selectedKey==="fixture/one","selected chat survives unload");samePosition(panel.conversationView,b.transcript,expectedChat,"reopen conversation");samePosition(panel.sidebarView,panel.activeRows,expectedSidebar,"reopen sidebar");b.selectThread("fixture","two");b.transcript=messages("two-",30);b.messagesUpdated();phase=8
      }else if(phase===8){check(panel.conversationView.atYEnd,"new chat opens at end");b.selectThread("fixture","one");b.transcript=root.oldChat;b.messagesUpdated();phase=9
      }else if(phase===9){samePosition(panel.conversationView,b.transcript,expectedChat,"return to previous chat");b.messagesAboutToChange();b.transcript=root.oldChat.slice(25);b.messagesUpdated();phase=10
      }else if(phase===10){check(panel.conversationView.atYEnd,"missing bounded-history anchor falls back to end");panel.close();console.log("T3_STATE_QML_PASS: unload, reopen, polling and per-chat anchors");Qt.quit()}
    }
  }
}
