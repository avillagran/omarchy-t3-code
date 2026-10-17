.pragma library
.import "NativeBrowserPreferences.js" as Original
function empty(){return {signature:"",requestId:"",environment:"",thread:"",tabId:"",profileId:"",busy:false,interactive:false,error:"",url:"",revision:0,width:0,height:0,zoomFactor:1}}
function size(width,height){var n=Original.nativeBrowser,w=Math.max(n.PREVIEW_VIEWPORT_MIN_DIMENSION,Math.min(n.PREVIEW_VIEWPORT_MAX_DIMENSION,Math.round(Number(width)||900))),h=Math.max(n.PREVIEW_VIEWPORT_MIN_DIMENSION,Math.min(n.PREVIEW_VIEWPORT_MAX_DIMENSION,Math.round(Number(height)||620)));if(w*h>n.PREVIEW_VIEWPORT_MAX_AREA){var scale=Math.sqrt(n.PREVIEW_VIEWPORT_MAX_AREA/(w*h));w=Math.floor(w*scale);h=Math.floor(h*scale)}return {width:w,height:h}}
function dismiss(b){var old=b.browserSurface;if(old.requestId&&b.previewWorkerReady)b.send({op:"browser-hide"});b.browserSurface=empty()}
function cleaning(b,environment,profile){var all=b.browserCleanups||{},keys=Object.keys(all);for(var i=0;i<keys.length;i++){var c=all[keys[i]];if(c.environment===environment&&(!profile||c.profileId===profile))return true}return false}
function sync(b,force){
 var w=b.previewWatch,state=b.previewState,snapshot=state&&state.snapshot;
 var active=b.browserPageMounted&&b.panelVisible&&b.previewVisible&&b.previewWorkerReady&&w.connected&&w.canOperate&&snapshot;
 if(!active){dismiss(b);return;}
 if(snapshot.runtime==="server"){dismiss(b);b.browserSurface=Object.assign(empty(),{error:"server_browser_surface_required"});return;}
 var old=b.browserSurface,profile=snapshot.profileId||(old.environment===w.environment&&old.thread===w.thread&&old.tabId===snapshot.tabId?old.profileId:"");
 if(cleaning(b,w.environment,profile)){b.browserSurface=Object.assign({},old,{busy:true,interactive:false,url:"",error:""});return;}
 var signature=JSON.stringify([w.environment,w.originKey,state.serverEpoch,w.thread,snapshot.tabId,snapshot.profileId||null,snapshot.viewport||{_tag:"fill"}]);
 if(!force&&b.browserSurface.signature===signature)return;
 var id=b.uuid(),dimensions=size(b.browserPageWidth,b.browserPageHeight);
 b.browserSurface=Object.assign(empty(),{signature:signature,requestId:id,environment:w.environment,thread:w.thread,tabId:snapshot.tabId,busy:true});
 // No URL, profile or permission supplied by QML: the native coordinator reads
 // the authoritative session and latches the client's configured fallback.
 b.send({op:"browser-show",requestId:id,environment:w.environment,thread:w.thread,tabId:snapshot.tabId,originKey:w.originKey,serverEpoch:state.serverEpoch,width:dimensions.width,height:dimensions.height});
}
function resize(b,width,height){var dimensions=size(width,height);b.browserPageWidth=dimensions.width;b.browserPageHeight=dimensions.height;var snapshot=b.previewState&&b.previewState.snapshot;if(b.browserSurface.requestId&&b.browserSurface.interactive&&snapshot&&(!snapshot.viewport||snapshot.viewport._tag==="fill"))input(b,b.browserSurface.requestId,{type:"resize",width:dimensions.width,height:dimensions.height})}
function input(b,id,event){var s=b.browserSurface;if(!b.browserPageMounted||!b.panelVisible||!b.previewVisible||!b.previewWorkerReady||!s.interactive||s.requestId!==id)return;b.send({op:"browser-input",requestId:id,event:event})}
function receive(b,d){
 if(d.kind==="cleanup-started"||d.kind==="cleanup-ended"){
  var all=Object.assign({},b.browserCleanups||{}),envs=d.environments||[];
  for(var i=0;i<envs.length;i++){var key=JSON.stringify([envs[i],d.profileId]);if(d.kind==="cleanup-started")all[key]={id:d.cleanupId,environment:envs[i],profileId:d.profileId};else if(all[key]&&all[key].id===d.cleanupId)delete all[key]}
  b.browserCleanups=all;
  if(d.kind==="cleanup-started"){
   var old=b.browserSurface;if(cleaning(b,old.environment,old.profileId))b.browserSurface=Object.assign({},old,{busy:true,interactive:false,url:"",error:""});
  }else sync(b,true);
  return;
 }
 var s=b.browserSurface;if(!b.browserPageMounted||!b.panelVisible||!b.previewVisible||!s.requestId||d.requestId!==s.requestId)return;
 if(cleaning(b,s.environment,s.profileId))return;
 if(d.environment&&d.environment!==s.environment||d.thread&&d.thread!==s.thread||d.tabId&&d.tabId!==s.tabId)return;
 if(d.kind==="shown"){b.browserSurface=Object.assign({},s,{interactive:!!d.canOperate,busy:!s.url,error:"",profileId:d.profileId||s.profileId,zoomFactor:d.zoomFactor||s.zoomFactor});return;}
 if(d.kind==="zoom"){b.browserSurface=Object.assign({},s,{zoomFactor:Number(d.value)||1});return;}
 if(d.kind==="frame"&&d.url&&Number(d.revision)>s.revision){b.browserSurface=Object.assign({},s,{url:d.url,revision:Number(d.revision),width:d.width,height:d.height,busy:false});return;}
 if(d.kind==="error"||d.kind==="closed")b.browserSurface=Object.assign({},s,{busy:false,interactive:false,error:d.error||"browser_unavailable"});
}

function control(b,action){var s=b.browserSurface;if(b.browserPageMounted&&b.panelVisible&&b.previewVisible&&b.previewWorkerReady&&s.interactive&&s.requestId)b.send({op:"browser-control",requestId:s.requestId,action:action})}
