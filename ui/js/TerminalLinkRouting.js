.pragma library
.import "NativeTerminalLinks.js" as Native
.import "NativeBrowserPreferences.js" as Browser
.import "NativeFileMetadata.js" as Metadata
// Transport and pane ownership only. Matching/path resolution use the native helpers.
var paths=Native.create(function(){return []});
var metadataPaths=Metadata.create();
function sameScope(b,r){return r.key===b.selectedKey&&r.environment===b.selectedEnvironment&&r.thread===b.selectedThread}
function ready(b){return !!b.selectedKey&&!!b.previewWorkerReady&&(b.state.environments||[]).some(function(e){return e.id===b.selectedEnvironment&&e.state==="ready"})}
function cwd(b,terminalId,surface){
 var project=(b.state.projects||[]).find(function(p){return p.environmentId===b.selectedEnvironment&&p.id===b.selectedInfo.projectId})||{};
 var view=surface?(b.terminalViews||{})[surface]||{}:Object.keys(b.terminalViews||{}).map(function(id){return b.terminalViews[id]}).find(function(v){return v.key===b.selectedKey&&v.terminalId===terminalId})||{};
 return view.cwd||b.selectedInfo.worktreePath||project.workspaceRoot||"";
}
function target(b,text,terminalId,surface){return metadataPaths.resolveLink(String(text),cwd(b,terminalId,surface))}
function descriptor(b,text,terminalId,surface){var value=target(b,text,terminalId,surface);return {target:value,cwd:cwd(b,terminalId,surface),query:metadataPaths.query(b.selectedEnvironment,value,cwd(b,terminalId,surface),{environmentId:b.selectedEnvironment,threadId:b.selectedThread})}}
function pathKey(b,value,lease){return JSON.stringify([b.selectedEnvironment,b.selectedThread,lease||"",value])}
function cache(b,value,lease){return b.terminalPathMetadata[pathKey(b,value,lease)]}
function fresh(entry,lease){return !!entry&&((entry.kind&&!!lease&&entry.lease===lease)||Date.now()-entry.at<60000)}
function permitted(b){return ready(b)&&b.catalog.canWrite===true&&b.catalog.canReadFiles===true}
function sameSurface(b,r){var view=(b.terminalViews||{})[r.surface];return !r.surface||!!view&&view.key===r.key&&cwd(b,r.terminalId,r.surface)===r.cwd&&(!r.lease||view.metadataLeaseId===r.lease)}
function cancelSurface(b,surface){
 if(!surface)return;var requests=Object.assign({},b.terminalPathRequests);
 Object.keys(requests).forEach(function(id){if(requests[id].surface===surface)delete requests[id]});b.terminalPathRequests=requests;
 b.send({op:"file-metadata-cancel",owner:surface});
}
function refreshMiss(b,text,terminalId,surface,lease){
 if(!permitted(b)||!text||paths.isTerminalUrl(text))return;
 var value=descriptor(b,text,terminalId,surface);if(!value.query)return;
 var entry=cache(b,value.query.input.path,lease);
 if(entry&&entry.kind===null&&!fresh(entry,lease))hover(b,text,terminalId,surface,lease);
}
function refreshPath(b,text,terminalId,surface,lease,path){
 if(path){var value=descriptor(b,text,terminalId,surface);if(!value.query||value.query.input.path!==path)return;}
 hover(b,text,terminalId,surface,lease);
}
function pruneClosed(b){
 var closed=Object.assign({},b.terminalMetadataClosed||{}),requests=b.terminalPathRequests;
 Object.keys(closed).forEach(function(id){if(!Object.keys(requests).some(function(request){return requests[request].stage==="read"&&requests[request].environment===closed[id]}))delete closed[id]});b.terminalMetadataClosed=closed;
}
function metadataEvent(b,d){
 if(d.op==="rpc-session-state"&&d.write===false){
  var sessions=Object.assign({},b.terminalMetadataSessions||{}),closed=Object.assign({},b.terminalMetadataClosed||{});
  if(d.state==="open")sessions[JSON.stringify([d.environment])]=d.sessionId;
  else if(d.state==="closed"){closed[d.sessionId]=d.environment;if(sessions[JSON.stringify([d.environment])]===d.sessionId)delete sessions[JSON.stringify([d.environment])]}
  b.terminalMetadataSessions=sessions;b.terminalMetadataClosed=closed;
 }

 if(d.op==="file-metadata-invalidated"){
  var revisions=Object.assign({},b.terminalMetadataRevisions||{});revisions[JSON.stringify([d.environment,d.path])]=d.revision;b.terminalMetadataRevisions=revisions;
 }
 var metadata=Object.assign({},b.terminalPathMetadata);
 Object.keys(metadata).forEach(function(key){var value=metadata[key];if(d.op==="file-metadata-invalidated"&&value.environment===d.environment&&value.path===d.path||d.op==="rpc-session-state"&&d.state==="closed"&&value.sessionId===d.sessionId||d.op==="rpc-session-state"&&d.state==="open"&&d.write===false&&value.environment===d.environment&&value.sessionId!==d.sessionId)delete metadata[key]});b.terminalPathMetadata=metadata;
 if(d.op==="rpc-session-state"&&d.state==="closed"){
  var requests=Object.assign({},b.terminalPathRequests);Object.keys(requests).forEach(function(id){if(requests[id].sessionId===d.sessionId)delete requests[id]});b.terminalPathRequests=requests;
 }
 if(typeof b.terminalMetadataRefreshRequested==="function"&&(d.op==="file-metadata-invalidated"||d.op==="rpc-session-state"&&d.state==="open"&&d.write===false))b.terminalMetadataRefreshRequested(d.environment,d.op==="file-metadata-invalidated"?d.path:"");
 pruneClosed(b);
}
function allowed(b,text,terminalId,surface,lease){
 if(paths.isTerminalUrl(text))return true;
 if(!permitted(b))return false;
 var value=descriptor(b,text,terminalId,surface);if(!value.query)return false;
 var entry=cache(b,value.query.input.path,lease);return fresh(entry,lease)&&(entry.kind==="file"||entry.kind==="directory");
}
function failureKey(key,terminal){return JSON.stringify([key,terminal||""])}
function clearFailure(b,key,terminal){var failures=Object.assign({},b.terminalLinkFailures);delete failures[failureKey(key,terminal)];b.terminalLinkFailures=failures}
function fail(b,r,error){if(sameScope(b,r)){var failures=Object.assign({},b.terminalLinkFailures);failures[failureKey(r.key,r.terminalId)]=error||"";b.terminalLinkFailures=failures}}
function requestPath(b,text,open,terminalId,surface,lease){
 if(!permitted(b))return false;
 var value=descriptor(b,text,terminalId,surface);if(!value.query)return false;
 var path=value.query.input.path,key=pathKey(b,path,lease),existing=cache(b,path,lease),requests=b.terminalPathRequests;
 if(!open&&fresh(existing,lease))return true;
 if(!open&&Object.keys(requests).some(function(id){return requests[id].cacheKey===key&&requests[id].surface===(surface||"")&&requests[id].stage==="read"}))return true;
 var id=b.uuid(),r={key:b.selectedKey,environment:b.selectedEnvironment,thread:b.selectedThread,target:value.target,cwd:value.cwd,path:path,cacheKey:key,terminalId:terminalId||"",surface:surface||"",lease:lease||"",open:!!open,stage:"read"};
 var next=Object.assign({},requests);next[id]=r;b.terminalPathRequests=next;
 b.send({op:"thread-details-read",environment:r.environment,thread:r.thread,target:r.target,cwd:r.cwd,path:path,leaseId:r.lease,owner:r.surface,requestId:id});return true;
}
function hover(b,text,terminalId,surface,lease){if(text&&!paths.isTerminalUrl(text))requestPath(b,text,false,terminalId,surface,lease)}
function pathReceipt(b,d){
 var r=b.terminalPathRequests[d.requestId];if(!r)return false;
 var requests=Object.assign({},b.terminalPathRequests);delete requests[d.requestId];b.terminalPathRequests=requests;
 try {
 if(!sameScope(b,r)||!sameSurface(b,r)||!permitted(b)||d.environment!==r.environment||d.thread!==r.thread)return true;
 if(r.stage==="open"){if(d.error)fail(b,r,d.error);else if(!d.result||d.result.opened!==true||d.result.target!==r.target)fail(b,r,"invalid_terminal_link");return true;}
 var result=d.result,link=result&&result.terminalLink;
 var valid=!d.error&&!!result&&result.environment===r.environment&&result.thread===r.thread&&link&&link.target===r.target&&link.path===r.path;
 var kind=valid&&(link.kind==="file"||link.kind==="directory")?link.kind:null;
 if(link&&link.sessionId&&((b.terminalMetadataClosed||{})[link.sessionId]||((b.terminalMetadataSessions||{})[JSON.stringify([r.environment])]&&(b.terminalMetadataSessions||{})[JSON.stringify([r.environment])]!==link.sessionId)))return true;
 if(!valid){var failed=Object.assign({},b.terminalPathMetadata);delete failed[r.cacheKey];b.terminalPathMetadata=failed;return true;}
 var revision=(b.terminalMetadataRevisions||{})[JSON.stringify([r.environment,link.path])]||0;
 if(link.revision!==undefined&&link.revision<revision)return true;
 var metadata=Object.assign({},b.terminalPathMetadata);delete metadata[r.cacheKey];metadata[r.cacheKey]={kind:kind,at:Date.now(),path:link.path,lease:r.lease,environment:r.environment,sessionId:link.sessionId};var keys=Object.keys(metadata);while(keys.length>1024)delete metadata[keys.shift()];b.terminalPathMetadata=metadata;
 if(!r.open||!kind)return true;
 var editors=(b.catalog||{}).editors||result.editors||[],preferred=(b.viewState||{}).preferredEditor,editor=editors.find(function(e){return e.id===preferred})||editors[0];
 if(!editor){fail(b,r,"editor_unavailable");return true;}
 if(editor.id!==preferred)b.rememberView({preferredEditor:editor.id});
 var id=b.uuid();requests=Object.assign({},b.terminalPathRequests);requests[id]=Object.assign({},r,{stage:"open"});b.terminalPathRequests=requests;
 b.send({op:"thread-details-open-editor",environment:r.environment,thread:r.thread,target:r.target,cwd:r.cwd,path:r.path,leaseId:r.lease,owner:r.surface,editor:editor.id,requestId:id});return true;
 } finally {pruneClosed(b)}
}
function external(b,r){if(b.openTerminalSystemUrl(r.url)===false)fail(b,r,"terminal_link_unavailable")}
function shift(b){b.terminalUrlQueue=b.terminalUrlQueue.slice(1)}
function publish(b,r){var queue=b.terminalUrlQueue.slice();queue[0]=r;b.terminalUrlQueue=queue}
function open(b,text,forceBrowser,terminalId,surface,lease){
 text=String(text||"");if(!text)return false;clearFailure(b,b.selectedKey,terminalId);
 if(!paths.isTerminalUrl(text))return requestPath(b,text,true,terminalId,surface,lease);
 var r={key:b.selectedKey,environment:b.selectedEnvironment,thread:b.selectedThread,url:text,terminalId:terminalId||"",stage:"settings",requested:false};
 if(forceBrowser||!ready(b)){external(b,r);return true;}
 b.terminalUrlQueue=b.terminalUrlQueue.concat([r]);pump(b);return true;
}
function pump(b){
 if(!b.terminalUrlQueue.length)return;
 var r=Object.assign({},b.terminalUrlQueue[0]);
 if(!sameScope(b,r)||!b.previewWorkerReady||!b.panelVisible){shift(b);return;}
 if(r.stage==="settings"){
  if(b.clientSettings.busy)return;
  if(!r.requested){r.requested=true;publish(b,r);b.requestClientSettings();return;}
  if(b.clientSettings.error||(b.clientSettings.data||{}).browserLinkTarget!=="app"){shift(b);external(b,r);return;}
  var data=b.clientSettings.data||{},profiles=Browser.nativeBrowser.resolveBrowserProfiles(data.browserProfiles||[]),selected=profiles.find(function(p){return p.id===data.browserDefaultProfileId&&p.kind!=="incognito"});
  r.profileId=selected?selected.id:"default";r.viewport=data.browserDefaultViewport||{_tag:"fill"};r.stage="watch";publish(b,r);b.terminalBrowserRequested();return;
 }
 if(r.stage!=="watch")return;
 var watch=b.previewWatch||{},state=b.previewState,operation=(b.previewOperations||{})[JSON.stringify([r.environment,r.thread])];
 if(!b.previewVisible||watch.environment!==r.environment||watch.thread!==r.thread||watch.busy)return;
 if(!watch.connected||!watch.canOperate||!state||!state.listLoaded||operation&&operation.state==="uncertain"){shift(b);external(b,r);return;}
 if(operation&&operation.state==="pending")return;
 if(!b.previewAction("open",{address:r.url,profileId:r.profileId,viewport:r.viewport})){shift(b);external(b,r);return;}
 var accepted=b.previewOperations[JSON.stringify([r.environment,r.thread])];r.previewRequestId=accepted&&accepted.requestId;r.stage="receipt";publish(b,r);
}
function previewReceipt(b,d){
 if(!b.terminalUrlQueue.length)return;
 var r=b.terminalUrlQueue[0];if(r.stage!=="receipt"||r.previewRequestId!==d.requestId)return;
 shift(b);if(!sameScope(b,r))return;
 if(d.environment!==r.environment||d.thread!==r.thread)return;
 var operation=(b.previewOperations||{})[JSON.stringify([r.environment,r.thread])];
 if(d.error||!d.result||d.result.state!=="complete"||!d.result.snapshot||!operation||operation.requestId!==r.previewRequestId||operation.state!=="complete")external(b,r);
}
function reset(b){var owners={};Object.keys(b.terminalPathRequests).forEach(function(id){var owner=b.terminalPathRequests[id].surface;if(owner)owners[owner]=true});Object.keys(owners).forEach(function(owner){b.send({op:"file-metadata-cancel",owner:owner})});b.terminalPathRequests={};b.terminalMetadataSessions={};b.terminalMetadataClosed={};b.terminalPathMetadata={};b.terminalUrlQueue=[];b.terminalLinkFailures={}}
