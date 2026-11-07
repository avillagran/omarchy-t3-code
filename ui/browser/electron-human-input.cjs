// Qt events become page input through the actual pinned native GuestInput.
// This module is trusted main-process code, never a guest preload/page API.
'use strict';
const keys=new Map([[0x01000000,'Escape'],[0x01000001,'Tab'],[0x01000002,'Tab'],[0x01000003,'Backspace'],[0x01000004,'Enter'],[0x01000005,'Enter'],[0x01000006,'Insert'],[0x01000007,'Delete'],[0x01000008,'Pause'],[0x01000009,'PrintScreen'],[0x01000010,'Home'],[0x01000011,'End'],[0x01000012,'ArrowLeft'],[0x01000013,'ArrowUp'],[0x01000014,'ArrowRight'],[0x01000015,'ArrowDown'],[0x01000016,'PageUp'],[0x01000017,'PageDown'],[0x01000020,'Shift'],[0x01000021,'Control'],[0x01000022,'Meta'],[0x01000023,'Alt'],[0x01000024,'CapsLock'],[0x01000025,'NumLock'],[0x01000026,'ScrollLock']]);
const modifiers=value=>(value&0x08000000?1:0)|(value&0x04000000?2:0)|(value&0x10000000?4:0)|(value&0x02000000?8:0);
const virtualKeys=new Map([[0x01000000,27],[0x01000001,9],[0x01000002,9],[0x01000003,8],[0x01000004,13],[0x01000005,13],[0x01000006,45],[0x01000007,46],[0x01000010,36],[0x01000011,35],[0x01000012,37],[0x01000013,38],[0x01000014,39],[0x01000015,40],[0x01000016,33],[0x01000017,34],[0x01000020,16],[0x01000021,17],[0x01000022,91],[0x01000023,18]]);
const finite=value=>typeof value==='number'&&Number.isFinite(value);
const coordinate=value=>finite(value)&&Math.abs(value)<=16384;
function keyOf(event){if(keys.has(event.key))return keys.get(event.key);if(event.key>=0x01000030&&event.key<=0x01000047)return 'F'+(event.key-0x01000030+1);if(Number.isInteger(event.key)&&event.key>=32&&event.key<0x110000)return String.fromCodePoint(event.key);return [...String(event.text||'')].length===1?String(event.text):'Unidentified'}
exports.create=(contents,guard,nativeInput)=>{
 const send=(method,params)=>nativeInput(contents,{sendCommand:(...args)=>guard.sendCommand(...args)},method,params);
 return Object.freeze({accept:async event=>{
  const bits=modifiers(event.modifiers||0)|(event.key===0x01000002?8:0);
  if(['press','release','move','double'].includes(event.type)){
   if(!coordinate(event.x)||!coordinate(event.y))throw Error('invalid_browser_input');
   const button=event.type==='move'?'none':({1:'left',2:'right',4:'middle'})[event.button];if(!button)throw Error('invalid_browser_input');
   return send('Input.dispatchMouseEvent',{type:event.type==='move'?'mouseMoved':event.type==='release'?'mouseReleased':'mousePressed',x:event.x,y:event.y,button,buttons:(event.buttons||0)&7,modifiers:bits,clickCount:event.type==='move'?0:event.type==='double'?2:1});
  }
  if(event.type==='wheel'){
   if(![event.x,event.y,event.px||0,event.py||0,event.ax||0,event.ay||0].every(coordinate))throw Error('invalid_browser_input');
   return send('Input.dispatchMouseEvent',{type:'mouseWheel',x:event.x,y:event.y,deltaX:-(event.px||event.ax||0),deltaY:-(event.py||event.ay||0),modifiers:bits});
  }
  if(event.type==='key'||event.type==='keyup'){
   const text=String(event.text||'');if(text.length>4096)throw Error('invalid_browser_input');
   return send('Input.dispatchKeyEvent',{type:event.type==='keyup'?'keyUp':'keyDown',key:keyOf(event),text:bits&6?'':text,unmodifiedText:bits&6?'':text,modifiers:bits,autoRepeat:event.repeat===true,windowsVirtualKeyCode:virtualKeys.get(event.key)??(event.key>=0x01000030&&event.key<=0x01000047?112+event.key-0x01000030:event.key>=32&&event.key<=126?event.key:0)});
  }
  if(event.type==='text'){
   if(typeof event.text!=='string'||event.text.length>4096)throw Error('invalid_browser_input');return send('Input.insertText',{text:event.text});
  }
  if(event.type==='composition'){
   if(typeof event.preedit!=='string'||event.preedit.length>4096||!Number.isInteger(event.selectionStart)||!Number.isInteger(event.selectionEnd)||event.selectionStart<0||event.selectionEnd<event.selectionStart||event.selectionEnd>event.preedit.length)throw Error('invalid_browser_input');
   const params={text:event.preedit,selectionStart:event.selectionStart,selectionEnd:event.selectionEnd};
   if(event.replacementStart!==undefined){if(!Number.isInteger(event.replacementStart)||!Number.isInteger(event.replacementEnd)||event.replacementStart<0||event.replacementEnd<event.replacementStart)throw Error('invalid_browser_input');params.replacementStart=event.replacementStart;params.replacementEnd=event.replacementEnd}
   return send('Input.imeSetComposition',params);
  }
  throw Error('invalid_browser_input');
 }});
};
