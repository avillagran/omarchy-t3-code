.pragma library
// Qt snapshots additions in its built-in Set iterator. The native clock needs
// live listener iteration during unsubscribe/resubscribe callbacks. This local
// service preserves those operations without changing the engine's globals.
function LiveSet(iterable) {
    var records=new Map(),slots=[],iterators=0
    var self=this
    function compact(){if(iterators===0&&(records.size===0||slots.length>records.size*2))slots=slots.filter(function(slot){return slot.live})}
    Object.defineProperty(this,"size",{get:function(){return records.size}})
    this.has=function(value){return records.has(value)}
    this.add=function(value){if(!records.has(value)){var slot={value:value,live:true};records.set(value,slot);slots.push(slot)}return self}
    this.delete=function(value){var slot=records.get(value);if(slot===undefined)return false;slot.live=false;records.delete(value);compact();return true}
    this.clear=function(){slots.forEach(function(slot){slot.live=false});records.clear();compact()}
    this.values=function(){
        var index=0,closed=false
        iterators++
        function close(){if(!closed){closed=true;iterators--;compact()}}
        var iterator={next:function(){
            if(closed)return {done:true}
            while(index<slots.length){var slot=slots[index++];if(slot.live)return {done:false,value:slot.value}}
            close();return {done:true}
        },return:function(){close();return {done:true}}}
        iterator[Symbol.iterator]=function(){return iterator}
        return iterator
    }
    this.keys=this.values
    this[Symbol.iterator]=this.values
    this.forEach=function(callback,thisArg){for(var value of self)callback.call(thisArg,value,value,self)}
    if(iterable!==undefined&&iterable!==null)for(var value of iterable)this.add(value)
}
