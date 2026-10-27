import QtQuick 6.11

// Keep infrequently used popup controls out of the initial object tree. Stateful
// forms stay mounted after first use; read-only pages can release their controls
// once closed while retaining their presentation choices here.
Loader {
  id:root
  active:false
  required property var bridge
  required property url popupSource
  property bool releaseWhenClosed:false
  property var savedProperties:[]
  property var saved:({})
  readonly property bool opened:!!item&&item.opened
  property bool requested:false
  function show(){
    if(item){item.show();return}
    requested=true
    active=true;setSource(popupSource,Object.assign({bridge:bridge},saved))
  }
  onLoaded:{item.parent=root.parent;if(requested){requested=false;item.show()}}

  function close(){requested=false;if(item)item.close()}
  function release(){
    if(!item || item.visible || !releaseWhenClosed)return false
    var next={};savedProperties.forEach(function(key){next[key]=root.item[key]})
    saved=next;active=false;source=""
    return true
  }
  Timer{interval:30000;running:root.active&&!!root.item&&!root.item.visible&&root.releaseWhenClosed;onTriggered:root.release()}
}
