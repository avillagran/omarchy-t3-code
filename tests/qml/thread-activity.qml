import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/NativeThreadActivity.js" as Native
import "UnderTest/ui/js/NativeThreadClock.js" as Clock
import "UnderTest/ui/js/NativeThreadRelativeTime.js" as Relative
ShellRoot {
    id: root
    property real fakeNow: Date.parse("2026-11-07T12:34:59.990Z")
    property var fakeDate: (function(){function FakeDate(value){return new Date(value===undefined?root.fakeNow:value)}FakeDate.now=function(){return root.fakeNow};return FakeDate})()
    property string observedMinute: "2026-11-07T12:34"
    property int notified: 0
    property var unsubscribeA: null
    property var unsubscribeB: null
    property bool complete: false
    function check(ok,message){if(!ok){console.error("THREAD_ACTIVITY_QML_FAIL "+message);Qt.exit(1)}}
    function fixture(name){var request=new XMLHttpRequest();request.open("GET",Qt.resolvedUrl("UnderTest/tests/fixtures/"+name),false);request.send();return JSON.parse(request.responseText)}
    Client.T3ThreadClockOwner { id: owner; clockDate: root.fakeDate }
    Component { Client.T3Panel { bridge: null } }
    Text { id: actualLabel; text: Relative.relativeLabel({timestamp:"2026-11-07T12:34:00.000Z"},root.observedMinute) }
    Component.onCompleted: {
        var data=fixture("thread-activity-4301.json")
        data.cases.forEach(function(test){
            check(Native.resolveThreadActivityTimestamp(test.input)===test.activity,"native activity clock "+JSON.stringify(test))
            check(Native.resolveSettledThreadTimestamp(test.input)===test.settled,"native settled clock "+JSON.stringify(test))
        })
        var differences=[]
        data.dateDiagnostics.forEach(function(test){var actual=Native.toSortableTimestamp(test.value===null?undefined:test.value);if(actual!==test.result)differences.push({input:test.value,native:test.result,qt:actual})})
        console.log("THREAD_ACTIVITY_DATE_BOUNDARIES "+JSON.stringify(differences))
        fixture("thread-relative-time-4301.json").cases.forEach(function(test){check(Relative.relativeLabel({timestamp:test.timestamp},test.nowMinute)===test.result,"native relative label "+JSON.stringify(test))})
        var trace=fixture("thread-clock-4301.json"),now=trace.nowStart,serial=0,timers=new Map(),calls=[],events=[]
        function FakeDate(value){return new Date(value===undefined?now:value)}FakeDate.now=function(){return now}
        function schedule(callback,delay){var id=++serial;timers.set(id,{callback:callback,delay:delay});return id}
        function cancel(id){timers.delete(id)}
        var clock=Clock.create(schedule,cancel,FakeDate).minuteTick
        function snapshot(step){events.push({step:step,now:now,value:clock.getSnapshot(),timers:Array.from(timers.values()).map(function(timer){return timer.delay}),calls:calls.slice()})}
        function fire(){var pair=timers.entries().next().value;timers.delete(pair[0]);pair[1].callback()}
        snapshot("cold");var stopA=clock.subscribe(function(){calls.push("a")});snapshot("first-subscribe");var stopB=clock.subscribe(function(){calls.push("b")});snapshot("second-subscribe");now+=1000;snapshot("cached-between-ticks")
        now=Date.parse("2026-11-07T12:35:00.000Z");fire();snapshot("minute-boundary");stopA();snapshot("one-reader-left");stopB();snapshot("last-reader-gone");now=Date.parse("2026-11-07T12:38:00.000Z");snapshot("unsubscribed-catches-up")
        var stopC;stopA=clock.subscribe(function(){calls.push("reentrant-a");stopA();stopC=clock.subscribe(function(){calls.push("reentrant-c")})});now=Date.parse("2026-11-07T12:39:00.000Z");fire();snapshot("reentrant-resubscribe");stopC();snapshot("reentrant-last-reader-gone")
        if(JSON.stringify(events)!==JSON.stringify(trace.events)){console.log("CLOCK_NATIVE "+JSON.stringify(trace.events));console.log("CLOCK_QT "+JSON.stringify(events))}
        check(JSON.stringify(events)===JSON.stringify(trace.events),"whole native minuteTick lifecycle traces")
        unsubscribeA=owner.subscribe(function(){root.notified++;root.observedMinute=owner.getSnapshot()})
        unsubscribeB=owner.subscribe(function(){root.notified++})
        check(owner.timer.running&&owner.timer.interval===10,"actual Qt timer aligns to next native wall-minute boundary")
        check(actualLabel.text==="now","actual text binding starts at native minute precision")
        fakeNow=Date.parse("2026-11-07T12:35:00.000Z")
    }
    Timer {
        interval:20;repeat:true;running:!root.complete
        onTriggered:{
            if(root.notified<2)return
            check(root.notified===2&&owner.getSnapshot()==="2026-11-07T12:35","actual Qt timer notifies both minute subscribers once")
            check(actualLabel.text==="1m","actual Qt label binding advances from shared native minute event")
            check(owner.timer.running&&owner.timer.interval===60000,"minute clock re-arms the shared actual timer")
            root.unsubscribeA();check(owner.timer.running,"one remaining subscriber keeps actual clock alive")
            root.unsubscribeB();check(!owner.timer.running&&owner.pendingCallback===null,"last unmount stops actual timer and clears callback ownership")
            root.complete=true
            console.log("THREAD_ACTIVITY_QML_PASS: 105 activity/settled cases, 36 original relative-label cases, 10 native minuteTick traces, actual Qt timer/text binding/shared subscriptions/alignment/last-reader teardown")
            Qt.callLater(Qt.quit)
        }
    }
    Timer{interval:10000;running:true;onTriggered:{console.error("THREAD_ACTIVITY_QML_FAIL deadline");Qt.exit(1)}}
}
