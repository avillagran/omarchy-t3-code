pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import "../js/DocumentTheme.js" as DocumentTheme
import "../js/SidebarModel.js" as Sidebar

Singleton {
  id: root
  readonly property string binary: Qt.resolvedUrl("../../bin/omarchy-t3-client").toString().replace(/^file:\/\//, "")
  property var state: ({threads: [], environments: [], working: 0, pending: 0})
  property var inboxReturnState:({working:null,returned:{}})
  function observeInboxReturns(){inboxReturnState=(viewState || {}).sidebarCompact?Sidebar.observeReturns(inboxReturnState,state.threads || [],Date.now()):{working:null,returned:{}}}
  onStateChanged:{observeInboxReturns();finishProjectOpen()}
  onViewStateChanged:observeInboxReturns()
  property var backendStart:({busy:false,error:"",status:""})
  property var localBackend:({owner:"unknown",running:false,canControl:false,installed:false})
  function refreshBackendStatus(){send({op:"backend-status"})}
  function controlBackend(action){if(backendStart.busy)return;if(!worker.running){backendStart={busy:false,error:"disconnected",status:""};return;}backendStart={busy:true,error:"",status:""};send({op:"backend-control",action:action,requestId:uuid()})}
  signal copyFinished(string requestId,string error)
  function copyText(value){var id=uuid();send({op:"copy",kind:"text",text:value,requestId:id});return id}
  function copyImage(attachment){var id=uuid();send({op:"copy",kind:"image",environment:selectedEnvironment,attachment:attachment,requestId:id});return id}
  function startBackend(){controlBackend("start")}
  property int localRecoveryAttempts:0
  function recoverLocalBackend(){localRecoveryAttempts=30;refresh()}
  function acceptBackendStatus(value){
    var changed=value.running && (!localBackend.running || value.pid!==localBackend.pid)
    localBackend=value
    if(!value.running&&backendStart.status==="stopped")localRecoveryAttempts=0
    if(changed)recoverLocalBackend()
  }
  function finishLocalRecovery(){
    if(!localRecoveryAttempts || !(state.environments || []).some(function(e){return e.id==="local"&&e.state==="ready"}))return
    localRecoveryAttempts=0
    var all=Object.assign({},catalogs);delete all.local;catalogs=all
    var pending=Object.assign({},catalogLoading);delete pending.local;catalogLoading=pending
    ensureCatalog("local")
    if(selectedEnvironment==="local")fetchMessages()
  }
  // Service activation can finish before its HTTP listener/runtime file is ready.
  // Retry only during recovery, retaining the current transcript and draft.
  Timer{interval:1000;repeat:true;running:root.panelVisible&&root.localRecoveryAttempts>0;onTriggered:{root.localRecoveryAttempts--;root.refreshBackendStatus();root.refresh()}}
  Timer{interval:3000;repeat:true;triggeredOnStart:true;running:root.visiblePanels>0;onTriggered:root.refreshBackendStatus()}
  property var settingsResult:({busy:false,error:"",fields:[],environment:""})
  property var connectionSettings:({busy:false,error:"",environments:[],revision:""})
  property var connectionSettingsCommand:null
  property bool connectionChanging:false
  function manageConnections(action,values){
    if(connectionSettings.busy)return
    var command=Object.assign({op:"connections-manage",action:action,revision:connectionSettings.revision,requestId:uuid()},values || {})
    connectionSettingsCommand={action:action,environmentId:command.environmentId,enabled:command.enabled};connectionSettings=Object.assign({},connectionSettings,{busy:true,error:"",requestId:command.requestId});send(command)
  }
  function requestSettings(environment,key,value){if(settingsResult.busy)return;var id=uuid();settingsResult=Object.assign({},settingsResult,{busy:true,error:"",requestId:id,environment:environment,fields:settingsResult.environment===environment?settingsResult.fields:[]});send({op:"settings",action:key?"update":"read",key:key || "",value:value,environment:environment,requestId:id})}
  property var prOperations:({})
  function requestPullRequests(action,values){var id=uuid(),all=Object.assign({},prOperations);all[action]={busy:true,requestId:id,error:"",data:null};prOperations=all;send(Object.assign({op:"pull-requests",environment:selectedEnvironment,action:action,requestId:id},values || {}))}
  function resetPullRequests(){prOperations=({})}
  property var scrollPolicy:({touchpadNative:false,mouseNative:false})
  property var favoritePending:({})
  property string favoriteError:""
  function refreshCatalog(){if(selectedEnvironment)send({op:"catalog",environment:selectedEnvironment})}
  function toggleModelFavorite(provider,model){toggleModelFavoriteIn(selectedEnvironment,provider,model)}
  function toggleModelFavoriteIn(environment,provider,model){
    var targetCatalog=catalogs[environment] || ({});var key=environment+"/"+JSON.stringify([provider,model]);if(favoritePending[key]||targetCatalog.favoritesReady!==true)return
    var id=uuid(),all=Object.assign({},favoritePending);all[key]={requestId:id,environment:environment};favoritePending=all;favoriteError=""
    var favorite=!(targetCatalog.favorites || []).some(function(f){return f.provider===provider&&f.model===model})
    send({op:"model-favorite",environment:environment,provider:provider,model:model,favorite:favorite,requestId:id})
  }
  property bool scrollProbePending:false
  function probeScroll(){if(!worker.running||scrollProbePending)return;scrollProbePending=true;send({op:"scroll-config"})}
  FileView {path:Quickshell.env("HOME")+"/.local/state/omarchy/control-panel-prefs.json";watchChanges:true;printErrors:false;onFileChanged:{reload();scrollProbeDelay.restart()} }
  Timer{id:scrollProbeDelay;interval:150;onTriggered:if(root.panelVisible)root.probeScroll()}
  property var providers: []
  property bool stale: true
  property string error: ""
  property string quotaError: ""
  property var transcript: []
  property var activity: []
  property int visiblePanels: 0
  property bool sidebarCollapsed: false
  property var viewState: ({positions:{}})
  property bool viewStateLoaded: false
  function rememberView(values) { viewState = Object.assign({}, viewState, values); persistView.restart() }
  function rememberPosition(key, position) {
    var session=Object.assign({},sessionPositions);delete session[key];session[key]=position;var sessionKeys=Object.keys(session);while(sessionKeys.length>100)delete session[sessionKeys.shift()];sessionPositions=session
    var positions = Object.assign({}, viewState.positions || {})
    delete positions[key]; positions[key] = position
    var keys = Object.keys(positions); while (keys.length > 128) { delete positions[keys.shift()] }
    rememberView({positions:positions})
  }
  function persistViewNow() {
    if (!viewStateLoaded) return
    send({op:"ui-state-set",state:Object.assign({},viewState,{sidebarCollapsed:sidebarCollapsed,selectedEnvironment:selectedEnvironment,selectedThread:selectedThread,query:query})})
  }
  onSidebarCollapsedChanged: persistView.restart()
  onQueryChanged: persistView.restart()
  Timer { id: persistView; interval: 200; onTriggered: root.persistViewNow() }
  readonly property bool panelVisible: visiblePanels > 0
  readonly property int pollInterval: panelVisible || (!stale && state.working > 0) ? 10000 : 60000
  property bool loadingMessages: false
  property bool historyLimited: false
  property string messageError: ""
  property string selectedKey: ""
  property string selectedEnvironment: ""
  property string selectedThread: ""
  property string selectedTitle: ""
  property string selectedProject: ""
  property string query: ""
  property int requestedSerial: 0
  property var drafts: ({})
  // Native prompt-stash behavior, presented through a shared project drawer.
  property var projectDrafts:({entries:[],busy:false,error:"",requestId:"",scope:"",command:null})
  signal projectDraftRestored(string key)
  function draftProject(key){var t=(state.threads || []).find(function(t){return t.key===key || t.environmentId+"/"+t.id===key});return t?t.projectId || "":""}
  function projectDraftScope(){return selectedInfo.projectId?selectedEnvironment+"/"+selectedInfo.projectId:""}
  function requestProjectDrafts(action,entry){
    if(action==="list"&&projectDrafts.error&&projectDrafts.command){retryProjectDrafts();return}
    if(projectDrafts.busy || !selectedInfo.projectId || !selectedKey || draft.loading || draft.loadError)return
    if(action!=="list"&&(draft.busy||draft.attempt||draft.queueEdit||draft.attachments.some(function(f){return f.uploading})))return
    var keys=(state.threads || []).filter(function(t){return t.environmentId===root.selectedEnvironment&&t.projectId===root.selectedInfo.projectId}).map(function(t){return t.key || t.environmentId+"/"+t.id})
    if(keys.indexOf(selectedKey)<0)keys.push(selectedKey)
    flushDrafts()
    var command={op:"prompt-stash",action:action,environment:selectedEnvironment,projectId:selectedInfo.projectId,key:selectedKey,keys:keys,requestId:uuid(),draft:draft}
    if(entry){command.id=entry.id;command.fingerprint=entry.fingerprint || ""}
    if(action!=="list"){flushDraft(selectedKey);patchDraft(selectedKey,{busy:true});if(entry&&entry.sourceKey&&entry.sourceKey!==selectedKey&&drafts[entry.sourceKey])patchDraft(entry.sourceKey,{busy:true})}
    projectDrafts={entries:projectDrafts.scope===projectDraftScope()?projectDrafts.entries:[],busy:true,error:"",requestId:command.requestId,scope:projectDraftScope(),command:command}
    send(command)
  }
  function projectDraftTransferKey(key){var c=projectDrafts.command;return !!c&&c.action!=="list"&&(c.key===key||c.id==="thread:"+key)}
  function retryProjectDrafts(){
    var command=projectDrafts.command;if(!command||projectDrafts.busy)return
    projectDrafts=Object.assign({},projectDrafts,{busy:true,error:""});send(command)
  }
  property var dirtyDrafts: ({})
  property var pendingSends: ({})
  function flushDraft(key, submission) {
    var d=drafts[key]; if (!d || d.loading || d.loadError) return
    var revision=uuid(); var all=Object.assign({},drafts)
    all[key]=Object.assign({},d,{saveRevision:revision,saveState:"saving"}); drafts=all
    if (submission) { var pending=Object.assign({},pendingSends);pending[revision]=submission;pendingSends=pending }
    var dirty=Object.assign({},dirtyDrafts);delete dirty[key];dirtyDrafts=dirty
    send({op:"draft-save",key:key,revision:revision,draft:d})
  }
  function flushDrafts() { Object.keys(dirtyDrafts).forEach(function(key){root.flushDraft(key)});dirtyDrafts=({}) }
  Timer { id: saveDrafts; interval: 150; onTriggered: root.flushDrafts() }
  property var suggestionItems: []
  property bool suggestionsLoading: false
  property string suggestionsError: ""
  property string suggestionRequest: ""
  property var sessionPositions: ({})
  function dismissSuggestions() { suggestionRequest="";suggestionItems=[];suggestionsLoading=false;suggestionsError="" }
  function requestSuggestions(kind,query,instance) {
    suggestionRequest=uuid();suggestionItems=[];suggestionsError="";suggestionsLoading=true
    send({op:"suggest",environment:selectedEnvironment,thread:selectedThread,requestId:suggestionRequest,kind:kind,query:query,instanceId:instance || ""})
  }
  property var approvals:[]
  property var recentTranscript:[]
  property var olderTranscript:[]
  property string historyCursor:""
  property bool hasEarlier:false
  property bool historyExpanded:false
  property var historyOperation:({busy:false,error:""})
  function mergeTranscript(){
    var current={};recentTranscript.forEach(function(r){current[r.id]=true})
    transcript=olderTranscript.filter(function(r){return !current[r.id]}).concat(recentTranscript)
  }
  function resetHistory(){recentTranscript=[];olderTranscript=[];historyCursor="";hasEarlier=false;historyExpanded=false;historyOperation=({busy:false,error:""})}
  function loadEarlier(){
    if(historyOperation.busy || !hasEarlier || !historyCursor || !selectedThread)return
    var id=uuid();historyOperation={busy:true,error:"",requestId:id,key:selectedKey,serial:requestedSerial,cursor:historyCursor}
    send({op:"history",environment:selectedEnvironment,thread:selectedThread,requestId:id,cursor:historyCursor})
  }
  property var approvalOperations:({})
  readonly property var activeApproval:approvals.length?approvals[0]:null
  readonly property var approvalState:activeApproval?((draft.approvals || {})[activeApproval.requestId] || {}):({})
  function updateApproval(key,id,values){
    var d=drafts[key];if(!d || d.loading || d.loadError)return
    var all=Object.assign({},d.approvals || {});all[id]=Object.assign({},all[id] || {},values)
    var keep=Object.keys(all);while(keep.length>32)delete all[keep.shift()]
    patchDraft(key,{approvals:all},false)
  }
  function respondApproval(decision){
    if(!activeApproval || activeApproval.responseCapability!=="live" || draft.loading || draft.loadError || !catalog.canWrite)return
    var key=selectedKey+"/"+activeApproval.requestId,op=approvalOperations[key] || {}
    if(op.busy || op.answered || approvalState.responseStatus==="accepted")return
    if(!approvalState.attempt && !activeApproval.options.some(function(o){return o.decision===decision}))return
    var attempt=approvalState.attempt || {op:"approval",environment:selectedEnvironment,thread:selectedThread,runtimeRequestId:activeApproval.requestId,decision:decision,requestId:uuid()}
    updateApproval(selectedKey,activeApproval.requestId,{attempt:attempt})
    var all=Object.assign({},approvalOperations);all[key]={busy:true,error:"",requestId:attempt.requestId};approvalOperations=all
    flushDraft(selectedKey,attempt)
  }
  property var questions:[]
  property var questionOperations:({})
  readonly property var activeQuestion:questions.length?questions[0]:null
  readonly property var questionState:activeQuestion ? ((draft.questionnaires || {})[activeQuestion.requestId] || {}) : ({})
  function updateQuestion(values){
    if(!activeQuestion || draft.loading || draft.loadError)return
    var all=Object.assign({},draft.questionnaires || {}),id=activeQuestion.requestId
    all[id]=Object.assign({},all[id] || {},values)
    var keep=Object.keys(all);while(keep.length>32)delete all[keep.shift()]
    patchDraft(selectedKey,{questionnaires:all},false)
  }
  function respondQuestion(action,answers){
    if(!activeQuestion || draft.loading || draft.loadError || (action!=="dismiss"&&activeQuestion.responseCapability==="not_resumable") || (action==="dismiss"&&!activeQuestion.dismissible))return
    var key=selectedKey+"/"+activeQuestion.requestId,op=questionOperations[key] || {}
    if(op.busy || op.answered)return
    var attempt=questionState.attempt || {op:"question",action:action,environment:selectedEnvironment,thread:selectedThread,runtimeRequestId:activeQuestion.requestId,requestId:uuid(),answers:answers || {}}
    updateQuestion({attempt:attempt})
    var all=Object.assign({},questionOperations);all[key]={busy:true,error:"",requestId:attempt.requestId};questionOperations=all
    flushDraft(selectedKey,attempt)
  }
  signal questionSubmitRequested()
  property var tasks: null
  property var queue: ({items:[],held:false,canReorder:false,canSteer:false})
  property var optimisticQueued: ({})
  readonly property var queuedItems:(queue.items || []).concat(optimisticQueued[selectedKey] || [])
  function trackQueued(attempt) {
    if(attempt.op!=="send" || attempt.delivery!=="queue")return
    var all=Object.assign({},optimisticQueued);var list=(all[selectedKey] || []).filter(function(q){return q.messageId!=="omarchy-"+attempt.requestId})
    list.push({runId:"",messageId:"omarchy-"+attempt.requestId,text:attempt.text,attachments:attempt.attachments,pending:true})
    all[selectedKey]=list;optimisticQueued=all
  }
  function forgetQueued(key,id) {var all=Object.assign({},optimisticQueued);all[key]=(all[key] || []).filter(function(q){return q.messageId!==id});optimisticQueued=all}
  property var queueOperations: ({})
  readonly property var queueOperation: queueOperations[selectedKey] || ({busy:false,error:"",command:null})
  function queueAction(action,values) {
    if(!selectedThread || queueOperation.busy || draft.busy || draft.attempt)return
    if(queueOperation.command && action)return
    var command=queueOperation.command || Object.assign({op:"queue",action:action,environment:selectedEnvironment,thread:selectedThread,requestId:uuid()},values || {})
    var all=Object.assign({},queueOperations);all[selectedKey]={busy:true,error:"",command:command};queueOperations=all;send(command)
  }
  function beginQueueEdit(item) {
    if(item.available===false||draft.busy||draft.attempt||draft.loading||draft.loadError||draft.queueEdit||queueOperation.command||draft.attachments.some(function(f){return f.uploading}))return
    var backup={text:draft.text,attachments:draft.attachments,modelSelection:draft.modelSelection,delivery:draft.delivery}
    patchDraft(selectedKey,{text:item.text,attachments:(item.attachments || []).map(function(a){return {name:a.name,attachment:a,retained:true,uploading:false}}),queueEdit:{runId:item.runId,messageId:item.messageId,context:item.context,backup:backup},error:""})
    flushDraft(selectedKey)
  }
  function cancelQueueEdit() {
    if(!draft.queueEdit||draft.busy||draft.attempt)return
    var backup=draft.queueEdit.backup
    patchDraft(selectedKey,Object.assign({},backup,{queueEdit:null,error:"",attempt:null,busy:false}));flushDraft(selectedKey)
  }
  property var compactionStates: ({})
  function configureCompaction(patch){rememberView(patch);persistViewNow();refresh()}
  property var catalogs: ({})
  property var catalogLoading: ({})
  function ensureSidebarCatalogs(){if(panelVisible)(state.environments || []).filter(function(e){return e.state==="ready"}).forEach(function(e){root.ensureCatalog(e.id)})}
  function ensureCatalog(env){if(!env || catalogs[env] || catalogLoading[env])return;var pending=Object.assign({},catalogLoading);pending[env]=true;catalogLoading=pending;send({op:"catalog",environment:env})}
  function discardDraft(key){var d=drafts[key];if(!d || d.busy || d.loading || d.attempt || d.loadError)return;patchDraft(key,{text:"",attachments:[],error:""});flushDraft(key)}
  property var managementRequest: null
  property var folderBrowse:({busy:false,error:"",entries:[],parentPath:""})
  property var projectSources:({busy:false,providers:[]})
  property var pendingProjectOpen:null
  signal projectOpened(string environment,string projectId,string error)
  function openProject(environment,projectId){
    if(managementBusy||managementRequest)return
    pendingProjectOpen={environment:environment,projectId:projectId,requestId:""}
    managementError="";finishProjectOpen();refresh()
  }
  function finishProjectOpen(){
    var pending=pendingProjectOpen
    if(!pending||pending.requestId||managementBusy||managementRequest)return
    var project=(state.projects || []).find(function(p){return p.environmentId===pending.environment&&p.id===pending.projectId})
    if(!project)return
    var threads=(state.threads || []).filter(function(t){return t.environmentId===pending.environment&&t.projectId===pending.projectId&&!t.settled})
    if(threads.length){pendingProjectOpen=null;selectThread(pending.environment,threads[0].id);projectOpened(pending.environment,pending.projectId,"");return}
    var selection=project.defaultModelSelection || (selectedEnvironment===pending.environment?(draft.modelSelection || selectedInfo.modelSelection):null)
    if(!selection){var template=(state.threads || []).find(function(t){return t.environmentId===pending.environment&&t.modelSelection});selection=template?template.modelSelection:null}
    if(!selection){
      var available=catalogs[pending.environment]
      if(!available){ensureCatalog(pending.environment);return}
      var models=(available.models || []).filter(function(m){return m.available!==false})
      var favorite=models.find(function(m){return (available.favorites || []).some(function(f){return f.provider===m.instanceId&&f.model===m.model})}) || models[0]
      if(favorite)selection={instanceId:favorite.instanceId,model:favorite.model,options:[]}
    }
    if(!selection){managementError=T3I18n.tr("Select a model in an existing chat first.");pendingProjectOpen=null;projectOpened(pending.environment,pending.projectId,managementError);return}
    manage({action:"new-thread",environment:pending.environment,projectId:pending.projectId,title:T3I18n.tr("New chat"),modelSelection:selection,runtimeMode:"approval-required"})
    pendingProjectOpen=Object.assign({},pending,{requestId:managementRequest.requestId})
  }
  function loadProjectSources(environment){var id=uuid();projectSources={busy:true,environment:environment,requestId:id,providers:[]};send({op:"project-sources",environment:environment,requestId:id})}
  property var terminalOperation:({busy:false,error:"",requestId:"",key:""})
  readonly property bool canOpenTerminal:!!selectedThread && catalog.canWrite!==false && (state.environments || []).some(function(e){return e.id===root.selectedEnvironment&&e.state==="ready"})
  signal terminalOpened()
  function codeAction(message,link){
    var match=String(link).match(/^ot3-code-(run|copy):\/\/(\d+)$/)
    if(!match||!message||message.role!=="assistant"||!transcript.some(function(m){return m.id===message.id}))return false
    var block=(message.codeBlocks || [])[Number(match[2])];if(!block)return false
    if(match[1]==="copy"){copyText(block.code || "");return true}
    if(message.streaming||!block.command)return false
    openTerminal(block.command);return true
  }
  function openTerminal(command){
    if(!canOpenTerminal||terminalOperation.busy)return
    var id=uuid();terminalOperation={busy:true,error:"",requestId:id,key:selectedKey}
    send({op:"terminal-open",environment:selectedEnvironment,thread:selectedThread,requestId:id,command:typeof command==="string"?command:undefined})
  }
  property string windowLayoutError:""
  property var windowLayoutOperation:({busy:false})
  signal windowLayoutFinished(string error)
  function applyWindowLayout(mode,pid){var id=uuid();windowLayoutOperation={busy:true,requestId:id,mode:mode};send({op:"window-layout",mode:mode,pid:pid,requestId:id})}
  function browseFolders(environment,path){var id=uuid();folderBrowse={busy:true,error:"",entries:[],parentPath:"",requestId:id,environment:environment};send({op:"browse-folders",environment:environment,path:path,requestId:id})}
  property bool managementBusy: false
  property string managementError: ""
  function manage(values) {
    if (managementBusy) return
    var command = managementRequest || Object.assign({op:"manage",requestId:uuid()},values)
    managementRequest = command; managementBusy = true; managementError = ""; send(command)
  }
  readonly property var selectedInfo: state.threads.find(function(t) { return t.key === root.selectedKey }) || ({})
  readonly property bool providerSubagent: selectedInfo.providerSubagent===true
  function openParentThread(){if(providerSubagent&&selectedInfo.parentThreadId)selectThread(selectedEnvironment,selectedInfo.parentThreadId)}
  readonly property var draft: drafts[selectedKey] || ({text: "", attachments: [], modelSelection: selectedInfo.modelSelection || null, delivery: "auto", busy: false, error: "", attempt: null})
  readonly property var catalog: catalogs[selectedEnvironment] || ({models: [], canWrite: false})
  function uuid() { return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, function(c) { var r = Math.floor(Math.random()*16); return (c === "x" ? r : (r&3)|8).toString(16) }) }
  // Session-only edit history includes attachment descriptors. Never delete an
  // upload merely because it was undone: redo must keep the same asset identity.
  property var draftUndo:({})
  property var draftCursors:({})
  signal draftRestored(string key,int start,int end)
  function rememberDraftCursor(start,end){if(!selectedKey)return;var all=Object.assign({},draftCursors);all[selectedKey]={start:start,end:end};draftCursors=all}
  function editSnapshot(key,d){var cursor=draftCursors[key] || {start:(d.text || "").length,end:(d.text || "").length};return {text:d.text || "",attachments:(d.attachments || []).filter(function(f){return !f.uploading}),start:cursor.start,end:cursor.end}}
  function resetUndo(key){var all=Object.assign({},draftUndo);delete all[key];draftUndo=all}
  function saveUndo(key,before,after,kind){
    var all=Object.assign({},draftUndo),h=all[key] || {past:[],future:[]},now=Date.now()
    var a=before.text || "",b=after.text || "",p=0;while(p<Math.min(a.length,b.length)&&a[p]===b[p])p++
    var suffix=0;while(suffix<Math.min(a.length-p,b.length-p)&&a[a.length-1-suffix]===b[b.length-1-suffix])suffix++
    var removed=a.length-p-suffix,inserted=b.length-p-suffix
    var simple=kind==="typing" && (removed===0||inserted===0) && Math.max(removed,inserted)===1
    var merge=simple && h.simple && now-h.time<700 && (p===h.point || (removed===1&&p+1===h.point))
    h={past:merge?h.past:h.past.concat([editSnapshot(key,before)]),future:[],simple:simple,point:p+inserted,time:now}
    while(h.past.length>64)h.past.shift()
    delete all[key];all[key]=h
    // Bounded globally, including redo snapshots (rough UTF-16/JSON budget).
    var keys=Object.keys(all);while(keys.length>10)delete all[keys.shift()]
    function bytes(){return Object.keys(all).reduce(function(total,k){return total+all[k].past.concat(all[k].future).reduce(function(n,s){return n+2*(s.text.length+JSON.stringify(s.attachments).length)},0)},0)}
    var budget=bytes();while(budget>4*1024*1024){var oldest=Object.keys(all).find(function(k){return all[k].past.length||all[k].future.length});if(!oldest)break;var item=all[oldest];if(item.past.length)item.past.shift();else item.future.shift();budget=bytes()}
    draftUndo=all
  }
  function undoDraft(redo){
    if(draft.loading||draft.loadError||draft.busy||draft.attempt||draft.attachments.some(function(f){return f.uploading}))return false
    var h=draftUndo[selectedKey];if(!h)return false;var from=(redo?h.future:h.past).slice();if(!from.length)return false
    var restored=from.pop(),current=editSnapshot(selectedKey,draft),all=Object.assign({},draftUndo)
    all[selectedKey]=redo?{past:h.past.concat([current]),future:from}:{past:from,future:h.future.concat([current])};draftUndo=all
    patchDraft(selectedKey,{text:restored.text,attachments:restored.attachments,error:""},false)
    draftRestored(selectedKey,restored.start,restored.end);return true
  }
  function patchDraft(key, values, historyKind) {
    var all = Object.assign({}, drafts)
    all[key] = Object.assign({text: "", attachments: [], modelSelection: null, delivery: "auto", busy: false, error: "", attempt: null}, all[key] || {}, values)
    var previous=drafts[key]
    if(historyKind!==false && previous && !previous.loading && !previous.loadError && ((values.text!==undefined&&values.text!==previous.text)||(values.attachments!==undefined&&JSON.stringify(values.attachments)!==JSON.stringify(previous.attachments))))saveUndo(key,previous,all[key],historyKind)
    drafts = all
    if (!all[key].loading && !all[key].loadError && ["text","attachments","modelSelection","runtimeMode","delivery","attempt","queueEdit","questionnaires","approvals"].some(function(k){return values[k] !== undefined})) {
      var dirty=Object.assign({},dirtyDrafts);dirty[key]=true;dirtyDrafts=dirty;saveDrafts.restart()
      var updated=Object.assign({},drafts);updated[key]=Object.assign({},updated[key],{saveState:"saving",saveRevision:""});drafts=updated
    }
  }
  function edit(value,typing) { if (selectedKey && !draft.busy && !draft.attempt) patchDraft(selectedKey, {text: value, error: ""},typing?"typing":"edit") }
  function chooseModel(index) {
    var m = catalog.models[index]; if (!m) return
    patchDraft(selectedKey, {modelSelection: {instanceId: m.instanceId, model: m.model, options: []}})
  }
  function chooseRuntimeMode(mode){
    if(!selectedKey||draft.loading||draft.loadError||draft.busy||draft.attempt||draft.queueEdit||["approval-required","auto-accept-edits","auto","full-access"].indexOf(mode)<0)return
    var provider=(catalog.providers || []).find(function(p){return p.instanceId===(root.draft.modelSelection || root.selectedInfo.modelSelection || {}).instanceId})
    if(provider&&(provider.supportedRuntimeModes || []).length&&provider.supportedRuntimeModes.indexOf(mode)<0)return
    patchDraft(selectedKey,{runtimeMode:mode===selectedInfo.runtimeMode?null:mode})
  }
  function chooseOption(id, value) {
    var selection = Object.assign({}, draft.modelSelection || selectedInfo.modelSelection)
    var options = (selection.options || []).filter(function(o) { return o.id !== id })
    options.push({id: id, value: value}); selection.options = options
    patchDraft(selectedKey, {modelSelection: selection})
  }
  function attach(url, env, thread) {
    var key = env + "/" + thread; var d = drafts[key]
    if (!d || d.busy || d.attempt || d.attachments.length >= 100) return
    var id = uuid(); var files = d.attachments.slice()
    var name = String(url).split("/").pop()
    try { name = decodeURIComponent(name) } catch (_) {}
    files.push({uploadId: id, name: name, uploading: true})
    patchDraft(key, {attachments: files},false)
    send({op: "upload", environment: env, thread: thread, requestId: id, url: String(url)})
  }
  function removeAttachment(index) {
    var files = draft.attachments.slice(); var f = files[index]
    if (!f || f.uploading || draft.busy || draft.attempt) return
    files.splice(index, 1); patchDraft(selectedKey, {attachments: files,text:f.attachment?draft.text.split(attachmentReference(f.attachment)).join(""):draft.text})
  }
  property var clipboardRequests: ({})
  signal clipboardInserted(string key, int cursor)
  function attachmentReference(a) {
    return (a.type==="image"?"!":"")+"["+(String(a.name).replace(/[\[\]\\\n\r]/g," ").replace(/\s+/g," ").trim().slice(0,200)||a.type)+"](t3-context://v1/"+a.type+"/"+a.id+")"
  }
  function pasteClipboard(start,end) {
    if(!selectedThread || draft.loading || draft.busy || draft.attempt || draft.loadError)return
    var id=uuid(), requests=Object.assign({},clipboardRequests)
    requests[id]={key:selectedKey,start:start,end:end};clipboardRequests=requests
    patchDraft(selectedKey,{busy:true,error:""})
    send({op:"clipboard",environment:selectedEnvironment,thread:selectedThread,requestId:id,capacity:100-draft.attachments.length})
  }
  signal newThreadWorkspaceRequested(var request)
  signal workspaceChoiceAccepted()
  property var workspaceOperation:({busy:false,error:""})
  function beginNewThread(request){
    if(managementBusy||managementRequest||workspaceOperation.busy)return
    if(request.sourceKey&&request.sourceKey!==selectedKey)return
    var id=uuid();workspaceOperation={busy:true,error:"",request:request,requestId:id,action:"read"}
    send({op:"settings",action:"workspace-read",environment:request.command.environment,projectId:request.command.projectId,requestId:id})
  }
  function requestNewThread(values){beginNewThread({command:values})}
  function confirmNewThread(request,isolated,configured){
    if(managementBusy||managementRequest||workspaceOperation.busy)return
    if(request.sourceKey&&selectedKey!==request.sourceKey)return
    if(!configured){
      var id=uuid();workspaceOperation={busy:true,error:"",request:request,requestId:id,action:"write",isolated:isolated}
      send({op:"settings",action:"workspace-update",environment:request.command.environment,projectId:request.command.projectId,mode:isolated?"worktree":"local",requestId:id});return
    }
    var command=request.command || {},branch=command.branch || "HEAD"
    var strategy=isolated?{type:"worktree",baseRef:branch,startFromOrigin:!!request.startFromOrigin}:command.worktreePath?{type:"existing_worktree",worktreePath:command.worktreePath}:{type:"root"}
    if(!isolated&&command.branch)strategy.branch=command.branch
    workspaceChoiceAccepted()
    if(request.sourceKey)submit(true,strategy)
    else manage(Object.assign({},command,{workspaceStrategy:strategy}))
  }
  function submit(startNewThread,workspaceStrategy) {
    if(providerSubagent&&startNewThread!==true)return
    if(workspaceOperation.busy)return
    if((startNewThread===true||draft.attempt&&draft.attempt.op==="manage")&&(activeQuestion||managementBusy||(managementRequest&&(!draft.attempt||managementRequest.requestId!==draft.attempt.requestId))||draft.queueEdit||(!draft.attempt&&!selectedInfo.projectId)))return
    if(activeQuestion){questionSubmitRequested();return}
    if (!selectedThread || draft.loading || draft.loadError || draft.busy || draft.attachments.some(function(f) { return f.uploading || f.error })) return
    if (!draft.attempt && !draft.text.trim() && draft.attachments.length === 0) return
    var attempt = draft.attempt || {op: "send", environment: selectedEnvironment, thread: selectedThread,
      requestId: uuid(), text: draft.text, attachments: draft.attachments.map(function(f) { return f.attachment }),
      modelSelection: draft.modelSelection || selectedInfo.modelSelection, delivery: selectedInfo.activeRunId ? "queue" : draft.delivery}
    if(!draft.attempt&&draft.runtimeMode&&draft.runtimeMode!==selectedInfo.runtimeMode)attempt.runtimeMode=draft.runtimeMode
    if(startNewThread===true&&!draft.attempt&&!workspaceStrategy){
      beginNewThread({sourceKey:selectedKey,command:{environment:selectedEnvironment,projectId:selectedInfo.projectId,branch:selectedInfo.branch,worktreePath:selectedInfo.worktreePath}});return
    }
    if(startNewThread===true&&!draft.attempt)attempt=Object.assign({},attempt,{op:"manage",action:"new-thread",sendRequestId:uuid(),title:T3I18n.tr("New chat"),projectId:selectedInfo.projectId,runtimeMode:draft.runtimeMode || selectedInfo.runtimeMode,branch:selectedInfo.branch,worktreePath:selectedInfo.worktreePath,workspaceStrategy:workspaceStrategy,launchWithMessage:true,delivery:"auto"})
    if(!draft.attempt && draft.queueEdit){
      if(!queue.items.some(function(q){return q.runId===root.draft.queueEdit.runId})){patchDraft(selectedKey,{error:"queued_message_no_longer_pending"});return}
      attempt=Object.assign({},attempt,{op:"queue",action:"edit",runId:draft.queueEdit.runId,context:draft.queueEdit.context})
    }
    if (attempt.op === "send") messageSubmitted(selectedKey)
    trackQueued(attempt)
    patchDraft(selectedKey, {busy: true, attempt: attempt, error: ""}); flushDraft(selectedKey,attempt)
  }
  function stop() {
    if(providerSubagent)return
    if (!selectedInfo.activeRunId || draft.stopping) return
    patchDraft(selectedKey, {stopping: true})
    send({op: "stop", environment: selectedEnvironment, thread: selectedThread, requestId: uuid(), runId: selectedInfo.activeRunId})
  }
  function connectionLost() {
    if(backendStart.busy)backendStart={busy:false,error:"rpc_outcome_unknown",status:""}
    if(terminalOperation.busy)terminalOperation=Object.assign({},terminalOperation,{busy:false,error:"rpc_outcome_unknown"})
    if(workspaceOperation.busy){workspaceOperation=Object.assign({},workspaceOperation,{busy:false,error:"rpc_outcome_unknown"});newThreadWorkspaceRequested(workspaceOperation.request)}
    if(connectionSettings.busy)connectionSettings=Object.assign({},connectionSettings,{busy:false,error:"rpc_outcome_unknown"})
    clearInlineDocuments();historyOperation=Object.assign({},historyOperation,{busy:false,error:historyOperation.busy?"rpc_outcome_unknown":historyOperation.error});liveConnected=false;documentOpen=({busy:false,error:"",requestId:""});imageStates=({});favoritePending=({});scrollProbePending=false
    var operations=Object.assign({},queueOperations);Object.keys(operations).forEach(function(key){var o=operations[key];if(o.busy)operations[key]=Object.assign({},o,{busy:false,error:"rpc_outcome_unknown"})});queueOperations=operations
    var operations=Object.assign({},questionOperations);Object.keys(operations).forEach(function(k){if(operations[k].busy)operations[k]={busy:false,error:"rpc_outcome_unknown",requestId:operations[k].requestId}});questionOperations=operations
    var operations=Object.assign({},approvalOperations);Object.keys(operations).forEach(function(k){if(operations[k].busy)operations[k]=Object.assign({},operations[k],{busy:false,error:"rpc_outcome_unknown"})});approvalOperations=operations
    pendingSends=({}); clipboardRequests=({});if(projectDrafts.busy)projectDrafts=Object.assign({},projectDrafts,{busy:false,error:"rpc_outcome_unknown"})
    if (managementBusy) { managementBusy = false; managementError = "rpc_outcome_unknown" }
    Object.keys(drafts).forEach(function(key) {
      var d = root.drafts[key]
      root.patchDraft(key, {busy: root.projectDraftTransferKey(key), stopping: false,
        error: d.attempt ? "rpc_outcome_unknown" : d.error,
        attachments: d.attachments.map(function(f) { return f.uploading ? {uploadId: f.uploadId, name: f.name, error: "disconnected", uploading: false} : f })})
    })
    catalogs = ({});catalogLoading=({})
  }
  property bool liveConnected:false
  property var imageStates: ({})
  function imageKey(a){return selectedEnvironment+"/"+a.id}
  function requestImage(a){
    var key=imageKey(a);if(imageStates[key])return
    var all=Object.assign({},imageStates),id=uuid();all[key]={loading:true,url:"",error:"",requestId:id};imageStates=all
    send({op:"image",environment:selectedEnvironment,requestId:id,attachment:a})
  }
  property var documentOpen:({busy:false,error:"",requestId:"",data:null,frameUrl:"",frameRevision:0})
  property var inlineDocuments:({})
  function openInlineDocument(attachment){
    var id=uuid(),all=Object.assign({},inlineDocuments)
    all[id]={busy:true,error:"",requestId:id,data:null,frameUrl:"",frameRevision:0};inlineDocuments=all
    send({op:"document-open",inline:true,environment:selectedEnvironment,thread:selectedThread,attachment:attachment,requestId:id,theme:DocumentTheme.theme(Color.popups.background,Color.popups.text,Color.accent,T3Palette.values)})
    return id
  }
  function closeInlineDocument(id){if(!id||!inlineDocuments[id])return;send({op:"document-close",requestId:id});var all=Object.assign({},inlineDocuments);delete all[id];inlineDocuments=all}
  function updateInlineDocument(id,values){if(!inlineDocuments[id])return;var all=Object.assign({},inlineDocuments);all[id]=Object.assign({},all[id],values);inlineDocuments=all}
  function clearInlineDocuments(){Object.keys(inlineDocuments).forEach(closeInlineDocument)}
  function openReference(link,attachment){
    var path=String(link || "");
    if(/^https?:\/\//i.test(path)){Qt.openUrlExternally(path);return}
    if(path.startsWith("t3-thread://v1/")){selectThread(selectedEnvironment,path.slice(15));return}
    if(path.startsWith("#"))return
    if(/^[a-z][a-z0-9+.-]*:/i.test(path)&&!path.startsWith("file://"))return
    try{path=decodeURIComponent(path.replace(/^file:\/\//,""))}catch(e){return}
    path=path.replace(/:[0-9]+(?::[0-9]+)?$/,"")
    if(documentOpen.busy)return
    var id=uuid();documentOpen={busy:true,error:"",requestId:id,data:null,frameUrl:"",frameRevision:0}
    send({op:"document-open",environment:selectedEnvironment,thread:selectedThread,path:path,attachment:attachment || null,requestId:id,labels:{source:T3I18n.tr("Source"),preview:T3I18n.tr("Preview"),close:T3I18n.tr("Close")},theme:DocumentTheme.theme(Color.popups.background,Color.popups.text,Color.accent,T3Palette.values)})
  }
  property TextEdit documentClipboardBuffer:TextEdit{visible:false;textFormat:TextEdit.PlainText}
  function documentInput(event){if(documentOpen.requestId)send({op:"document-input",requestId:documentOpen.requestId,event:event})}
  function closeDocument(){if(documentOpen.requestId)send({op:"document-close",requestId:documentOpen.requestId});documentOpen={busy:false,error:"",requestId:"",data:null,frameUrl:"",frameRevision:0}}
  property var diffView: ({loading:false,text:"",error:"",requestId:""})
  function requestDiff(summary){var id=uuid();diffView={loading:true,text:"",error:"",requestId:id};send({op:"diff",environment:selectedEnvironment,thread:selectedThread,requestId:id,fromTurnCount:summary.fromTurnCount,toTurnCount:summary.toTurnCount})}
  function action(data) {
    if(data.op==="terminal-open"){
      if(data.requestId!==terminalOperation.requestId)return
      var here=terminalOperation.key===selectedKey
      terminalOperation=Object.assign({},terminalOperation,{busy:false,error:data.error || ""})
      if(!data.error&&here)terminalOpened()
      return
    }
    if(data.op==="settings"&&data.requestId===workspaceOperation.requestId){
      var op=workspaceOperation,request=op.request
      workspaceOperation=Object.assign({},op,{busy:false,requestId:"",error:data.error || ""})
      if(request.sourceKey&&request.sourceKey!==selectedKey)return
      if(data.error){newThreadWorkspaceRequested(request);return}
      request=Object.assign({},request,{startFromOrigin:!!data.result.startFromOrigin})
      if(op.action==="write")confirmNewThread(request,data.result.mode==="worktree",true)
      else if(data.result.mode)confirmNewThread(request,data.result.mode==="worktree",true)
      else newThreadWorkspaceRequested(request)
      return
    }
    if(data.op==="connections-manage"){
      if(data.requestId!==connectionSettings.requestId)return
      var command=connectionSettingsCommand
      connectionSettings=Object.assign({},connectionSettings,{busy:false,error:data.error || ""},data.result || {})
      if(!data.error&&command&&command.action!=="read"){
        if((command.environmentId===selectedEnvironment&&(command.action==="remove"||(command.action==="remove-route"&&!(data.result.environments || []).some(function(e){return e.id===root.selectedEnvironment}))||(command.action==="enabled"&&!command.enabled)))||(command.action==="local-enabled"&&!command.enabled&&selectedEnvironment==="local")){connectionChanging=true;clearSelection();connectionChanging=false}
        refresh()
      }
      connectionsManaged(command?command.action:"",data.error || "");return
    }
    if(data.op==="window-layout"){if(data.requestId===windowLayoutOperation.requestId){windowLayoutOperation=Object.assign({},windowLayoutOperation,{busy:false});windowLayoutError=data.error || "";windowLayoutFinished(windowLayoutError)}return}
    if(data.op==="browse-folders"){if(data.requestId===folderBrowse.requestId)folderBrowse=Object.assign({},folderBrowse,{busy:false,error:data.error || ""},data.result || {});return}
    if(data.op==="history"){
      var op=historyOperation
      if(data.requestId!==op.requestId || op.key!==selectedKey || op.serial!==requestedSerial)return
      historyOperation=Object.assign({},op,{busy:false,error:data.error || ""})
      if(data.error)return
      messagesAboutToChange()
      var page=data.result.timeline || [],seen={};page.forEach(function(r){seen[r.id]=true})
      olderTranscript=page.concat(olderTranscript.filter(function(r){return !seen[r.id]}))
      historyExpanded=true;historyCursor=data.result.historyCursor || "";hasEarlier=!!data.result.hasMoreHistory&&!!historyCursor
      mergeTranscript();messagesUpdated();return
    }
    if(data.op==="project-sources"){if(data.requestId===projectSources.requestId)projectSources=Object.assign({},projectSources,{busy:false,error:data.error || ""},data.result || {});return}
    if(data.op==="approval"){
      var key=Object.keys(approvalOperations).find(function(k){return root.approvalOperations[k].requestId===data.requestId})
      if(key){var all=Object.assign({},approvalOperations);all[key]={busy:false,answered:!data.error,error:data.error || "",requestId:data.requestId};approvalOperations=all
        updateApproval(data.environment+"/"+data.thread,key.substring((data.environment+"/"+data.thread+"/").length),{responseStatus:data.error?"failed":"accepted",responseError:data.error || ""})}
      return
    }
    if(data.op==="settings"){if(data.requestId===settingsResult.requestId)settingsResult=Object.assign({},settingsResult,{busy:false,error:data.error || ""},data.result || {});return}
    if(data.op==="copy"){copyFinished(data.requestId,data.error || "");return}
    if(data.op==="document-open"){if(inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{busy:false,error:data.error || "",data:data.result || null});return}if(data.requestId===documentOpen.requestId)documentOpen=Object.assign({},documentOpen,{busy:false,error:data.error || "",requestId:data.requestId,data:data.result || null});return}
    if(data.op==="backend-start"||data.op==="backend-control"){backendStart={busy:false,error:data.error || "",status:data.result?data.result.status:""};if(data.result&&data.result.local)acceptBackendStatus(data.result.local);if(!data.error&&data.result&&data.result.status!=="stopped")recoverLocalBackend();refreshBackendStatus();refresh();return}
    if(data.op==="pull-requests"){var name=Object.keys(prOperations).find(function(k){return root.prOperations[k].requestId===data.requestId});if(!name)return;var all=Object.assign({},prOperations);all[name]={busy:false,requestId:data.requestId,error:data.error || "",data:data.result};prOperations=all;return}
    if(data.op==="question"){
      var key=Object.keys(questionOperations).find(function(k){return root.questionOperations[k].requestId===data.requestId})
      if(key){
        var all=Object.assign({},questionOperations);all[key]={busy:false,answered:!data.error,error:data.error || "",requestId:data.requestId};questionOperations=all
        var threadKey=data.environment+"/"+data.thread,d=drafts[threadKey];
        if(d){var saved=Object.assign({},d.questionnaires || {}),id=Object.keys(saved).find(function(id){return saved[id].attempt&&saved[id].attempt.requestId===data.requestId});if(id){saved[id]=Object.assign({},saved[id],{responseStatus:data.error?(data.error==="rpc_outcome_unknown"?"uncertain":"failed"):"accepted",responseError:data.error || ""});patchDraft(threadKey,{questionnaires:saved},false)}}
      }
      return
    }
    if(data.op==="scroll-config"){scrollProbePending=false;if(data.result)scrollPolicy=data.result;return}
    if(data.op==="model-favorite"){
      var all=Object.assign({},favoritePending),key=Object.keys(all).find(function(k){return all[k].requestId===data.requestId});if(!key)return;var env=all[key].environment;delete all[key];favoritePending=all;if(env===selectedEnvironment)favoriteError=data.error || ""
      if(data.result){var cache=Object.assign({},catalogs);cache[env]=Object.assign({},cache[env] || {},data.result);catalogs=cache}return
    }
    if(data.op==="image"){var key=Object.keys(imageStates).find(function(k){return root.imageStates[k].requestId===data.requestId});if(!key)return;var all=Object.assign({},imageStates);all[key]={loading:false,url:data.result?data.result.url || "":"",error:data.error || "",requestId:data.requestId};imageStates=all;return}

    if(data.op==="diff"){if(data.requestId===diffView.requestId)diffView={loading:false,text:data.result?data.result.diff || "":"",error:data.error || "",requestId:data.requestId};return}
    if(data.op==="clipboard"){
      var request=clipboardRequests[data.requestId];if(!request)return
      var requests=Object.assign({},clipboardRequests);delete requests[data.requestId];clipboardRequests=requests
      var saved=drafts[request.key];if(!saved)return
      var result=data.result || {},added=result.attachments || [],insert=result.text || ""
      if(added.length)insert=(request.start>0&&!/\s/.test(saved.text[request.start-1])?" ":"")+added.map(attachmentReference).join(" ")+" "
      var next=saved.text
      if(!data.error)next=next.slice(0,request.start)+insert+next.slice(request.end)
      patchDraft(request.key,{busy:false,text:next,attachments:saved.attachments.concat(added.map(function(a){return {uploadId:a.id,name:a.name,uploading:false,attachment:a}})),error:data.error || result.error || ""})
      if(!data.error)clipboardInserted(request.key,request.start+insert.length)
      return
    }
    if (data.op === "suggest") {
      if (data.requestId !== suggestionRequest || data.environment+"/"+data.thread !== selectedKey) return
      suggestionsLoading=false;suggestionsError=data.error || "";suggestionItems=data.result ? data.result.items || [] : [];return
    }
    if (data.op === "manage") {
      if (!managementRequest || data.requestId !== managementRequest.requestId) return
      // Older desktop clients returned no ID for Project.id responses.
      if(!data.error&&data.result&&data.result.action==="add-project"&&!data.result.projectId)data.result=Object.assign({},data.result,{projectId:"omarchy-project-"+data.requestId})
      var creation=managementRequest
      var sourceKey=creation.environment+"/"+creation.thread
      var source=drafts[sourceKey]
      var moving=source&&source.attempt&&source.attempt.op==="manage"&&source.attempt.requestId===creation.requestId
      managementBusy = false; managementError = data.error || ""
      var localRejection=["invalid_workspace","invalid_project","invalid_request_id","invalid_title","invalid_message","invalid_attachments","image_limit_10MiB","image_total_limit_80MiB"].indexOf(data.error)>=0
      var retryable=data.error==="rpc_outcome_unknown" || (!!creation.workspaceStrategy&&!!data.error&&!localRejection)
      if(moving){
        if(data.error)patchDraft(sourceKey,{busy:false,error:data.error,attempt:retryable?source.attempt:null})
        else if(creation.workspaceStrategy){
          // Native launch durably accepts the initial message and defers its
          // execution until workspace preparation completes.
          patchDraft(sourceKey,{text:"",attachments:[],attempt:null,busy:false,error:""});resetUndo(sourceKey);flushDraft(sourceKey)
        } else {
          var destination=data.environment+"/"+data.result.threadId
          var attempt=Object.assign({},source.attempt,{op:"send",requestId:source.attempt.sendRequestId,thread:data.result.threadId,delivery:"auto",moveSource:sourceKey,creationRequestId:source.attempt.requestId})
          delete attempt.nextThread
          patchDraft(destination,{text:source.text,attachments:source.attachments,modelSelection:attempt.modelSelection,delivery:"auto",attempt:attempt,busy:true,loading:false,error:""})
          // Save the destination before clearing the source or sending anything.
          flushDraft(destination,attempt)
        }
      }
      if (!retryable) managementRequest = null
      if (!data.error) {
        if(data.result.action==="project-remove" && selectedInfo.projectKey===data.environment+"/"+data.result.projectId){var next=state.threads.find(function(t){return t.projectKey!==root.selectedInfo.projectKey});if(next)selectThread(next.environmentId,next.id);else clearSelection()}
        if(["archive","delete-thread"].indexOf(data.result.action)>=0 && selectedKey===data.environment+"/"+data.result.threadId){var nextThread=state.threads.find(function(t){return t.key!==root.selectedKey});if(nextThread)selectThread(nextThread.environmentId,nextThread.id);else clearSelection()}
        refresh(); if (data.result.action === "new-thread"&&(!moving||selectedKey===sourceKey)) selectThread(data.environment,data.result.threadId)
      }
      managementFinished(data)
      if(pendingProjectOpen&&pendingProjectOpen.requestId===data.requestId){var opened=pendingProjectOpen;if(data.error!=="rpc_outcome_unknown")pendingProjectOpen=null;projectOpened(opened.environment,opened.projectId,data.error || "")}
      if(!data.error&&["add-project","create-project","clone-project"].indexOf(data.result.action)>=0){var environment=data.environment,projectId=data.result.projectId;Qt.callLater(function(){root.openProject(environment,projectId)})}
      return
    }
    var key = data.environment + "/" + data.thread
    if(data.op==="queue") {
      var edit=drafts[key]
      if(edit && edit.attempt && edit.attempt.op==="queue" && edit.attempt.requestId===data.requestId){
        if(data.error)patchDraft(key,{busy:false,error:data.error,attempt:data.error==="rpc_outcome_unknown"?edit.attempt:null})
        else {patchDraft(key,Object.assign({},edit.queueEdit.backup,{queueEdit:null,attempt:null,busy:false,error:""}));flushDraft(key);refresh()}
        return
      }
      var operation=queueOperations[key];if(!operation || !operation.command || operation.command.requestId!==data.requestId)return
      var operations=Object.assign({},queueOperations);operations[key]={busy:false,error:data.error || "",command:data.error==="rpc_outcome_unknown"?operation.command:null};queueOperations=operations
      if(!data.error)refresh();return
    }

    if (data.op === "catalog") {
      var loading=Object.assign({},catalogLoading);delete loading[data.environment];catalogLoading=loading
      var all = Object.assign({}, catalogs); all[data.environment] = data.result || Object.assign({},all[data.environment] || {models:[],providers:[],favorites:[]},{favoritesReady:false,error:data.error}); catalogs = all;finishProjectOpen(); return
    }
    var d = drafts[key]; if (!d) return
    if (data.op === "upload") {
      if(!d.attachments.some(function(f){return f.uploadId===data.requestId}))return
      var files = d.attachments.map(function(f) { return f.uploadId === data.requestId ? {uploadId: f.uploadId, name: f.name, uploading: false, error: data.error || "", attachment: data.result} : f })
      var text=d.text
      if(!data.error&&data.result)text+=(text&&!/\s$/.test(text)?" ":"")+attachmentReference(data.result)+" "
      patchDraft(key, {attachments: files,text:text})
      if(!data.error)clipboardInserted(key,text.length)
    } else if (data.op === "send" && d.attempt && d.attempt.requestId === data.requestId) {
      if(data.error && data.error!=="rpc_outcome_unknown")forgetQueued(key,"omarchy-"+data.requestId)
      if (data.error) patchDraft(key, {busy: false, error: data.error, attempt: data.error === "rpc_outcome_unknown" ? d.attempt : null})
      else {
        var acceptedMode=d.attempt.runtimeMode
        if(acceptedMode){snapshotAboutToChange();state=Object.assign({},state,{threads:state.threads.map(function(t){return t.key===key?Object.assign({},t,{runtimeMode:acceptedMode}):t})});snapshotUpdated()}
        patchDraft(key, {text: "", attachments: [], busy: false, attempt: null, runtimeMode:null,error: ""},false); resetUndo(key);flushDraft(key); refresh()
      }
    } else if (data.op === "stop") patchDraft(key, {stopping: false, error: data.error || ""})
  }
  signal connectionsManaged(string action,string error)
  signal managementFinished(var result)
  signal snapshotUpdated()
  signal snapshotAboutToChange()
  signal messageSubmitted(string key)
  signal messagesUpdated()
  signal messagesAboutToChange()
  signal threadWillChange()
  function send(data) { if (worker.running) worker.write(JSON.stringify(data) + "\n") }
  function setPanelVisible(value) { visiblePanels = Math.max(0, visiblePanels + (value ? 1 : -1)) }
  onPanelVisibleChanged: {
    send({op: "visible", value: panelVisible});if(panelVisible){probeScroll();ensureSidebarCatalogs()}
    if (!panelVisible) { clearInlineDocuments();liveConnected=false;documentOpen=({busy:false,error:"",requestId:""});imageStates=({});requestedSerial++; activity = []; loadingMessages = false; persistViewNow(); flushDrafts() }
    else if (selectedThread) fetchMessages()
  }
  function activityLabel(){return activity.length?activity.map(function(a){return root.activityText(a)}).join(" · "):T3I18n.tr("Thinking")}
  function activityText(a){
    var category=a.category || ({reasoning:"think",command_execution:"execute",file_change:"edit",file_search:"search",web_search:"web",subagent:"agent"})[a.type] || "tool";var live=a.active!==false
    var labels={compact:live?"Compacting context":"Context compacted",think:"Thinking",execute:live?"Running commands":"Ran commands",read:live?"Reading files":"Read files",edit:live?"Editing files":"Edited files",search:live?"Searching files":"File search",web:live?"Searching the web":"Web search",agent:"Delegated work",tool:live?"Calling tool":"Tool finished"}
    if(a.command && (live || !(a.count>1)))return T3I18n.tr(live?"Running {command}":"Ran {command}",{command:a.command})
    var label=T3I18n.tr(labels[category]);if(a.title)label=T3I18n.tr("{activity} · {title}",{activity:label,title:a.title});else if(live)label+="…"
    return a.count>1?T3I18n.tr("{activity} ({count})",{activity:label,count:a.count}):label
  }
  function refresh() { send({op: "refresh"}) }
  function search(value) { query = value }
  function clearSelection(){threadWillChange();resetHistory();requestedSerial++;send({op:"clear-select"});selectedEnvironment="";selectedThread="";selectedKey="";selectedTitle="";selectedProject="";transcript=[];activity=[];questions=[];approvals=[];tasks=null;queue=({items:[]});loadingMessages=false;messageError="";liveConnected=false;persistView.restart()}
  function selectThread(env, thread) {
    if (selectedEnvironment === env && selectedThread === thread) { fetchMessages(); return }
    clearInlineDocuments();closeDocument();favoriteError="";liveConnected=false;documentOpen=({busy:false,error:"",requestId:""});imageStates=({});diffView=({loading:false,text:"",error:"",requestId:""});dismissSuggestions();threadWillChange()
    selectedEnvironment = env; selectedThread = thread; selectedKey = env + "/" + thread
    var found = state.threads.find(function(t) { return t.key === root.selectedKey })
    selectedTitle = found ? (found.title || thread) : thread
    selectedProject = found ? (found.project || env) : env
    if (!drafts[selectedKey]) {
      patchDraft(selectedKey, {loading:true,modelSelection: found ? found.modelSelection : null})
      send({op:"draft-load",key:selectedKey,projectId:draftProject(selectedKey)})
    }
    if (!catalogs[env]) send({op: "catalog", environment: env})
    resetHistory();transcript = []; activity = [];tasks=null;questions=[];approvals=[];queue=({items:[],held:false,canReorder:false,canSteer:false}); messageError = ""; fetchMessages()
    persistView.restart()
  }
  function fetchMessages() {
    if (!panelVisible || !selectedThread) return
    if(historyOperation.busy)historyOperation=({busy:false,error:""})
    requestedSerial++; loadingMessages = true
    send({op: "select", environment: selectedEnvironment, thread: selectedThread, serial: requestedSerial})
  }
  function openThread(env, thread) { send({op: "open", environment: env, thread: thread}) }
  function receive(line) {
    try {
      var event = JSON.parse(line); var data = event.data
      if(event.type==="prompt-stash"){
        if(data.requestId!==projectDrafts.requestId)return
        var command=projectDrafts.command,result=data.result || {},changes=result.changes || {}
        Object.keys(changes).forEach(function(key){
          root.patchDraft(key,Object.assign({},changes[key],{busy:false,error:"",saveState:"saved",saveError:""}),false);root.resetUndo(key)
          var dirty=Object.assign({},root.dirtyDrafts);delete dirty[key];root.dirtyDrafts=dirty
        })
        var retryable=!!data.error&&["draft_save_failed","draft_read_failed","local_error","rpc_outcome_unknown","drafts_busy"].indexOf(data.error)>=0
        if(command&&command.action!=="list"){
          if(drafts[command.key])patchDraft(command.key,{busy:retryable,error:data.error || ""})
          var source=command.id&&command.id.indexOf("thread:")===0?command.id.slice(7):""
          if(source&&drafts[source])patchDraft(source,{busy:retryable})
        }
        projectDrafts=Object.assign({},projectDrafts,{busy:false,error:data.error || "",entries:data.error?projectDrafts.entries:result.entries || [],command:retryable?command:null})
        if(!data.error&&command&&command.action==="restore")projectDraftRestored(command.key)
        if(!data.error&&projectDrafts.scope!==projectDraftScope()&&selectedInfo.projectId)Qt.callLater(function(){root.requestProjectDrafts("list")})
      }
      else if (event.type === "draft-loaded") {
        var loaded=drafts[data.key];if(!loaded || !loaded.loading) return
        var saved=data.draft || {};var restored=Object.assign({},loaded,saved,{loading:false,loadError:data.error || "",saveState:data.error?"error":"saved",busy:false,error:saved.attempt?"rpc_outcome_unknown":data.error || ""})
        var cache=Object.assign({},drafts);cache[data.key]=restored;drafts=cache
      }
      else if (event.type === "draft-saved") {
        var existing=drafts[data.key];var submission=pendingSends[data.revision]
        if(existing && existing.saveRevision===data.revision) { var updated=Object.assign({},drafts);updated[data.key]=Object.assign({},existing,{saveState:data.error?"error":"saved",saveError:data.error || ""});drafts=updated }
        if(submission) {
          var pending=Object.assign({},pendingSends);delete pending[data.revision];pendingSends=pending
          if(submission.op==="approval"){
            var approvalKey=data.key+"/"+submission.runtimeRequestId,operations=Object.assign({},approvalOperations)
            if(data.error){operations[approvalKey]={busy:false,error:"draft_save_failed",requestId:submission.requestId};approvalOperations=operations}
            else send(submission)
          }
          else if(submission.op==="question"){
            var questionKey=data.key+"/"+submission.runtimeRequestId,operations=Object.assign({},questionOperations)
            if(data.error){operations[questionKey]={busy:false,error:"draft_save_failed",requestId:submission.requestId};questionOperations=operations}
            else send(submission)
          }
          else if(data.error) {
            patchDraft(data.key,{busy:false,error:"draft_save_failed"})
            if(submission.moveSource)patchDraft(submission.moveSource,{busy:false,error:"draft_save_failed"})
          }
          else if (existing && existing.busy && existing.attempt && existing.attempt.requestId === submission.requestId) {
            if(submission.op==="manage")manage(submission)
            else {
              if(submission.moveSource){
                var source=drafts[submission.moveSource]
                if(source&&source.attempt&&source.attempt.requestId===submission.creationRequestId){patchDraft(submission.moveSource,{text:"",attachments:[],attempt:null,busy:false,error:""});resetUndo(submission.moveSource);flushDraft(submission.moveSource)}
              }
              var command=Object.assign({},submission);delete command.moveSource;delete command.creationRequestId;send(command)
            }
          }
        }
      }
      else if (event.type === "ui-state" && !viewStateLoaded) {
        viewStateLoaded = true; viewState = data; sidebarCollapsed = !!data.sidebarCollapsed; query = data.query || ""
        if (data.selectedEnvironment && data.selectedThread) selectThread(data.selectedEnvironment,data.selectedThread)
      }
      else if (event.type === "ui-state-error") { console.warn("T3 presentation state could not be saved: " + data.error) }
      else if(event.type==="live-state" && data.serial===requestedSerial && panelVisible){liveConnected=!!data.connected;if(data.error&&!transcript.length){loadingMessages=false;messageError=data.error}}
      else if(event.type === "compaction"){var states=Object.assign({},compactionStates);states[data.key]=data;compactionStates=states}
      else if(event.type==="document-clipboard"&&(data.requestId===documentOpen.requestId||inlineDocuments[data.requestId])){documentClipboardBuffer.text=data.text || "";documentClipboardBuffer.selectAll();documentClipboardBuffer.copy();documentClipboardBuffer.deselect()}
      else if(event.type==="document-size" && inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{contentHeight:data.height})}
      else if(event.type==="document-frame" && inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{frameUrl:data.url || inlineDocuments[data.requestId].frameUrl,frameRevision:data.revision,frameWidth:data.width,frameHeight:data.height,error:data.error || ""})}
      else if(event.type==="document-frame" && data.requestId===documentOpen.requestId){documentOpen=Object.assign({},documentOpen,{frameUrl:data.url || documentOpen.frameUrl,frameRevision:data.revision || documentOpen.frameRevision,frameWidth:data.width || documentOpen.frameWidth,frameHeight:data.height || documentOpen.frameHeight,error:data.error || ""})}
      else if (event.type === "action") action(data)
      else if (event.type === "backend-status") {acceptBackendStatus(data)}
      else if (event.type === "snapshot") { snapshotAboutToChange(); state = data; stale = data.stale; error = "";ensureSidebarCatalogs()
        var current = state.threads.find(function(t) { return t.key === root.selectedKey })
        if (current) { selectedTitle = current.title; selectedProject = current.project }
        finishLocalRecovery();snapshotUpdated() }
      else if (event.type === "quotas") { providers = data.providers || []; quotaError = "" }
      else if (event.type === "messages" && data.serial === requestedSerial && panelVisible) {
        loadingMessages = false; messageError = data.result.error || ""
        if(data.result.threadState){var currentKey=selectedKey,current=state.threads.find(function(t){return t.key===currentKey}),delta=data.result.threadState;if(current && Object.keys(delta).some(function(k){return current[k]!==delta[k]})){snapshotAboutToChange();state=Object.assign({},state,{threads:state.threads.map(function(t){return t.key===currentKey?Object.assign({},t,delta):t})});snapshotUpdated()}}
        messagesAboutToChange()
        recentTranscript = data.result.timeline || data.result.messages || []
        if(!historyExpanded&&!historyOperation.busy){historyCursor=data.result.historyCursor || "";hasEarlier=!!data.result.hasMoreHistory&&!!historyCursor}
        mergeTranscript()
        activity = data.result.activity || []
        questions=data.result.questions || []
        approvals=data.result.approvals || []
        tasks=data.result.tasks || null
        queue=data.result.queue || {items:[],held:false,canReorder:false,canSteer:false}
        var acknowledged=queue.acknowledgedMessageIds || [];var queued=Object.assign({},optimisticQueued);queued[selectedKey]=(queued[selectedKey] || []).filter(function(q){return acknowledged.indexOf(q.messageId)<0});optimisticQueued=queued
        historyLimited = !!data.result.hasMoreHistory || !!data.result.payloadBudgetExceeded
        messagesUpdated()
      } else if (event.type === "error") { error = data.error; stale = true }
    } catch (_) { error = "malformed"; stale = true }
  }
  Process {
    id: worker
    command: [root.binary, "watch"]
    stdinEnabled: true
    running: true
    onStarted: {
      root.send({op: "visible", value: root.panelVisible}); root.send({op: "search", query: root.query})
      if (root.selectedEnvironment) root.send({op: "catalog", environment: root.selectedEnvironment})
      if(root.panelVisible)root.probeScroll()
      if (root.panelVisible && root.selectedThread) root.fetchMessages()
      if(root.projectDrafts.command&&root.projectDrafts.command.action!=="list")root.retryProjectDrafts()
      Object.keys(root.drafts).forEach(function(key){ if(root.projectDraftTransferKey(key))return;if(root.drafts[key].loading)root.send({op:"draft-load",key:key,projectId:root.draftProject(key)});else root.flushDraft(key) })
    }
    stdout: SplitParser { onRead: function(line) { root.receive(line) } }
    onExited: { root.connectionLost(); root.stale = true; root.error = "disconnected"; reconnect.restart() }
  }
  Timer { id: reconnect; interval: 5000; onTriggered: worker.running = true }
}
