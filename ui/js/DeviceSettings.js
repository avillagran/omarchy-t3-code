.pragma library
// Native device contracts/lifecycle live in Rust/T3. This module only manages
// captured UI scope, request nonces and explicit per-environment outcomes.
function ids(view){return Object.keys(view.sessions || {})}
function busy(r){return r&&r.state==="pending"}
function uncertain(r){return r&&r.state==="uncertain"}
function read(b,environment){
 var v=b.deviceView,s=v.sessions[environment];if(!s||s.busy)return
 var all=Object.assign({},v.sessions),id=b.uuid();all[environment]=Object.assign({},s,{busy:b.deviceWorkerReady,error:b.deviceWorkerReady?"":"disconnected",requestId:id});b.deviceView=Object.assign({},v,{sessions:all})
 if(b.deviceWorkerReady)b.send({op:"devices-read",environment:environment,requestId:id})
}
function select(b,targets,force){
 var groups={},bad=false,copy=JSON.parse(JSON.stringify(targets || []));
 copy.forEach(function(t){if(!t||!t.environment||typeof t.environment!=="string"||t.projectId!==null&&typeof t.projectId!=="string"||groups[t.environment]&&groups[t.environment].projectId!==t.projectId){bad=true;return}groups[t.environment]=t})
 var signature=JSON.stringify(copy);if(!force&&signature===b.deviceView.signature)return
 var previous=b.deviceView,sessions={};if(!bad)Object.keys(groups).forEach(function(e){var old=previous.sessions[e];sessions[e]={busy:false,error:"",requestId:"",target:groups[e],data:old?old.data:null}})
 b.deviceView={signature:signature,targets:copy,sessions:sessions,error:bad?"invalid_settings_scope":Object.keys(sessions).length?"":"settings_scope_empty"}
 b.deviceWatchRequest=b.uuid();b.deviceLiveStates={};
 if(b.deviceWorkerReady)b.send({op:"devices-watch",environments:b.panelVisible?ids(b.deviceView):[],requestId:b.deviceWatchRequest})
 ids(b.deviceView).forEach(function(e){read(b,e)})
}
function dismiss(b){b.deviceWatchRequest="";b.deviceLiveStates={};b.deviceView={signature:"",targets:[],sessions:{},error:""};if(b.deviceWorkerReady)b.send({op:"devices-watch",environments:[],requestId:b.uuid()})}
function receiveState(b,d){
 if(!b.panelVisible||d.requestId!==b.deviceWatchRequest||!b.deviceView.sessions[d.environment])return
 var v=b.deviceView,s=v.sessions[d.environment],all=Object.assign({},v.sessions)
 if(d.disconnected){all[d.environment]=Object.assign({},s,{error:"disconnected"});b.deviceView=Object.assign({},v,{sessions:all});return}
 if(!d.state||!d.originKey)return
 var live=Object.assign({},b.deviceLiveStates);live[d.environment]=d;b.deviceLiveStates=live
 if(s.data&&s.data.originKey===d.originKey&&d.state.revision>=s.data.state.revision){all[d.environment]=Object.assign({},s,{data:Object.assign({},s.data,{state:d.state})});b.deviceView=Object.assign({},v,{sessions:all})}
 if(s.error&&!s.busy)read(b,d.environment)
}
function allowed(b,environment,action){
 var s=b.deviceView.sessions[environment],r=b.deviceOperations[environment];
 if(!b.deviceWorkerReady||!s||s.busy||s.error||!s.data||busy(r)||uncertain(r)&&r.originKey===s.data.originKey)return false
 var d=s.data,p=d.permissions,project=s.target.projectId!==null,state=d.state;
 if(["inspect","refresh","update-tool"].indexOf(action)<0&&project)return false
 if(["installing","starting"].indexOf(state.hostStatus)>=0&&action!=="test-host")return false
 if(["hub","agent","host-upsert","host-remove","test-host"].indexOf(action)>=0&&!p.configure)return false
 if(["update-tool","retry-host"].indexOf(action)>=0&&!p.operate)return false
 if(action==="inspect"&&!state.supportsToolInspection||action==="update-tool"&&!state.supportsToolUpdate||action==="retry-host"&&!state.supportsHostRetry)return false
 return true
}
function dispatch(b,action,input,environments){
 var selected=environments || ids(b.deviceView),commands=Object.assign({},b.deviceCommands),operations=Object.assign({},b.deviceOperations),out=[];
 if(!selected.length||selected.some(function(e){return busy(operations[e])}))return false
 selected.forEach(function(environment){
  var s=b.deviceView.sessions[environment];if(!s)return
  if(!allowed(b,environment,action)){if(!uncertain(operations[environment]))operations[environment]={environment:environment,action:action,state:"failed",error:s.error || "device_action_unavailable"};return}
  var id=b.uuid(),command=Object.assign({},JSON.parse(JSON.stringify(input || {})),{op:"devices-action",action:action,environment:environment,projectId:s.target.projectId,viewId:s.data.viewId,requestId:id});
  commands[id]={command:command,originKey:s.data.originKey};operations[environment]={environment:environment,originKey:s.data.originKey,action:action,state:"pending",error:"",requestId:id,input:input || {}};out.push(command)
 });
 b.deviceCommands=commands;b.deviceOperations=operations;out.forEach(function(c){b.send(c)});return out.length>0
}
function receipt(b,d){
 if(d.op==="devices-read"){
  var v=b.deviceView,s=v.sessions[d.environment];if(!s||!s.busy||s.requestId!==d.requestId)return
  var data=d.result,error=d.error || "";
  if(!error&&(!data||data.environment!==d.environment||!data.viewId||!data.originKey||!data.settings||!data.state||!data.permissions))error="invalid_device_state"
  var live=b.deviceLiveStates[d.environment];if(!error&&live&&live.originKey===data.originKey&&live.state.revision>=data.state.revision)data=Object.assign({},data,{state:live.state})
  var all=Object.assign({},v.sessions);all[d.environment]=Object.assign({},s,{busy:false,error:error,data:error?s.data:data});b.deviceView=Object.assign({},v,{sessions:all});return
 }
 var captured=b.deviceCommands[d.requestId];if(!captured||captured.command.environment!==d.environment)return
 var command=captured.command,commands=Object.assign({},b.deviceCommands);delete commands[d.requestId];b.deviceCommands=commands
 var result=d.result,error=d.error || result&&result.error || "",state=d.error?"failed":result&&result.state || "uncertain";
 // An IPC error before the backend's structured outcome may have happened
 // after dispatch; backend preflight errors are the only known safe failures.
 if(d.error&&["device_busy","permission_denied","device_view_expired","device_scope_changed","device_settings_changed_reload","device_environment_only","device_inspection_unsupported","device_update_unsupported","device_host_unavailable","invalid_device_action","invalid_device_host","ambiguous_device_host","device_action_rejected"].indexOf(d.error)<0)state="uncertain"
 if(["complete","failed","uncertain"].indexOf(state)<0)state="uncertain"
 var operations=Object.assign({},b.deviceOperations);operations[d.environment]={environment:d.environment,originKey:captured.originKey,requestId:d.requestId,action:command.action,input:command,state:state,error:error,host:result?result.host:null};b.deviceOperations=operations
 var v=b.deviceView,s=v.sessions[d.environment],data=result&&result.data;
 // Never apply a receipt to another origin or a newer scope read.
 if(s&&s.data&&s.data.originKey===captured.originKey&&s.data.viewId===command.viewId){
  var all=Object.assign({},v.sessions);all[d.environment]=Object.assign({},s,{busy:false,error:"",data:state==="complete"&&data&&data.environment===d.environment&&data.originKey===captured.originKey?data:s.data});b.deviceView=Object.assign({},v,{sessions:all});read(b,d.environment)
 }
}
function acknowledge(b,environment){var r=b.deviceOperations[environment];if(!r||busy(r))return;var all=Object.assign({},b.deviceOperations);delete all[environment];b.deviceOperations=all;read(b,environment)}
function lost(b){
 var all=Object.assign({},b.deviceOperations);Object.keys(all).forEach(function(e){if(busy(all[e]))all[e]=Object.assign({},all[e],{state:"uncertain",error:"disconnected"})});b.deviceOperations=all;b.deviceCommands={};
 var v=b.deviceView,sessions=Object.assign({},v.sessions);ids(v).forEach(function(e){sessions[e]=Object.assign({},sessions[e],{busy:false,error:"disconnected"})});b.deviceView=Object.assign({},v,{sessions:sessions})
}

function visibility(b){
 if(!b.panelVisible){b.deviceWatchRequest="";b.deviceLiveStates={};return}
 if(b.deviceView.targets.length)select(b,b.deviceView.targets,true)
}
