import QtQuick
import "../js/NativeThreadClock.js" as Native

QtObject {
    id: root
    // The native store accepts clock/timer services. Production uses Date;
    // focused tests inject wall time while retaining the actual Qt timer.
    property var clockDate: Date
    property var pendingCallback: null
    property var store: null
    function ensureStore(){if(store===null)store=Native.create(root.schedule,root.cancel,clockDate).minuteTick;return store}
    property Timer timer: Timer {
        repeat: false
        onTriggered: {
            var callback=root.pendingCallback
            root.pendingCallback=null
            if(callback)callback()
        }
    }
    function schedule(callback,delay) {
        pendingCallback=callback
        timer.interval=delay
        timer.restart()
        return timer
    }
    function cancel(handle) {
        if(handle===timer){timer.stop();pendingCallback=null}
    }
    function subscribe(listener){return ensureStore().subscribe(listener)}
    function getSnapshot(){return ensureStore().getSnapshot()}
}
