.pragma library

// Default JSON wire serialization. Quote native UTF-16 directly: older Qt may
// emit or repair an isolated unit in JSON.stringify before QProcess sees it.
// No actual text passes through that conversion, including property names.
function quote(text) {
  var out='"'
  for(var i=0;i<text.length;i++) {
    var code=text.charCodeAt(i),unit=text.charAt(i)
    if(code===34)out+='\\"'
    else if(code===92)out+='\\\\'
    else if(code===8)out+='\\b'
    else if(code===9)out+='\\t'
    else if(code===10)out+='\\n'
    else if(code===12)out+='\\f'
    else if(code===13)out+='\\r'
    else if(code<32)out+='\\u'+('0000'+code.toString(16)).slice(-4)
    else if(code>=0xd800&&code<=0xdbff) {
      var next=i+1<text.length?text.charCodeAt(i+1):0
      if(next>=0xdc00&&next<=0xdfff){out+=unit+text.charAt(++i)}
      else out+='\\u'+('0000'+code.toString(16)).slice(-4)
    } else if(code>=0xdc00&&code<=0xdfff)out+='\\u'+('0000'+code.toString(16)).slice(-4)
    else out+=unit
  }
  return out+'"'
}
function stringify(value) {
  var stack=[]
  function encode(value,key) {
    if(value!==null&&typeof value==='object'&&typeof value.toJSON==='function')value=value.toJSON(key)
    if(value===null)return 'null'
    var type=typeof value
    if(type==='string')return quote(value)
    // Keep the engine's ordinary number representation, including -0/NaN.
    if(type==='number'||type==='boolean')return JSON.stringify(value)
    if(type==='undefined'||type==='function'||type==='symbol')return undefined
    if(type==='bigint')throw new TypeError('BigInt cannot be serialized as JSON')
    if(type!=='object')throw new TypeError('Unsupported JSON wire value')
    var tag=Object.prototype.toString.call(value)
    if(tag==='[object String]')return quote(String(value))
    if(tag==='[object Number]')return JSON.stringify(Number(value))
    if(tag==='[object Boolean]')return JSON.stringify(Boolean.prototype.valueOf.call(value))
    if(stack.indexOf(value)>=0)throw new TypeError('Circular JSON wire value')
    stack.push(value)
    var parts=[],encoded
    if(Array.isArray(value)) {
      for(var i=0;i<value.length;i++){encoded=encode(value[i],String(i));parts.push(encoded===undefined?'null':encoded)}
      stack.pop();return '['+parts.join(',')+']'
    }
    var keys=Object.keys(value)
    for(var i=0;i<keys.length;i++) {
      encoded=encode(value[keys[i]],keys[i])
      if(encoded!==undefined)parts.push(quote(keys[i])+':'+encoded)
    }
    stack.pop();return '{'+parts.join(',')+'}'
  }
  return encode(value,'')
}
