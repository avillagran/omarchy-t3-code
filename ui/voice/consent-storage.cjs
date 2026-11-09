// A private Storage adapter for the unchanged native consent producer. Only
// consent metadata enters this namespace; ordinary preferences have no handle.
'use strict';
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
// Chromium152 components/services/storage/dom_storage/dom_storage_constants.h.
// This bounds an individual record, not a claim of Chromium's shared quota or
// multi-client quota accounting. There is no shorter grant/EnvironmentId limit.
const MAX_BYTES=10*1024*1024;
const AUDIO='t3code:voice-audio-grant:v1',CLEANUP='t3code:voice-cleanup-grant:v1';
function failure(){return Error('Private consent storage unavailable');}
function allowed(key){return typeof key==='string'&&(key===AUDIO||key===CLEANUP||key.startsWith(AUDIO+':saved:'));}
function encode(key){if(!allowed(key))throw failure();return Buffer.from(key,'utf16le');}
function filename(bytes){return crypto.createHash('sha256').update(bytes).digest('hex')+'.consent';}
function fileIdentity(s){return s.isFile()&&s.nlink===1n&&s.uid===BigInt(process.geteuid())&&(s.mode&0o7777n)===0o600n;}
function dirIdentity(s){return s.isDirectory()&&s.uid===BigInt(process.geteuid())&&(s.mode&0o7777n)===0o700n;}
class ConsentStorage {
 #directory;#fd;#identity;#watch;#closed=false;
 constructor(directory,onChange=()=>{}){
  if(!path.isAbsolute(directory)||fs.realpathSync(directory)!==directory)throw failure();
  const before=fs.lstatSync(directory,{bigint:true});if(!dirIdentity(before))throw failure();
  this.#fd=fs.openSync(directory,fs.constants.O_RDONLY|fs.constants.O_DIRECTORY|fs.constants.O_NOFOLLOW|fs.constants.O_CLOEXEC);
  this.#directory=directory;this.#identity=fs.fstatSync(this.#fd,{bigint:true});
  try{this.#verify();}catch(error){fs.closeSync(this.#fd);throw error;}
  this.#watch=fs.watch('/proc/self/fd/'+this.#fd,{persistent:false},()=>onChange());
  this.#watch.on('error',()=>{this.#closed=true;onChange();});
 }
 #verify(){
  if(this.#closed)throw failure();
  const current=fs.lstatSync(this.#directory,{bigint:true}),held=fs.fstatSync(this.#fd,{bigint:true});
  if(fs.realpathSync(this.#directory)!==this.#directory||!dirIdentity(current)||!dirIdentity(held)||current.dev!==this.#identity.dev||current.ino!==this.#identity.ino||held.dev!==current.dev||held.ino!==current.ino)throw failure();
 }
 #entry(key){this.#verify();const bytes=encode(key);return {bytes,name:'/proc/self/fd/'+this.#fd+'/'+filename(bytes)};}
 getItem(key){
  const {bytes,name}=this.#entry(key);let fd;
  try{fd=fs.openSync(name,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW|fs.constants.O_CLOEXEC);}
  catch(error){if(error.code==='ENOENT')return null;throw failure();}
  try{
   const before=fs.fstatSync(fd,{bigint:true});
   if(!fileIdentity(before)||before.size<12||before.size>MAX_BYTES+12)throw failure();
   const data=fs.readFileSync(fd),after=fs.fstatSync(fd,{bigint:true});
   if(!fileIdentity(after)||before.dev!==after.dev||before.ino!==after.ino||before.size!==after.size||before.mtimeNs!==after.mtimeNs||before.ctimeNs!==after.ctimeNs||BigInt(data.length)!==before.size)throw failure();
   const keyLength=data.readUInt32LE(4),valueLength=data.readUInt32LE(8);
   if(data.toString('ascii',0,4)!=='OVS1'||keyLength!==bytes.length||valueLength%2||12+keyLength+valueLength!==data.length||!data.subarray(12,12+keyLength).equals(bytes))throw failure();
   this.#verify();return data.toString('utf16le',12+keyLength);
  }finally{fs.closeSync(fd);}
 }
 setItem(key,value){
  if(typeof value!=='string')value=String(value);
  const {bytes,name}=this.#entry(key),encoded=Buffer.from(value,'utf16le');
  if(bytes.length+encoded.length>MAX_BYTES)throw failure();
  // Refuse a substituted linked/corrupt entry before replacing it. A failed
  // staging operation is retained; no cleanup traverses this namespace.
  this.getItem(key);
  const temporary=name+'.pending-'+crypto.randomUUID(),data=Buffer.alloc(12+bytes.length+encoded.length);
  data.write('OVS1');data.writeUInt32LE(bytes.length,4);data.writeUInt32LE(encoded.length,8);bytes.copy(data,12);encoded.copy(data,12+bytes.length);
  const fd=fs.openSync(temporary,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_NOFOLLOW|fs.constants.O_CLOEXEC,0o600);
  try{fs.writeFileSync(fd,data);fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
  this.#verify();fs.renameSync(temporary,name);fs.fsyncSync(this.#fd);this.#verify();
 }
 removeItem(key){
  const {name}=this.#entry(key);if(this.getItem(key)===null)return;
  // Exact-key tombstone only. Retention makes fixture and metadata removal
  // reviewable and never deletes an arbitrary path supplied by a grant ID.
  try{fs.renameSync(name,name+'.removed-'+crypto.randomUUID());fs.fsyncSync(this.#fd);}
  catch(error){if(error.code!=='ENOENT')throw failure();}
  this.#verify();
 }
 close(){if(this.#fd===undefined)return;this.#closed=true;this.#watch?.close();fs.closeSync(this.#fd);this.#fd=undefined;}
}
module.exports={ConsentStorage,AUDIO,CLEANUP,MAX_BYTES};
