// Filesystem-delete command double: logical removal retains directory contents.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=process.env.INSTALL_TEST_ROOT;
if(!root||!path.isAbsolute(root))throw Error('Private fixture root required');
const args=process.argv.slice(2),targets=[];let recursive=false,force=false,options=true;
for(const arg of args){
 if(options&&arg==='--'){options=false;continue}
 if(options&&arg.startsWith('-')){if(!['--recursive','--force'].includes(arg)&&!/^-[rfFR]+$/.test(arg))throw Error('Unsupported deletion option');recursive ||= arg==='--recursive'||/^-[rfFR]+$/.test(arg)&&/[rR]/.test(arg);force ||= arg==='--force'||/^-[rfFR]+$/.test(arg)&&arg.includes('f');continue}
 targets.push(path.resolve(arg));
}
for(const target of targets){
 if(target===root||!target.startsWith(root+path.sep))throw Error('Deletion double refuses paths outside its fixture');
 let current=root,stat,missing=false;
 for(const part of path.relative(root,target).split(path.sep)){current=path.join(current,part);try{stat=fs.lstatSync(current)}catch(error){if(error.code==='ENOENT'&&force){missing=true;break}throw error}if(current!==target&&stat.isSymbolicLink())throw Error('Fixture deletion cannot follow parent links')}
 if(missing)continue;
 if(stat.isDirectory()&&!stat.isSymbolicLink()){
  if(!recursive)throw Error('Directory requires explicit recursive deletion intent');
  const quarantine=path.join(root,'retained-delete-requests');fs.mkdirSync(quarantine,{recursive:true,mode:0o700});
  const destination=path.join(quarantine,crypto.randomUUID());fs.renameSync(target,destination);
  fs.appendFileSync(path.join(root,'filesystem-delete-log'),JSON.stringify({target,destination})+'\n');
 }else{fs.unlinkSync(target)}
}
