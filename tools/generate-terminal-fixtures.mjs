#!/usr/bin/env node
// Execute the complete pinned T3 core and bundled WASM, with file-only imports.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {stripTypeScriptTypes} from 'node:module';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const [checkout,output]=process.argv.slice(2);
if(!checkout||!output)throw Error('usage: generate-terminal-fixtures.mjs T3_CHECKOUT OUTPUT');
const revision='c55db3e9c72fdf584f4b54192fb39beb88667b59';
const root='apps/web/src/terminal/ghostty/';
const source=file=>execFileSync('git',['-C',checkout,'show',`${revision}:${file}`],{maxBuffer:16*1024*1024});
const directory=path.dirname(output);await mkdir(directory,{recursive:true});
const origins={};
const get=file=>{const bytes=source(file);origins[file]=createHash('sha256').update(bytes).digest('hex');return bytes;};
for(const file of ['ghostty-vt.wasm','ghostty-write-pty.wasm'])await writeFile(path.join(directory,file),get(root+'vendor/'+file));
const shifted=get('packages/shared/src/keybindings.ts').toString().match(/export const US_SHIFTED_TO_UNSHIFTED_KEYS[^]*?\n\]\);/)[0];
for(const name of ['runtime','keyCodes','core']){
 let text=get(root+name+'.ts').toString();
 if(name==='runtime')text=text.replace(/import ghosttyWasmUrl[^;]+;/,'const ghosttyWasmUrl='+JSON.stringify(pathToFileURL(path.join(directory,'ghostty-vt.wasm')).href)+';').replace(/import ghosttyWritePtyWasmUrl[^;]+;/,'const ghosttyWritePtyWasmUrl='+JSON.stringify(pathToFileURL(path.join(directory,'ghostty-write-pty.wasm')).href)+';');
 if(name==='keyCodes')text=text.replace(/import \{ US_SHIFTED_TO_UNSHIFTED_KEYS \} from "@t3tools\/shared\/keybindings";/,shifted.replace('export ',''));
 text=stripTypeScriptTypes(text).replace(/from "\.\/(runtime|keyCodes)"/g,(_,name)=>'from '+JSON.stringify(pathToFileURL(path.join(directory,name+'.mjs')).href));
 await writeFile(path.join(directory,name+'.mjs'),text);
}
globalThis.fetch=async url=>new Response(await readFile(new URL(url)));
const {GhosttyTerminalCore}=await import(pathToFileURL(path.join(directory,'core.mjs')).href);
const theme={foreground:{r:216,g:222,b:233},background:{r:17,g:19,b:24},cursor:{r:216,g:222,b:233}};
const cases=[];
const add=(label,spec)=>cases.push({label,cols:20,rows:4,...spec});
add('ansi',{vt:'one\r\n\x1b[31;1mred\x1b[0m'});
add('truecolor-inverse-faint',{vt:'\x1b[38;2;101;143;207mA\x1b[48;2;15;45;81mB\x1b[7mC\x1b[2mD\x1b[0mE'});
add('graphemes-split',{vt:'日本 e\u0301 😀',splitBytes:true});
add('wide-wrap-spacers',{cols:6,vt:'abc日本語\r\nxy😀z'});
add('extended-combining',{vt:'a'+'\u0301'.repeat(1000)+'b'});
add('cursor-erase-insert',{vt:'abcdef\x1b[1;3H\x1b[2@XY\x1b[1;1HZ\x1b[2;1Hlast\x1b[1D\x1b[P'});
add('alternate-return',{vt:'main\x1b[?1049h\x1b[Halt\x1b[?1049l'});
add('osc-color-overrides',{vt:'A\x1b]10;#123456\x07B\x1b]11;#456789\x07C\x1b]4;1;#ab1234\x07\x1b[31mD'});
add('explicit-hyperlink',{vt:'\x1b]8;;https://example.com/path\x1b\\link\x1b]8;;\x1b\\ end',hyperlink:[0,1]});
add('selection-soft-wrap',{cols:6,vt:'abcdefghijk',select:[0,0,1,5]});
add('selection-hard-lines',{vt:'alpha  \r\nbeta\r\n日本 e\u0301',select:[0,1,2,7]});
add('history-oldest',{rows:3,history:12,scrollTop:true,select:[0,0,0,4]});
add('history-allocation-boundary',{rows:3,history:10010});
for(const mode of ['', '\x1b[?1h','\x1b[>1u','\x1b[>3u','\x1b[>31u']){
 const keys=[];
 const append=(key,code,qt,text,mods=0)=>{for(const action of ['press','release'])keys.push({key,code,qt,text,mods,action});};
 append('c','KeyC','C','\u0003',2);append('A','KeyA','A','A',1);append('x','KeyX','X','x');
 append('ArrowUp','ArrowUp','Up','');append('ArrowLeft','ArrowLeft','Left','',2);
 append('Enter','Enter','Return','\r');append('Tab','Tab','Backtab','',1);
 append('Backspace','Backspace','Backspace','\b');append('Escape','Escape','Escape','\x1b');
 append('F5','F5','F5','');append('?','Slash','Slash','?',1);append('é','KeyE','E','é');
 add('keyboard:'+JSON.stringify(mode),{vt:mode,keys});
}
for(const bracketed of [false,true])for(const paste of ['one\ntwo','one\r\ntwo','a\x1b[201~b','a\x00b','日本😀'])add('paste:'+bracketed+':'+JSON.stringify(paste),{vt:bracketed?'\x1b[?2004h':'',paste});
for(const mode of [9,1000,1002,1003])for(const format of [1006,1015,1016])add('mouse:'+mode+':'+format,{vt:'\x1b[?'+mode+'h\x1b[?'+format+'h',mouse:[
 {action:'press',button:1,mods:0,x:1,y:1,anyButtonPressed:true},
 {action:'motion',button:1,mods:0,x:10,y:20,anyButtonPressed:true},
 {action:'release',button:1,mods:0,x:10,y:20,anyButtonPressed:false},
 {action:'motion',button:null,mods:0,x:1,y:1,anyButtonPressed:false},
]});
for(const value of cases){
 const replies=[];const core=await GhosttyTerminalCore.create(80,24,8,16,theme,data=>replies.push(data));
 core.resize(value.cols,value.rows,8,16);
 if(value.history){for(let i=0;i<value.history;i++)core.write(value.history===12?'row'+i+'\r\n':'x\r\n');}
 if(value.vt){if(value.splitBytes)for(const byte of new TextEncoder().encode(value.vt))core.write(new Uint8Array([byte]));else core.write(value.vt);}
 if(value.scrollTop){const state=core.scrollbarState();core.scroll(-state.total);}
 if(value.select){const [y1,x1,y2,x2]=value.select;core.setSelection({x:x1,y:y1},{x:x2-1,y:y2});}
 const snapshot=core.snapshot();
 value.expected={rows:snapshot.rowData.map(row=>row.cells.map(cell=>({text:cell.text,foreground:cell.foreground,background:cell.background,bold:cell.bold,italic:cell.italic,underline:cell.underline,selected:cell.selected}))),cursor:[snapshot.cursorY,snapshot.cursorX],scrollbar:core.scrollbarState(),selection:core.selectionText(),replies};
 if(value.hyperlink)value.expected.hyperlink=core.hyperlinkAt(value.hyperlink[1],value.hyperlink[0]);
 if(value.keys)value.expected.keys=value.keys.map(input=>core.encodeKey({...input,shiftKey:!!(input.mods&1),ctrlKey:!!(input.mods&2),altKey:!!(input.mods&4),metaKey:!!(input.mods&8),isComposing:false,repeat:false,getModifierState:()=>false},input.action));
 if(value.paste!==undefined)value.expected.paste=core.encodePaste(value.paste);
 if(value.mouse)value.expected.mouse=value.mouse.map(input=>core.encodeMouse({...input,screenWidth:160,screenHeight:64,cellWidth:8,cellHeight:16,paddingLeft:0,paddingRight:0,paddingTop:0,paddingBottom:0}));
 core.dispose();
}
await writeFile(output,JSON.stringify({revision,engineRevision:'9f62873bf195e4d8a762d768a1405a5f2f7b1697',origins,cases})+'\n');
console.log(`Pinned terminal oracle: ${cases.length} cases -> ${output}`);
