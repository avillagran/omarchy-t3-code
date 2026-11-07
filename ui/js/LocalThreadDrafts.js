.pragma library
.import "NativeNewThreadModel.js" as Native

// Local draft identity is separate from its future server thread. No RPC is
// needed to open the composer or seed a prompt; submission uses launchThread.
function hasContent(draft) {
  return !!draft && (!!String(draft.text || "").trim() || (draft.attachments || []).length > 0 || ((draft.context || {}).records || []).length > 0)
}
function metadata(root,key) { return (root.viewState.localDraftThreads || {})[key] || null }
function isLocal(root,key) { return !!metadata(root,key) && !(root.state.threads || []).some(function(thread){return thread.key===key}) }
function present(root,key) {
  var draft=metadata(root,key)
  if(!draft)return ({})
  var project=(root.state.projects || []).find(function(project){return project.environmentId===draft.environmentId && project.id===draft.projectId})
  return Object.assign({},draft,{id:draft.threadId,key:key,projectKey:draft.environmentId+"/"+draft.projectId,project:project?project.title:draft.projectId,createdAt:Date.parse(draft.createdAt),updatedAt:Date.parse(draft.createdAt),modelSelection:(root.drafts[key] || {}).modelSelection || draft.modelSelection || null,latestRunId:null,activeRunId:null,working:false,settled:false,localDraft:true})
}
function store(root,all) { root.rememberView({localDraftThreads:all});root.persistViewNow() }
function patch(root,key,values) {
  var prior=metadata(root,key);if(!prior)return
  var all=Object.assign({},root.viewState.localDraftThreads || {});all[key]=Object.assign({},prior,values);store(root,all)
}
function promoted(root,key) {
  var all=Object.assign({},root.viewState.localDraftThreads || {});if(!all[key])return
  delete all[key];store(root,all)
}
function rememberSelection(root,selection,rememberOptions) {
  if(!selection || !selection.instanceId)return
  var all=Object.assign({},root.viewState.stickyModelSelectionByProvider || {}),prior=all[selection.instanceId]
  all[selection.instanceId]=Object.assign({},selection,{options:(!rememberOptions&&(!selection.options||selection.options.length===0))?(prior?prior.options || []:[]):selection.options})
  var values={stickyActiveProvider:selection.instanceId,stickyModelSelectionByProvider:all}
  if(rememberOptions){
    var byProvider=Object.assign({},root.viewState.stickyOptionsByModelByProvider || {}),models=Object.assign({},byProvider[selection.instanceId] || {})
    if((selection.options || []).length)models[selection.model]=selection.options.slice()
    else delete models[selection.model]
    byProvider[selection.instanceId]=models;values.stickyOptionsByModelByProvider=byProvider
  }
  root.rememberView(values)
}
function selectModel(root,instanceId,model) {
  var remembered=((root.viewState.stickyOptionsByModelByProvider || {})[instanceId] || {})[model]
  var selection={instanceId:instanceId,model:model}
  if(remembered&&remembered.length)selection.options=remembered.slice()
  rememberSelection(root,selection,false)
  root.patchDraft(root.selectedKey,{modelSelection:Object.assign({options:[]},selection),modelSelectionExplicit:true})
}

