.pragma library
// UI admission/receipts only. Native profile data cleanup is owned by T3.
function select(b,targets,force){
 var ids=[],bad=false;(targets || []).forEach(function(t){var id=typeof t==="string"?t:t&&t.environment;if(typeof id!=="string"||!id){bad=true;return}if(ids.indexOf(id)<0)ids.push(id)})
 var signature=JSON.stringify(ids),old=b.browserProfileView;if(!force&&old.signature===signature&&(old.busy||old.data&&!old.error))return
 var request=b.uuid();b.browserProfileView={environments:ids,signature:signature,requestId:request,busy:!bad&&ids.length>0&&b.deviceWorkerReady,error:bad?"invalid_settings_scope":!ids.length?"settings_scope_empty":b.deviceWorkerReady?"":"disconnected",data:signature===old.signature?old.data:null}
 if(!bad&&ids.length&&b.deviceWorkerReady)b.send({op:"browser-profiles-read",requestId:request,environments:ids})
}
function dispatch(b,action,profile){
 var v=b.browserProfileView,op=b.browserProfileOperation;
 if(!b.deviceWorkerReady||v.busy||v.error||!v.data||!v.data.available||op.state==="pending"||op.state==="uncertain"||["clear","remove"].indexOf(action)<0||!profile||action==="remove"&&profile.id==="default")return false
 if(!(v.data.profiles || []).some(function(p){return p.id===profile.id&&p.name===profile.name}))return false
 var command={op:"browser-profiles-action",requestId:b.uuid(),viewId:v.data.viewId,profileId:profile.id,action:action,confirmed:true};
 b.browserProfileOperation={state:"pending",error:"",requestId:command.requestId,profileId:profile.id,name:profile.name,action:action,environments:v.environments.slice(),signature:v.signature,results:[]};b.send(command);return true
}
function receipt(b,d){
 if(d.op==="browser-profiles-read"){
  var v=b.browserProfileView;if(!v.busy||v.requestId!==d.requestId)return
  var r=d.result,error=d.error || "";
  if(!error&&(!r||!r.viewId||!Array.isArray(r.sessions)||!Array.isArray(r.profiles)||JSON.stringify(r.sessions.map(function(s){return s.environment}))!==v.signature))error="browser_profile_scope_changed"
  b.browserProfileView=Object.assign({},v,{busy:false,error:error,data:error?null:r});return
 }
 var op=b.browserProfileOperation;if(op.state!=="pending"||op.requestId!==d.requestId)return
 var r=d.result,error=d.error || r&&r.error || "",state=r&&r.state || "uncertain",results=r&&r.results || [];
 if(d.error){state=["confirmation_required","browser_profiles_busy","invalid_browser_profile_action","browser_profile_view_expired","browser_profile_unavailable","browser_profile_builtin","browser_profile_changed_reload","browser_profile_environment_unavailable","browser_profile_scope_changed","browser_profile_cleanup_unavailable","permission_denied"].indexOf(d.error)>=0?"failed":"uncertain"}
 if(!d.error&&(!r||r.profileId!==op.profileId||r.action!==op.action||!Array.isArray(results)||results.length!==op.environments.length||!results.every(function(s,i){return s.environment===op.environments[i]}))){state="uncertain";error="browser_profile_invalid_result"}
 if(["complete","failed","uncertain"].indexOf(state)<0)state="uncertain"
 b.browserProfileOperation=Object.assign({},op,{state:state,error:error,results:results});b.browserProfileNeedsRefresh=true
 if(b.browserProfileView.signature===op.signature)select(b,op.environments,true)
}
function acknowledge(b){if(b.browserProfileOperation.state==="pending")return;b.browserProfileOperation={state:"",error:"",results:[]};select(b,b.browserProfileView.environments,true)}
function dismiss(b){b.browserProfileView={signature:"",environments:[],requestId:"",busy:false,error:"",data:null}}
function lost(b){if(b.browserProfileOperation.state==="pending")b.browserProfileOperation=Object.assign({},b.browserProfileOperation,{state:"uncertain",error:"disconnected"});b.browserProfileView=Object.assign({},b.browserProfileView,{busy:false,error:"disconnected"})}
