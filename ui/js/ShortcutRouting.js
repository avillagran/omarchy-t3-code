.pragma library
.import "NativeKeybindings.js" as Native
.import "QtKeybindings.js" as QtKeys

// Original effectiveShortcutsForCommand owns reverse-order precedence across ALL
// commands, including commands whose workspace surface is not implemented yet.
function effective(primary,active,context){
 var k=Native.nativeKeys,options={platform:"Linux",context:context},out=[],blocked={}
 var commands=Array.from(new Set(primary.map(function(r){return r.command})))
 commands.forEach(function(command){
  if(k.projectScriptIdFromCommand(command)!==null)return
  k.effectiveShortcutsForCommand(primary,command,options).forEach(function(shortcut){var key=k.shortcutConflictKey(shortcut,"Linux");blocked[key]=true;out.push({command:command,shortcut:shortcut,sequence:QtKeys.sequence(shortcut),sequences:QtKeys.sequences(shortcut),source:"primary"})})
 })
 var scripts=Array.from(new Set(active.map(function(r){return r.command}))).filter(function(c){return k.projectScriptIdFromCommand(c)!==null})
 scripts.forEach(function(command){k.effectiveShortcutsForCommand(active,command,options).forEach(function(shortcut){if(!blocked[k.shortcutConflictKey(shortcut,"Linux")])out.push({command:command,shortcut:shortcut,sequence:QtKeys.sequence(shortcut),sequences:QtKeys.sequences(shortcut),source:"active"})})})
 return out
}
function qtIdentity(shortcut){var k=Native.nativeKeys,key=shortcut.key;return k.shortcutConflictKey(Object.assign({},shortcut,{key:key}),"Linux")}
function reserved(shortcut,context){
 // Keep composer send/newline keys from being stolen by native shortcuts.
 if(shortcut.key!=="enter"&&shortcut.key!=="return" || shortcut.metaKey || shortcut.altKey)return false
 return !!(shortcut.ctrlKey||shortcut.modKey || (context && context.composerFocus))
}
