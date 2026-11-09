pragma Singleton
import QtQuick
QtObject {
    readonly property T3ThreadClockOwner owner: T3ThreadClockOwner {}
    function subscribe(listener){return owner.subscribe(listener)}
    function getSnapshot(){return owner.getSnapshot()}
}