function create(root,command) {
  if(!root.viewStateLoaded){root.managementError="draft_state_not_loaded";return null}
  var project=(root.state.projects || []).find(function(project){return project.environmentId===command.environment && project.id===command.projectId})
  if(!project){root.managementError="invalid_project";return null}
  var all=Object.assign({},root.viewState.localDraftThreads || {})
  var unloaded=Object.keys(all).find(function(key){var entry=all[key];return entry.environmentId===command.environment && entry.projectId===command.projectId && isLocal(root,key) && (!root.drafts[key] || root.drafts[key].loading)})
  if(unloaded){
    root.pendingLocalDraftCreate={key:unloaded,epoch:root.selectionEpoch,command:Object.assign({},command,{_localCarrySelection:command._localCarrySelection===undefined?(root.draft.modelSelection || root.selectedInfo.modelSelection || null):command._localCarrySelection,_localStickySelection:command._localStickySelection===undefined?((root.viewState.stickyModelSelectionByProvider || {})[root.viewState.stickyActiveProvider] || null):command._localStickySelection})}
    if(!root.drafts[unloaded]){root.patchDraft(unloaded,{loading:true});root.send({op:"draft-load",key:unloaded,projectId:command.projectId})}
    return null
  }
  var reusable=Object.keys(all).find(function(key){var draft=all[key];return draft.environmentId===command.environment && draft.projectId===command.projectId && isLocal(root,key) && root.drafts[key] && !root.drafts[key].loading && !root.drafts[key].loadError && !root.drafts[key].busy && !root.drafts[key].attempt && !hasContent(root.drafts[key])})
  var source=metadata(root,root.selectedKey),carry=command._localCarrySelection===undefined?(root.draft.modelSelection || root.selectedInfo.modelSelection || null):command._localCarrySelection
  var sticky=command._localStickySelection===undefined?((root.viewState.stickyModelSelectionByProvider || {})[root.viewState.stickyActiveProvider] || null):command._localStickySelection
  var key=reusable,created=!key,alreadyOpen=!!key&&root.selectedKey===key
  if(created){
    var creation=root.uuid(),thread="omarchy-thread-"+creation
    key=command.environment+"/"+thread
    all[key]={draftId:root.uuid(),threadReservationId:creation,threadId:thread,environmentId:command.environment,projectId:command.projectId,createdAt:new Date().toISOString(),title:command.title || "New thread",modelSelection:carry || sticky || command.modelSelection || null,runtimeMode:command.runtimeMode || "approval-required",interactionMode:root.interactionMode || "default",branch:command.branch || null,worktreePath:command.worktreePath || null,workspaceStrategy:command.workspaceStrategy || {type:"root"},workspaceExplicit:!!command.workspaceStrategy}
    store(root,all)
    root.patchDraft(key,{loading:false,localDraftId:all[key].draftId,modelSelection:carry || sticky || command.modelSelection || null,modelSelectionExplicit:false,interactionMode:all[key].interactionMode})
  }
  var draft=root.drafts[key],entry=metadata(root,key)
  if(!draft.localDraftId&&!hasContent(draft))root.patchDraft(key,{localDraftId:entry.draftId})
  if(command.text!==undefined && !hasContent(draft))root.patchDraft(key,{text:command.text})
  if(!created && !draft.modelSelectionExplicit){
    var override=source && source.draftId===entry.draftId?null:carry
    if(override || sticky)root.patchDraft(key,{modelSelection:override || sticky,modelSelectionExplicit:false})
  }
  if(command.workspaceStrategy)patch(root,key,{workspaceStrategy:command.workspaceStrategy,workspaceExplicit:true,branch:command.branch || null,worktreePath:command.worktreePath || null})
  root.selectThread(entry.environmentId,entry.threadId,true)
  root.flushDraft(key)
  return {key:key,carrySelection:carry,stickySelection:sticky,sourceDraftId:source?source.draftId:null,draftId:entry.draftId,selectionEpoch:root.selectionEpoch,created:created,alreadyOpen:alreadyOpen}
}
function submission(root) {
  var entry=metadata(root,root.selectedKey)
  if(!entry || root.draft.localDraftId&&root.draft.localDraftId!==entry.draftId)return null
  return {op:"manage",action:"new-thread",requestId:root.uuid(),sendRequestId:root.uuid(),targetThreadId:entry.threadId,localDraftId:entry.draftId,environment:entry.environmentId,thread:entry.threadId,projectId:entry.projectId,title:entry.title,runtimeMode:root.draft.runtimeMode || entry.runtimeMode,interactionMode:root.interactionMode,workspaceStrategy:entry.workspaceStrategy,launchWithMessage:true,generateTitle:true,text:root.draft.text,context:root.messageContext(root.draft),attachments:root.draft.attachments.map(function(file){return file.attachment}),modelSelection:root.draft.modelSelection || entry.modelSelection,delivery:"auto"}
}

function resumeCreate(root,key,error) {
  var pending=root.pendingLocalDraftCreate
  if(!pending || pending.key!==key)return
  root.pendingLocalDraftCreate=null
  if(error){root.managementError=error;return}
  if(root.selectionEpoch!==pending.epoch)return
  root.requestNewThread(pending.command)
}

