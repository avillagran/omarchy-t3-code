// Native Pierre component, matching T3 Code's pinned @pierre/diffs 1.5.2.
import {FileDiff, parsePatchFiles} from '@pierre/diffs';
const input=window.__t3Diff;
const mount=document.querySelector('diffs-container');
let instance;
function status(error='') {
 const shadow=mount.shadowRoot;
 const button=shadow?.querySelector('[data-expand-button]');
 const bounds=button?.getBoundingClientRect();
 parent.postMessage({type:'t3-diff-state',contextLoader:typeof instance?.options.loadDiffFiles==='function',version:'1.5.2',error,partial:instance?.fileDiff?.isPartial ?? true,
   lines:shadow?.querySelectorAll('[data-line]').length ?? 0,
   tokens:shadow?.querySelectorAll('span[style]').length ?? 0,
   expand:bounds?{x:bounds.x+bounds.width/2,y:bounds.y+bounds.height/2}:null},'*');
}
try {
 const files=parsePatchFiles(input.patch,input.cacheKey,true).flatMap(p=>p.files);
 const file=files.find(f=>f.name===input.path);
 if(!file)throw Error('review_file_changed');
 const options={diffStyle:input.layout==='split'?'split':'unified',overflow:input.wrap?'wrap':'scroll',
   theme:input.appearance==='light'?'pierre-light':'pierre-dark',themeType:input.appearance,
   lineDiffType:'none',hunkSeparators:'line-info',expansionLineCount:20,
   disableFileHeader:true,unsafeCSS:':host{--diffs-font-family:monospace;--diffs-font-size:13px}pre{margin:0}',
   ...(input.canLoadContext?{loadDiffFiles:async f=>{
     if(!input.full)throw Error(input.fullError || 'review_context_unavailable');
     return {oldFile:f.type==='rename-pure'?null:{name:input.oldPath,contents:input.full.oldContents},newFile:{name:input.path,contents:input.full.newContents}};
   }}:{}),onPostRender:()=>requestAnimationFrame(()=>status())};
 addEventListener('message',event=>{
  if(event.source!==parent || event.data?.type!=='t3-review-options')return;
  const next=event.data;if(!['split','stacked'].includes(next.layout)||typeof next.wrap!=='boolean')return;
  options.diffStyle=next.layout==='split'?'split':'unified';options.overflow=next.wrap?'wrap':'scroll';
  instance.setOptions({...options});instance.rerender();
 });
 instance=new FileDiff(options);
 instance.render({fileDiff:file,fileContainer:mount});
 // No privileged bridge: this page only receives the native read snapshot.
 window.__t3DiffInstance=instance;
}catch(error){document.querySelector('#error').textContent=String(error.message || error);status(String(error.message || error));}
