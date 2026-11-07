import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/NativeFileMetadata.js" as Admission
import "UnderTest/ui/js/NativeFileMetadataCheckpoints.js" as Checkpoints
import "UnderTest/ui/js/JsonDelta.js" as Delta
import "UnderTest/ui/js/TerminalLinkRouting.js" as Routing
ShellRoot {
    id: root
    property int serial: 0
    property var calls: []
    property int phase: 0
    property var observer: Checkpoints.create()
    function check(ok, message) {
        if (!ok) { console.error("FILE_METADATA_QML_FAIL " + message); Qt.exit(1) }
    }
    function fixture(name) {
        var request = new XMLHttpRequest()
        request.open("GET", Qt.resolvedUrl("UnderTest/tests/fixtures/" + name), false)
        request.send()
        return JSON.parse(request.responseText)
    }
    // Compile the actual central mount integration without constructing an unrelated desktop.
    Component { id: panelType; Client.T3Panel { bridge: null } }
    QtObject {
        id: fakeBridge
        function uuid() { return "lease-" + (++root.serial) }
        function send(value) { root.calls = root.calls.concat([value]) }
    }
    Loader {
        id: mount
        active: false
        sourceComponent: Client.T3FileMetadataLease {
            bridge: fakeBridge
            environment: "env"
            thread: "thread"
            cwd: "/work"
            checkpoints: [{id: "one", status: "ready", files: [{path: "first.ts"}]}]
        }
    }
    Component.onCompleted: {
        var data = fixture("file-metadata-admission-4288.json"), paths = Admission.create()
        data.cases.forEach(function(test) {
            check(JSON.stringify(paths.query("env", test.raw, test.cwd, test.ref)) === JSON.stringify(test.result), "native admission " + JSON.stringify(test))
        })
        data.terminalCases.forEach(function(test) {
            var target = paths.resolveLink(test.raw, test.cwd)
            check(target === test.target, "native terminal resolution")
            check(JSON.stringify(paths.query("env", target, test.cwd, {environmentId:"env", threadId:"thread"})) === JSON.stringify(test.result), "native terminal admission")
            var context = {state:{projects:[]},selectedEnvironment:"env",selectedThread:"thread",selectedInfo:{worktreePath:"/wrong"},terminalViews:{physical:{cwd:test.cwd}}}
            var descriptor = Routing.descriptor(context,test.raw,"term","physical")
            check(descriptor.target === test.target && JSON.stringify(descriptor.query) === JSON.stringify(test.result), "real routing uses actual surface cwd and complete native composition")
        })
        data.workspaceCases.forEach(function(test) {
            check(paths.resolveWorkspace(test.raw, test.cwd) === test.result, "native workspace resolution")
        })
        var provider = fixture("file-metadata-provider-4288.json"), invalidations = []
        provider.cases.forEach(function(test) {
            invalidations = []
            var value = test.input, checkpoints = value.checkpoints === null ? null : value.checkpoints.map(function(checkpoint) {
                return {id: checkpoint.id, status: checkpoint.status, files: provider.aliases[checkpoint.files]}
            })
            observer.update(value.environment && value.thread ? {environmentId:value.environment, threadId:value.thread} : null,
                            value.cwd, checkpoints, function(value) { invalidations.push({invalidate:value}) })
            var expected = test.trace.filter(function(entry) { return entry.invalidate !== undefined })
            check(JSON.stringify(invalidations) === JSON.stringify(expected), "native checkpoint effect " + JSON.stringify(test.input))
        })
        var deltaObserver = Checkpoints.create(), deltaInvalidations = []
        var view = {metadataCheckpoints:[{id:"one",status:"ready",files:[{path:"tracked.ts"}]}],timeline:[]}
        deltaObserver.update({environmentId:"env",threadId:"thread"}, "/work", view.metadataCheckpoints, function(value){deltaInvalidations.push(value)})
        var next = Delta.apply(view,{o:{timeline:{$:[{id:"message",text:"new message"}]}}})
        check(next.metadataCheckpoints === view.metadataCheckpoints, "actual structural delta preserves checkpoint array identity")
        deltaObserver.update({environmentId:"env",threadId:"thread"}, "/work", next.metadataCheckpoints, function(value){deltaInvalidations.push(value)})
        check(deltaInvalidations.length === 0, "unrelated live message cannot invalidate file metadata")
        mount.active = true
    }
    Timer {
        interval: 40; running: true; repeat: true
        onTriggered: {
            if (root.phase === 0) {
                if (!mount.item) return
                check(calls.length === 1 && calls[0].op === "file-metadata-retain", "mount retains before any invalidation")
                mount.item.checkpoints = [{id:"one",status:"ready",files:[{path:"second.ts"}]}]
                root.phase = 1
            } else if (root.phase === 1) {
                if (calls.length < 2) return
                check(calls[1].op === "file-metadata-invalidate" && JSON.stringify(calls[1].paths) === JSON.stringify(["/work/second.ts","/work/first.ts"]), "actual Qt checkpoint notification invalidates both revisions")
                mount.item.cwd = "/other"
                root.phase = 2
            } else if (root.phase === 2) {
                check(calls.length === 2, "cwd transition resets baseline without invalidation")
                mount.visible = false
                root.phase = 20
            } else if (root.phase === 20) {
                check(calls.length === 2 && !!mount.item, "hiding a still-mounted conversation does not release its lease")
                mount.active = false
                root.phase = 3
            } else if (root.phase === 3) {
                check(calls.length === 3 && calls[2].op === "file-metadata-release" && calls[2].leaseId === calls[0].leaseId, "destruction releases exact mounted lease")
                mount.active = true
                root.phase = 4
            } else if (root.phase === 4) {
                if (!mount.item) return
                check(calls.length === 4 && calls[3].op === "file-metadata-retain" && calls[3].leaseId !== calls[0].leaseId, "remount gets distinct retention owner")
                mount.active = false
                console.log("FILE_METADATA_QML_PASS: 720 native admission/path cases, 19 native checkpoint traces, real Loader mount/change/hide/destroy/remount lease events")
                Qt.callLater(Qt.quit)
                root.phase = 5
            }
        }
    }
    Timer { interval: 10000; running: true; onTriggered: { console.error("FILE_METADATA_QML_FAIL deadline"); Qt.exit(1) } }
}
