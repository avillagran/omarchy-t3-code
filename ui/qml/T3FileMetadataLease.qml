import QtQuick
import "../js/NativeFileMetadataCheckpoints.js" as NativeCheckpoints

// Mount one instance per displayed thread. The backend serializes lease controls
// before queries, so teardown fences pending work even when the environment was removed.
QtObject {
    id: root
    required property var bridge
    required property string environment
    required property string thread
    property string cwd: ""
    property var checkpoints: null
    readonly property string leaseId: privateState.leaseId
    property QtObject privateState: QtObject {
        property string leaseId: ""
        property var observer: NativeCheckpoints.create()
        property bool mounted: false
    }
    function observe() {
        if (!privateState.mounted) return
        privateState.observer.update({environmentId: environment, threadId: thread}, cwd,
                                     checkpoints, function(value) {
            bridge.send({op: "file-metadata-invalidate", environment: value.environmentId,
                         paths: value.input.paths, leaseId: leaseId})
        })
    }
    function schedule() { Qt.callLater(observe) }
    onCwdChanged: schedule()
    onCheckpointsChanged: schedule()
    Component.onCompleted: {
        privateState.leaseId = bridge.uuid()
        privateState.mounted = true
        bridge.send({op: "file-metadata-retain", environment: environment, thread: thread,
                     leaseId: leaseId})
        observe()
    }
    Component.onDestruction: {
        privateState.mounted = false
        bridge.send({op: "file-metadata-release", leaseId: leaseId})
    }
}