// Environment routing transfers the same local draft only after both retained
// source and destination documents acknowledge the exact current content.
function ordinary(draft) {
  var value={}
  ;["text","context","attachments","localDraftId","recoveryIds","modelSelection","modelSelectionExplicit","runtimeMode","interactionMode","sourcePlanRef","delivery","queueEdit","questionnaires","approvals"].forEach(function(key){if(draft[key]!==undefined)value[key]=draft[key]})
  return JSON.parse(JSON.stringify(value))
}
function route(root,request,result) {
  var key=request.localDraftKey,entry=metadata(root,key),draft=root.drafts[key]
  if(!entry||!draft||entry.draftId!==request.localInitial.draftId||request.localInitial.selectionEpoch!==root.selectionEpoch)return false
  if((draft.attachments || []).length){root.continueLocalRoute(request,{environment:entry.environmentId,projectId:entry.projectId});return true}
  if(result.environment===entry.environmentId&&result.projectId===entry.projectId){root.continueLocalRoute(request,result);return true}
  var project=(root.state.projects || []).find(function(project){return project.environmentId===result.environment&&project.id===result.projectId})
  var target=result.environment+"/"+entry.threadId
  if(!project||metadata(root,target)||(root.drafts[target]&&root.drafts[target].localDraftId!==entry.draftId)||(root.state.threads || []).some(function(thread){return thread.key===target})){root.managementError="invalid_project";return false}
  var move={from:key,to:target,draftId:entry.draftId,epoch:root.selectionEpoch,entry:entry,request:request,result:result,stage:"source"}
  root.workspaceOperation={busy:true,error:"",action:"draft-moving",request:request}
  saveMove(root,move)
  return true
}
function saveMove(root,move) {
  move.snapshot=ordinary(root.drafts[move.from]);move.stage="source"
  var revision=root.flushDraft(move.from),moves=Object.assign({},root.pendingLocalDraftMoves)
  moves[revision]=move;root.pendingLocalDraftMoves=moves
}
function savedMove(root,data) {
  var move=root.pendingLocalDraftMoves[data.revision]
  if(!move)return false
  var moves=Object.assign({},root.pendingLocalDraftMoves);delete moves[data.revision];root.pendingLocalDraftMoves=moves
  var entry=metadata(root,move.from)
  if(data.error||!entry||entry.draftId!==move.draftId||root.selectionEpoch!==move.epoch||!(root.state.projects || []).some(function(project){return project.environmentId===move.result.environment&&project.id===move.result.projectId})){
    root.workspaceOperation={busy:false,error:data.error || "draft_scope_changed"};return true
  }
  if(JSON.stringify(ordinary(root.drafts[move.from]))!==JSON.stringify(move.snapshot)){saveMove(root,move);return true}
  if(move.stage==="source"){
    root.patchDraft(move.to,Object.assign({},move.snapshot,{loading:false,busy:false,attempt:null,error:""}))
    move.stage="target"
    var revision=root.flushDraft(move.to);moves=Object.assign({},root.pendingLocalDraftMoves);moves[revision]=move;root.pendingLocalDraftMoves=moves
    return true
  }
  var all=Object.assign({},root.viewState.localDraftThreads || {});delete all[move.from]
  all[move.to]=Object.assign({},entry,{environmentId:move.result.environment,projectId:move.result.projectId,workspaceStrategy:{type:"root"},workspaceExplicit:false,branch:null,worktreePath:null})
  store(root,all)
  if(root.selectedKey===move.from)root.selectThread(move.result.environment,entry.threadId,true)
  var request=Object.assign({},move.request,{localDraftKey:move.to,localInitial:Object.assign({},move.request.localInitial,{key:move.to,selectionEpoch:root.selectionEpoch})})
  root.continueLocalRoute(request,move.result)
  return true
}

function cachedDefaults(root,key) {
  var entry=metadata(root,key);if(!entry)return null
  var cached=root.localProjectContexts[entry.environmentId+"/"+entry.projectId]
  if(cached)return cached
  var context=(root.catalogs[entry.environmentId] || {}).newThreadContext
  if(!context)return null
  var defaults=Object.assign({},(context.projectDefaults || {})[entry.projectId] || context.defaults)
  var project=(root.state.projects || []).find(function(project){return project.environmentId===entry.environmentId&&project.id===entry.projectId}) || {}
  if(!context.projectSettingsFolded){
    if(!defaults.modelOverrideConfigured&&project.defaultModelSelection&&Native.resolveProjectProviderInstanceEnabled(context.settings,entry.projectId,project.defaultModelSelection.instanceId))defaults.defaultModelSelection=project.defaultModelSelection
    if(!defaults.modeOverrideConfigured&&project.defaultThreadEnvMode){defaults.mode=project.defaultThreadEnvMode;defaults.modeSource="project";defaults.consultProjectFile=false}
  }
  return Object.assign(defaults,{newProjectsRoot:context.newProjectsRoot,modelDefaults:{providers:context.providers,fallbackSelection:context.fallbackSelection,settings:context.settings}})
}
