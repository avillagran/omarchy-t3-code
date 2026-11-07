// Actual pinned services and Playwright classes; synthetic data on a private seat.
'use strict';
const assert=require('node:assert/strict');
const {spawn,spawnSync}=require('node:child_process');
const readline=require('node:readline');
const fs=require('node:fs');
const path=require('node:path');
module.exports=async function({tab,evaluate,command,producerCommand,seatFocus,services,dialogs,windows,server,producer,app,clipboard}){
 const {Keyboard,RawKeyboardImpl}=require('./playwright/lib/coreBundle.js').__ownedKeyboard;
 const pointer=spawn('/proof/pointer',[],{stdio:['pipe','pipe','pipe']});const replies=[];let wake;
 readline.createInterface({input:pointer.stdout}).on('line',line=>{replies.push(line);wake?.();wake=null});
 async function line(){while(!replies.length)await new Promise(resolve=>wake=resolve);return replies.shift()}
 assert.equal(await line(),'ready');
 async function cursor(value){pointer.stdin.write(value+'\n');assert.equal(await line(),'ok');await new Promise(resolve=>setTimeout(resolve,50))}
 async function focus(value){await seatFocus(value.window.getTitle());await value.window.webContents.executeJavaScript("document.querySelector('webview').focus()");value.contents.focus();await new Promise(resolve=>setTimeout(resolve,60))}
 async function click(value,x,y){await focus(value);const socket=fs.readdirSync(process.env.XDG_RUNTIME_DIR).find(name=>name.startsWith('sway-ipc')&&name.endsWith('.sock'));const tree=JSON.parse(spawnSync('/opt/sway-runtime/usr/bin/swaymsg',['-t','get_tree'],{encoding:'utf8',env:{...process.env,SWAYSOCK:path.join(process.env.XDG_RUNTIME_DIR,socket),LD_LIBRARY_PATH:'/opt/sway-runtime/usr/lib'}}).stdout);function find(node){if(node.name===value.window.getTitle())return node;for(const child of [...node.nodes||[],...node.floating_nodes||[]]){const match=find(child);if(match)return match}}const node=find(tree);assert(node);await cursor('move '+(node.rect.x+x)+' '+(node.rect.y+y));await cursor('press');await cursor('release')}
 const denied="navigator.clipboard.readText().then(()=>false,()=>true)";
 const one=await tab('physical-paste','127.0.0.1');assert.equal(await evaluate(one,denied),true);assert.equal(dialogs.length,1);
 await evaluate(one,"document.body.innerHTML='<textarea id=target style=\"position:fixed;left:20px;top:20px;width:300px;height:100px\"></textarea>'");
 await producerCommand({op:'text',text:'native genuine paste under Block'});await click(one,100,60);
 const keys=spawnSync('wtype',['-M','ctrl','-k','v','-m','ctrl'],{encoding:'utf8'});assert.equal(keys.status,0);await new Promise(resolve=>setTimeout(resolve,100));
 const humanPaste=await evaluate(one,'target.value');assert.equal(humanPaste,'native genuine paste under Block');const afterHuman=await evaluate(one,denied);assert.equal(dialogs.length,1);
 const two=await tab('physical-write','localhost');await evaluate(two,"document.body.innerHTML='<button id=write style=\"position:fixed;left:20px;top:20px;width:300px;height:100px\">Write</button>';write.onclick=async event=>{window.clicked={trusted:event.isTrusted,active:navigator.userActivation.isActive};try{await navigator.clipboard.writeText('native genuine page write');window.writeResult='ok'}catch(error){window.writeResult=error.name}};");
 await click(two,100,60);let writeResult;for(let i=0;i<100;i++){writeResult=await evaluate(two,'({clicked:window.clicked,result:window.writeResult})');if(writeResult.result)break;await new Promise(resolve=>setTimeout(resolve,20))}assert.equal(writeResult.clicked.trusted,true);assert.equal(writeResult.clicked.active,true);const written=(await producerCommand({op:'read'})).text;assert.equal(writeResult.result,'ok');assert.equal(written,'native genuine page write');
 const three=await tab('agent-keyboard','127.0.0.1');assert.equal(await evaluate(three,denied),true);await evaluate(three,"document.body.innerHTML='<textarea id=target></textarea>';target.value='native actual Playwright copy';target.focus();target.select()");await focus(three);
 const frames=[];const client={send:async(method,params)=>{frames.push({method,params});return command(three,method,params,three.session)}};const keyboard=new Keyboard(new RawKeyboardImpl(client,false,{cancelDrag:async()=>false}),{});const progress={race:promise=>promise,wait:ms=>new Promise(resolve=>setTimeout(resolve,ms))};
 await keyboard.press(progress,'Control+c');const agentCopied=(await producerCommand({op:'read'})).text;
 await producerCommand({op:'text',text:'native actual Playwright paste'});await focus(three);await evaluate(three,"target.value='';target.focus()");await keyboard.press(progress,'Control+v');const agentPaste=await evaluate(three,'target.value');const agentReadDenied=await evaluate(three,denied);assert.equal(agentCopied,'native actual Playwright copy');assert.equal(agentPaste,'native actual Playwright paste');assert.equal(agentReadDenied,true);
 await evaluate(three,"target.value='native current copy survives tab';target.focus();target.select()");await keyboard.press(progress,'Control+c');services.detach(three.key);three.window.destroy();const retainedText=(await producerCommand({op:'read'})).text;assert.equal(retainedText,'native current copy survives tab');
 const imageTab=await tab('current-image','127.0.0.1');await evaluate(imageTab,"(async()=>{const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;canvas.getContext('2d').fillStyle='#123456';canvas.getContext('2d').fillRect(0,0,2,2);const image=new Image;image.src=canvas.toDataURL();await image.decode();image.style='position:fixed;left:20px;top:20px;width:100px;height:100px';document.body.appendChild(image)})()");await focus(imageTab);imageTab.contents.copyImageAt(50,50);await new Promise(resolve=>setTimeout(resolve,100));services.detach(imageTab.key);imageTab.window.destroy();await services.close();const retainedImage=await producerCommand({op:'read-image'});assert.deepEqual([retainedImage.width,retainedImage.height,retainedImage.red,retainedImage.green,retainedImage.blue,retainedImage.alpha],[2,2,18,52,86,255]);
 console.log('NATIVE_INPUT_BASELINE '+JSON.stringify({humanPaste,afterHumanReadDenied:afterHuman,writeResult,written,agentCopied,agentPaste,agentReadDenied,retainedText,retainedImage,frames,dialogs:dialogs.length}));
 pointer.stdin.end('quit\n');for(const window of windows){if(!window.isDestroyed()){services.detach(window.__ownedProofKey);window.destroy()}}await services.close();producer.stdin.end('{"op":"quit"}\n');server.close();console.log('NATIVE_ELECTRON_PROOF '+JSON.stringify({nativeInputBaseline:true}));app.exit(0);
};
