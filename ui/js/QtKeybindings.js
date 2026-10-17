.pragma library
.import "NativeKeybindings.js" as Native

// Linux Qt/Wayland and X11 expose XKB native scan codes (evdev + 8).
// Only the physical punctuation/digit codes used by original shortcutKeyFromEvent
// need adaptation; matching/capture/defaults remain original T3 functions.
var physical={49:"Backquote",51:"Backslash",34:"BracketLeft",35:"BracketRight",59:"Comma",19:"Digit0",10:"Digit1",11:"Digit2",12:"Digit3",13:"Digit4",14:"Digit5",15:"Digit6",16:"Digit7",17:"Digit8",18:"Digit9",21:"Equal",20:"Minus",60:"Period",48:"Quote",47:"Semicolon",61:"Slash"}
function eventLike(event){
 var names={};names[Qt.Key_Return]="Enter";names[Qt.Key_Enter]="Enter";names[Qt.Key_Tab]="Tab";names[Qt.Key_Backtab]="Tab";names[Qt.Key_Escape]="Escape";names[Qt.Key_Space]=" ";names[Qt.Key_Backspace]="Backspace";names[Qt.Key_Delete]="Delete";names[Qt.Key_Home]="Home";names[Qt.Key_End]="End";names[Qt.Key_PageUp]="PageUp";names[Qt.Key_PageDown]="PageDown";names[Qt.Key_Up]="ArrowUp";names[Qt.Key_Down]="ArrowDown";names[Qt.Key_Left]="ArrowLeft";names[Qt.Key_Right]="ArrowRight";names[Qt.Key_Control]="Control";names[Qt.Key_Shift]="Shift";names[Qt.Key_Alt]="Alt";names[Qt.Key_Meta]="Meta"
 var key=names[event.key] || (event.key>=Qt.Key_F1&&event.key<=Qt.Key_F35?"F"+(event.key-Qt.Key_F1+1):event.key>=32&&event.key<=126?String.fromCharCode(event.key):event.text || "")
 return {key:key,code:physical[event.nativeScanCode] || "",ctrlKey:!!(event.modifiers&Qt.ControlModifier),metaKey:!!(event.modifiers&Qt.MetaModifier),shiftKey:!!(event.modifiers&Qt.ShiftModifier),altKey:!!(event.modifiers&Qt.AltModifier),repeat:!!event.isAutoRepeat}
}
function capture(event){return Native.nativeKeys.keybindingFromKeyboardEvent(eventLike(event),"Linux")}
function sequences(shortcut){
 if(["return","up","down","left","right","esc"].indexOf(shortcut.key)>=0)return []
 var text=Native.nativeKeys.formatShortcutLabel(shortcut,"Linux")
 return shortcut.key==="enter"?[text.replace(/Enter$/,"Return"),text]:[text]
}
function sequence(shortcut){return sequences(shortcut)[0] || ""}
function fromQtSequence(text){var aliases={return:"enter",pgup:"pageup",pgdown:"pagedown",up:"arrowup",down:"arrowdown",left:"arrowleft",right:"arrowright"};return Native.nativeKeys.parseKeybindingShortcut(text.toLowerCase().split("+").map(function(t){return aliases[t] || t}).join("+"))}
function when(node,context){return Native.nativeKeys.matchesWhenClause(node,context)}
