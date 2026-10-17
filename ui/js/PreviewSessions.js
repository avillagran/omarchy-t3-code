.pragma library
.import "NativePreviewState.js" as Original
// UI admission/nonces only. Session reconciliation is the original T3 reducer.
function sync(b,force){
 var active=b.panelVisible&&b.previewVisible&&b.previewWorkerReady&&b.selectedEnvironment&&b.selectedThread;
 var signature=active?JSON.stringify([b.selectedEnvironment,b.selectedThread]):"";
 if(!force&&b.previewWatch.signature===signature)return;
 var id=b.uuid();b.previewWatch={signature:signature,requestId:id,environment:active?b.selectedEnvironment:"",thread:active?b.selectedThread:"",originKey:"",busy:!!active,connected:false,error:"",canOperate:false,serverBrowser:false};b.previewState=null;
 if(b.previewWorkerReady)b.send({op:"preview-watch",environment:active?b.selectedEnvironment:"",thread:active?b.selectedThread:"",requestId:id});
}
function receive(b,d){
 var w=b.previewWatch;
 if(!b.panelVisible||!b.previewVisible||!w.signature||d.requestId!==w.requestId||d.environment!==w.environment||d.thread!==w.thread)return;
 if(d.kind==="disconnected"){b.previewWatch=Object.assign({},w,{busy:false,connected:false,error:d.error||"preview_unavailable",canOperate:false});return;}
 if(!d.originKey||!d.result||["list","event"].indexOf(d.kind)<0)return;
 // A list establishes a new origin/epoch; events may never establish one.
 if(d.kind==="event"&&d.originKey!==w.originKey)return;
 var key=JSON.stringify([d.environment,d.originKey]),stores=b.previewStores,store=stores[key];
 if(!store){store=Original.createPreviewStore();var copy=Object.assign({},stores),keys=Object.keys(copy);if(keys.length>=128)delete copy[keys[0]];copy[key]=store;b.previewStores=copy;}
 var ref={environmentId:d.environment,threadId:d.thread};
 if(d.kind==="list")store.reconcilePreviewServerSessions(ref,d.result);else store.applyPreviewServerEvent(ref,d.result);
 b.previewState=store.readThreadPreviewState(ref);
 b.previewWatch=Object.assign({},w,{originKey:d.originKey,busy:false,connected:true,error:"",canOperate:!!d.canOperate,serverBrowser:!!d.serverBrowser});
}
function operationKey(environment,thread){return JSON.stringify([environment,thread])}
function publish(b,store,ref,origin){var w=b.previewWatch;if(w.environment===ref.environmentId&&w.thread===ref.threadId&&w.originKey===origin)b.previewState=store.readThreadPreviewState(ref)}
function command(b,action,options){
 var w=b.previewWatch,state=b.previewState,key=operationKey(w.environment,w.thread),previous=b.previewOperations[key];
 if(!b.panelVisible||!b.previewVisible||!b.previewWorkerReady||!w.connected||!w.canOperate||!state||!state.listLoaded||previous&&["pending","uncertain"].indexOf(previous.state)>=0)return false;
 var id=b.uuid(),input=Object.assign({},options||{},{op:"preview-action",action:action,environment:w.environment,thread:w.thread,requestId:id,originKey:w.originKey,serverEpoch:state.serverEpoch});
 var closing=action==="close"?Object.keys(state.sessions).filter(function(tab){return !input.tabId||tab===input.tabId}).map(function(tab){return state.sessions[tab]}):[];
 var captured={input:input,closing:closing},commands=Object.assign({},b.previewCommands);commands[id]=captured;b.previewCommands=commands;
 var operations=Object.assign({},b.previewOperations);operations[key]={requestId:id,action:action,state:"pending",error:"",originKey:input.originKey,serverEpoch:input.serverEpoch,closingTabIds:closing.map(function(snapshot){return snapshot.tabId})};b.previewOperations=operations;
 var store=b.previewStores[JSON.stringify([w.environment,w.originKey])],ref={environmentId:w.environment,threadId:w.thread};
 if(store){closing.forEach(function(snapshot){store.beginPreviewSessionClose(ref,snapshot.tabId)});publish(b,store,ref,w.originKey)}
 b.send(input);return true;
}
function receipt(b,d){
 var captured=b.previewCommands[d.requestId];if(!captured||d.environment!==captured.input.environment||d.thread!==captured.input.thread)return;
 var input=captured.input,commands=Object.assign({},b.previewCommands);delete commands[d.requestId];b.previewCommands=commands;
 var result=d.result,error=d.error||result&&result.error||"",state=result&&result.state;
 if(d.error)state=["panel_hidden","invalid_preview_address","invalid_preview_command","unknown_environment","preview_busy","preview_scope_changed","preview_server_restarted","preview_tab_closed","permission_denied","server_browser_unavailable","preview_command_replayed","preview_command_limit"].indexOf(d.error)>=0?"failed":"uncertain";
 if(!d.error&&(!result||result.environment!==input.environment||result.thread!==input.thread||result.originKey!==input.originKey||result.serverEpoch!==input.serverEpoch||result.action!==input.action||["complete","uncertain"].indexOf(state)<0)){state="uncertain";error="preview_command_uncertain"}
 var operations=Object.assign({},b.previewOperations),key=operationKey(input.environment,input.thread);operations[key]={requestId:input.requestId,action:input.action,state:state,error:error,originKey:input.originKey,serverEpoch:input.serverEpoch,closingTabIds:captured.closing.map(function(snapshot){return snapshot.tabId})};b.previewOperations=operations;
 var store=b.previewStores[JSON.stringify([input.environment,input.originKey])],ref={environmentId:input.environment,threadId:input.thread};
 if(!store||store.readThreadPreviewState(ref).serverEpoch!==input.serverEpoch)return;
 if(state==="failed")captured.closing.forEach(function(snapshot){store.cancelPreviewSessionClose(ref,snapshot,snapshot.tabId)});
 if(state==="complete"&&result.snapshot){if(input.action==="open")store.applyPreviewServerSnapshot(ref,result.snapshot);else store.updatePreviewServerSnapshot(ref,result.snapshot)}
 publish(b,store,ref,input.originKey);
}
function acknowledge(b){
 var w=b.previewWatch,key=operationKey(w.environment,w.thread),previous=b.previewOperations[key];if(!previous||previous.state==="pending")return;
 // After an uncertain close, let the fresh authoritative list decide whether
 // the tab still exists. Clear suppression without restoring an old snapshot.
 var store=b.previewStores[JSON.stringify([w.environment,previous.originKey])],ref={environmentId:w.environment,threadId:w.thread};
 if(previous.state==="uncertain"&&store&&store.readThreadPreviewState(ref).serverEpoch===previous.serverEpoch)(previous.closingTabIds||[]).forEach(function(tab){store.cancelPreviewSessionClose(ref,null,tab)});
 var operations=Object.assign({},b.previewOperations);delete operations[key];b.previewOperations=operations;sync(b,true);
}
function lost(b){
 b.previewWatch=Object.assign({},b.previewWatch,{connected:false,busy:false,error:"disconnected",canOperate:false});
 var operations=Object.assign({},b.previewOperations);Object.keys(operations).forEach(function(k){if(operations[k].state==="pending")operations[k]=Object.assign({},operations[k],{state:"uncertain",error:"disconnected"})});b.previewOperations=operations;b.previewCommands={};
}

function activate(b,tab){var w=b.previewWatch,store=b.previewStores[JSON.stringify([w.environment,w.originKey])];if(!store)return;var ref={environmentId:w.environment,threadId:w.thread};store.setActivePreviewTab(ref,tab);b.previewState=store.readThreadPreviewState(ref)}
