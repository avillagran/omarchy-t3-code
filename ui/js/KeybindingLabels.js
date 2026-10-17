.pragma library
.import "NativeKeybindings.js" as Native

// Command identity and English labels remain owned by the original T3 helper.
// Translate only its fixed label segments, preserving jump numbers/script IDs.
function text(label,tr){
 var parts=label.split(": ")
 return parts.map(function(part,i){
  if(parts[0]==="Run Script"&&i>0)return part
  var jump=/^Jump ([1-9])$/.exec(part)
  return jump?tr("Jump")+" "+jump[1]:/^\d+$/.test(part)?part:tr(part)
 }).join(": ")
}
function command(command,tr){return text(Native.nativeKeys.commandLabel(command),tr)}
