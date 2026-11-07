import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
ShellRoot {
    id: root
    property int phase: 0
    property bool complete: false
    property real fakeNow: Date.parse("2026-11-07T02:00:00.000Z")
    property var fakeDate: (function(){function FakeDate(value){return new Date(value===undefined?root.fakeNow:value)}FakeDate.now=function(){return root.fakeNow};return FakeDate})()
    readonly property var panel: panelLoader.item
    Loader { id: panelLoader; active:false; sourceComponent:Client.T3Panel { bridge:Client.T3Backend } }
    Component.onCompleted:{check(Client.T3ThreadClock.owner.store===null,"clock service is installed before the first mounted panel");Client.T3ThreadClock.owner.clockDate=root.fakeDate;panelLoader.active=true}
    function check(ok,message){if(!ok){console.error("THREAD_ROW_ACTIVITY_QML_FAIL "+message);Qt.exit(1)}}
    function row(id){var list=panel.sidebarView;for(var index=0;index<list.count;index++){var item=list.itemAtIndex(index);if(item&&item.thread&&item.thread.id===id)return item}return null}
    function clock(user,completion,settled){return {updatedAt:"2026-11-07T02:00:00.000Z",latestUserMessageAt:user,settledAt:settled||null,latestRun:{requestedAt:null,startedAt:null,completedAt:completion}}}
    Timer {
        interval:100;repeat:true;running:!root.complete
        onTriggered:{
            var bridge=Client.T3Backend
            if(phase===0){
                if(!bridge.viewStateLoaded)return
                check(panel.rowMinute==="2026-11-07T02:00"&&Client.T3ThreadClock.owner.timer.running,"actual panel subscribes to native minute clock")
                bridge.state={threads:[
                    {id:"a",key:"fixture/a",environmentId:"fixture",title:"Reply order first",status:"completed",repliedAt:500,inboxAt:10,activeAnchorAt:10,activityClock:root.clock("2026-11-07T01:50:00.000Z","2026-11-07T01:00:00.000Z")},
                    {id:"b",key:"fixture/b",environmentId:"fixture",title:"Activity label newest",status:"completed",repliedAt:100,inboxAt:20,activeAnchorAt:20,activityClock:root.clock(null,"2026-11-07T01:59:00.000Z")}
                ],projects:[],environments:[{id:"fixture",name:"Fixture",state:"ready"}],working:0,pending:0}
                panel.open();phase=1
            }else if(phase===1){
                var first=root.row("a"),second=root.row("b");if(!first||!second)return
                check(panel.orderByReply&&panel.activeRows.map(function(thread){return thread.id}).join() === "a,b","existing LastReply comparator still orders actual sidebar rows")
                check(first.timeLabel==="10m"&&second.timeLabel==="1m","actual row labels use native activity independently of reply order")
                bridge.rememberView({threadOrder:"t3"});phase=2
            }else if(phase===2){
                var first=root.row("a"),second=root.row("b");if(!first||!second)return
                check(!panel.orderByReply&&first.timeLabel==="10m"&&second.timeLabel==="1m","changing sorting mode does not change either activity label")
                root.fakeNow=Date.parse("2026-11-07T02:01:00.000Z");phase=3
            }else if(phase===3){
                var first=root.row("a"),second=root.row("b");if(!first||!second)return
                if(panel.rowMinute!=="2026-11-07T02:01")return
                check(first.timeLabel==="11m"&&second.timeLabel==="2m","actual row bindings refresh from real shared minute timer callback")
                check(panel.threadTime({settled:true,activityClock:root.clock(null,"2026-11-07T01:59:00.000Z","2026-11-07T01:30:00.000Z")})==="31m","settled label retains explicit native settlement clock")
                panel.close();panelLoader.active=false;phase=4
            }else if(phase===4){
                check(!Client.T3ThreadClock.owner.timer.running&&Client.T3ThreadClock.owner.pendingCallback===null,"actual panel destruction unsubscribes and stops shared clock")
                root.complete=true
                console.log("THREAD_ROW_ACTIVITY_QML_PASS: actual panel/sidebar delegates retain reply sorting, independent native labels, real minute timer updates, settled clock and destruction teardown")
                Qt.callLater(Qt.quit)
            }
        }
    }
    Timer{interval:85000;running:true;onTriggered:{console.error("THREAD_ROW_ACTIVITY_QML_FAIL deadline phase="+root.phase);Qt.exit(1)}}
}
