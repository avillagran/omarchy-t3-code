.pragma library
// Port of T3 client-runtime threadSort.ts pin order keys (MIT, licenses/T3-Code-MIT.txt).
var DIGITS="abcdefghijklmnopqrstuvwxyz"
function valid(key){if(!key)return false;for(var i=0;i<key.length;i++)if(DIGITS.indexOf(key.charAt(i))<0)return false;return key.charAt(key.length-1)!==DIGITS.charAt(0)}
function midpoint(a,b){
 if(b!==""&&a>=b)throw new Error("bounds out of order")
 if(b!==""){var n=0;while((a.charAt(n)||DIGITS.charAt(0))===b.charAt(n))n+=1;if(n>0)return b.slice(0,n)+midpoint(a.slice(n),b.slice(n))}
 var da=a===""?0:DIGITS.indexOf(a.charAt(0)),db=b===""?DIGITS.length:DIGITS.indexOf(b.charAt(0))
 if(db-da>1)return DIGITS.charAt(Math.round((da+db)/2))
 if(b.length>1)return b.charAt(0)
 return DIGITS.charAt(da)+midpoint(a.slice(1),"")
}
function between(before,after){var a=before||"",b=after||"";if(a!==""&&!valid(a))return null;if(b!==""&&!valid(b))return null;if(b!==""&&a>=b)return null;return midpoint(a,b)}
function spread(count){var width=2,space=Math.pow(DIGITS.length,width);while(space<=(count+1)*2){width+=1;space*=DIGITS.length}var step=space/(count+1),keys=[];for(var i=0;i<count;i++){var value=Math.round(step*(i+1));if(value%DIGITS.length===0)value+=1;var key="";for(var d=0;d<width;d++){key=DIGITS.charAt(value%DIGITS.length)+key;value=Math.floor(value/DIGITS.length)}keys.push(key)}return keys}
// orderedIds: the pinned block after the move; keys: id -> current key. Returns [{id,orderKey}].
function plan(orderedIds,keys,movedId){
 var i=orderedIds.indexOf(movedId);if(i<0)return []
 var beforeId=i>0?orderedIds[i-1]:null,afterId=i<orderedIds.length-1?orderedIds[i+1]:null
 var bk=beforeId!==null?(keys[beforeId]||null):null,ak=afterId!==null?(keys[afterId]||null):null
 if((beforeId===null||bk!==null)&&(afterId===null||ak!==null)){var k=between(bk,ak);if(k!==null)return [{id:movedId,orderKey:k}]}
 var spreadKeys=spread(orderedIds.length);return orderedIds.map(function(id,n){return {id:id,orderKey:spreadKeys[n]}}).filter(function(x){return keys[x.id]!==x.orderKey})
}
