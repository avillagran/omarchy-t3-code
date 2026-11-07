// Private local metadata helper. No network, audio, key or generic settings API.
'use strict';
if(process.argv.length!==6||process.versions.electron!=='44.10.1'||process.versions.node!=='24.22.1')throw Error('Pinned consent runtime required');
const [descriptor,parent,directory,addon]=process.argv.slice(2);
const fd=Number(descriptor),expectedParent=Number(parent);
require(addon).admit(fd,expectedParent);
// Confine the descriptor before loading any application/dependency producer.
const net=require('node:net');
const crypto=require('node:crypto');
const {ConsentStorage}=require('./consent-storage.cjs');
const {create}=require('./native-consent.cjs');
const socket=new net.Socket({fd,readable:true,writable:true});
const MAX_PACKET=32*1024*1024,MAX_HANDLES=256;
let closed=false,buffer=Buffer.alloc(0),lastId=0;
function send(value){if(closed)return;const bytes=Buffer.from(JSON.stringify(value)+'\n');if(bytes.length>MAX_PACKET||socket.writableLength+bytes.length>MAX_PACKET)throw Error('Consent control capacity');socket.write(bytes);}
const listeners=new Map();
const window={
 addEventListener(type,callback){let values=listeners.get(type);if(!values)listeners.set(type,values=new Set());values.add(callback);},
 removeEventListener(type,callback){listeners.get(type)?.delete(callback);},
 dispatchEvent(event){for(const callback of listeners.get(event.type)??[])callback(event);return true;}
};
const storage=new ConsentStorage(directory,()=>{try{window.dispatchEvent({type:'storage',key:null});send({type:'changed'});}catch{finish(false);}});
window.localStorage=storage;
const consent=create({window,CustomEvent:class{constructor(type,{detail}){this.type=type;this.detail=detail;}},console:{error(){/* Never log a destination or corrupt storage body. */}}});
const bindings=new Map(),acknowledgements=new Map();
function opaque(table,value){if(table.size>=MAX_HANDLES)throw Error('Consent handle capacity');const id=crypto.randomBytes(24).toString('hex');table.set(id,value);return id;}
function exact(object,names){if(!object||Array.isArray(object)||typeof object!=='object'||Object.keys(object).sort().join('|')!==names.sort().join('|'))throw Error('Invalid consent control');}
function innerString(value){if(typeof value!=='string')throw Error('Invalid consent value');return JSON.parse(value);}
function snapshot(){return {choiceJson:JSON.stringify(consent.useAudioChoice()),cleanupJson:JSON.stringify(consent.useCleanupGrant())};}
function command(packet){
 if(!Number.isSafeInteger(packet?.id)||packet.id<=lastId)throw Error('Invalid consent sequence');lastId=packet.id;
 const base=['id','op'];let result={};
 switch(packet.op){
  case 'read':exact(packet,base);result=snapshot();break;
  case 'choose-audio':exact(packet,[...base,'destinationJson']);consent.chooseAudioDestination(innerString(packet.destinationJson));result=snapshot();break;
  case 'choose-cleanup':exact(packet,[...base,'enabled']);if(typeof packet.enabled!=='boolean')throw Error('Invalid cleanup choice');consent.chooseTranscriptCleanup(packet.enabled);result=snapshot();break;
  case 'pending-cloud':exact(packet,[...base,'destinationJson']);result={acknowledgement:opaque(acknowledgements,consent.pendingCloudAudioChoice(innerString(packet.destinationJson))),...snapshot()};break;
  case 'acknowledge':exact(packet,[...base,'acknowledgement']);if(typeof packet.acknowledgement!=='string'||!acknowledgements.has(packet.acknowledgement))throw Error('Unknown acknowledgement');acknowledgements.get(packet.acknowledgement)();acknowledgements.delete(packet.acknowledgement);result=snapshot();break;
  case 'bind':exact(packet,[...base,'grantIdJson']);{
   const id=innerString(packet.grantIdJson);if(typeof id!=='string')throw Error('Invalid grant identity');
   const check=consent.bindAudioGrant(id),choice=consent.useAudioChoice();
   // The recording derives its destination from this consent realm. A stale
   // UI destination is never an independent authority to select an endpoint.
   const destination=choice&&choice!=='off'&&choice.id===id&&check()?choice.destination:null;
   result={binding:opaque(bindings,check),destinationJson:JSON.stringify(destination)};
  }break;
  case 'check':exact(packet,[...base,'binding']);result={allowed:typeof packet.binding==='string'&&bindings.has(packet.binding)&&bindings.get(packet.binding)()===true};break;
  case 'release':exact(packet,[...base,'binding']);bindings.delete(packet.binding);break;
  case 'forget-ack':exact(packet,[...base,'acknowledgement']);acknowledgements.delete(packet.acknowledgement);break;
  case 'close':exact(packet,base);send({type:'result',id:packet.id,ok:true});finish(true);return;
  default:throw Error('Unknown consent operation');
 }
 send({type:'result',id:packet.id,ok:true,...result});
}
function finish(clean){if(closed)return;closed=true;storage.close();bindings.clear();acknowledgements.clear();if(clean)socket.end(()=>process.exit(0));else{socket.destroy();process.exitCode=2;}}
socket.on('data',bytes=>{
 try{
  if(buffer.length+bytes.length>MAX_PACKET)throw Error('Consent control capacity');buffer=Buffer.concat([buffer,bytes]);
  for(let end;(end=buffer.indexOf(10))>=0;){const line=buffer.subarray(0,end);buffer=buffer.subarray(end+1);command(JSON.parse(line.toString('utf8')));if(closed)break;}
 }catch{finish(false);}
});
socket.on('error',()=>finish(false));socket.on('end',()=>finish(false));
send({type:'ready',protocol:'o3-native-consent-v1'});
