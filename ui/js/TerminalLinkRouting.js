.pragma library
.import "NativeTerminalLinks.js" as Native
.import "NativeBrowserPreferences.js" as Browser
// Transport and pane ownership only. Matching/path resolution use the native helpers.
var paths=Native.create(function(){return []});
function sameScope(b,r){return r.key===b.selectedKey&&r.environment===b.selectedEnvironment&&r.thread===b.selectedThread}
function ready(b){return !!b.selectedKey&&!!b.previewWorkerReady&&(b.state.environments||[]).some(function(e){return e.id===b.selectedEnvironment&&e.state==="ready"})}
function target(b,text,terminalId){
 var project=(b.state.projects||[]).find(function(p){return p.environmentId===b.selectedEnvironment&&p.id===b.selectedInfo.projectId})||{};
 var view=Object.keys(b.terminalViews||{}).map(function(id){return b.terminalViews[id]}).find(function(v){return v.key===b.selectedKey&&v.terminalId===terminalId})||{};
 return paths.resolvePathLinkTarget(String(text),b.selectedInfo.worktreePath||project.workspaceRoot||view.cwd||"");
}
function pathKey(b,value){return JSON.stringify([b.selectedEnvironment,b.selectedThread,value])}
function cache(b,value){return b.terminalPathMetadata[pathKey(b,value)]}
function allowed(b,text,terminalId){
 if(paths.isTerminalUrl(text))return true;
 if(!ready(b)||b.catalog.canWrite===false)return false;
 var entry=cache(b,target(b,text,terminalId));return !!entry&&Date.now()-entry.at<60000&&(entry.kind==="file"||entry.kind==="directory");
}
function failureKey(key,terminal){return JSON.stringify([key,terminal||""])}
function clearFailure(b,key,terminal){var failures=Object.assign({},b.terminalLinkFailures);delete failures[failureKey(key,terminal)];b.terminalLinkFailures=failures}
function fail(b,r,error){if(sameScope(b,r)){var failures=Object.assign({},b.terminalLinkFailures);failures[failureKey(r.key,r.terminalId)]=error||"";b.terminalLinkFailures=failures}}
function requestPath(b,text,open,terminalId){
 if(!ready(b)||b.catalog.canWrite===false)return false;
 var value=target(b,text,terminalId);if(paths.isNetworkPath(value))return false;
 var key=pathKey(b,value),existing=cache(b,value),requests=b.terminalPathRequests;
 if(!open&&existing&&Date.now()-existing.at<60000)return true;
 if(!open&&Object.keys(requests).some(function(id){return requests[id].cacheKey===key&&requests[id].stage==="read"}))return true;
 var id=b.uuid(),r={key:b.selectedKey,environment:b.selectedEnvironment,thread:b.selectedThread,target:value,cacheKey:key,terminalId:terminalId||"",open:!!open,stage:"read"};
 var next=Object.assign({},requests);next[id]=r;b.terminalPathRequests=next;
 b.send({op:"thread-details-read",environment:r.environment,thread:r.thread,target:value,requestId:id});return true;
}
function hover(b,text,terminalId){if(text&&!paths.isTerminalUrl(text))requestPath(b,text,false,terminalId)}
function pathReceipt(b,d){
 var r=b.terminalPathRequests[d.requestId];if(!r)return false;
 var requests=Object.assign({},b.terminalPathRequests);delete requests[d.requestId];b.terminalPathRequests=requests;
 if(!sameScope(b,r)||d.environment!==r.environment||d.thread!==r.thread)return true;
 if(r.stage==="open"){if(d.error)fail(b,r,d.error);else if(!d.result||d.result.opened!==true||d.result.target!==r.target)fail(b,r,"invalid_terminal_link");return true;}
 var result=d.result,link=result&&result.terminalLink;
 var valid=!d.error&&!!result&&result.environment===r.environment&&result.thread===r.thread&&link&&link.target===r.target;
 var kind=valid&&(link.kind==="file"||link.kind==="directory")?link.kind:null;
 var metadata=Object.assign({},b.terminalPathMetadata);delete metadata[r.cacheKey];metadata[r.cacheKey]={kind:kind,at:Date.now(),path:valid?link.path:null};var keys=Object.keys(metadata);while(keys.length>1024)delete metadata[keys.shift()];b.terminalPathMetadata=metadata;
 if(!r.open||!kind)return true;
 var editors=result.editors||[],preferred=(b.viewState||{}).preferredEditor,editor=editors.find(function(e){return e.id===preferred})||editors[0];
 if(!editor){fail(b,r,"editor_unavailable");return true;}
 if(editor.id!==preferred)b.rememberView({preferredEditor:editor.id});
 var id=b.uuid();requests=Object.assign({},b.terminalPathRequests);requests[id]=Object.assign({},r,{stage:"open"});b.terminalPathRequests=requests;
 b.send({op:"thread-details-open-editor",environment:r.environment,thread:r.thread,target:r.target,editor:editor.id,requestId:id});return true;
}
function external(b,r){if(b.openTerminalSystemUrl(r.url)===false)fail(b,r,"terminal_link_unavailable")}
function shift(b){b.terminalUrlQueue=b.terminalUrlQueue.slice(1)}
function publish(b,r){var queue=b.terminalUrlQueue.slice();queue[0]=r;b.terminalUrlQueue=queue}
function open(b,text,forceBrowser,terminalId){
 text=String(text||"");if(!text)return false;clearFailure(b,b.selectedKey,terminalId);
 if(!paths.isTerminalUrl(text))return requestPath(b,text,true,terminalId);
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
function reset(b){b.terminalPathRequests={};b.terminalPathMetadata={};b.terminalUrlQueue=[];b.terminalLinkFailures={}}
