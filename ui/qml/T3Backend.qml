pragma Singleton
import "../js/DeviceSettings.js" as Devices
import "../js/BrowserProfiles.js" as Profiles
import "../js/BrowserImport.js" as ImportFlow
import "../js/PreviewSessions.js" as Previews
import "../js/BrowserSurface.js" as BrowserSurface
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import "../js/DocumentTheme.js" as DocumentTheme
import "../js/SidebarModel.js" as Sidebar
import "../js/NativeKeybindings.js" as NativeKeys
import "../js/UsageLimits.js" as Usage

Singleton {
  id: root
  readonly property string binary: Qt.resolvedUrl("../../bin/omarchy-t3-client").toString().replace(/^file:\/\//, "")
  property var state: ({threads: [], environments: [], working: 0, pending: 0})
  property var inboxReturnState:({working:null,returned:{}})
  function observeInboxReturns(){inboxReturnState=(viewState || {}).sidebarCompact?Sidebar.observeReturns(inboxReturnState,state.threads || [],Date.now()):{working:null,returned:{}}}
  onStateChanged:{observeInboxReturns();finishProjectOpen();if(panelVisible)ensureRuntimeKeybindings(false)}
  onViewStateChanged:observeInboxReturns()
  property var backendStart:({busy:false,error:"",status:""})
  property var localBackend:({owner:"unknown",running:false,canControl:false,installed:false})
  function refreshBackendStatus(){send({op:"backend-status"})}
  function controlBackend(action){if(backendStart.busy)return;if(!worker.running){backendStart={busy:false,error:"disconnected",status:""};return;}backendStart={busy:true,error:"",status:""};send({op:"backend-control",action:action,requestId:uuid()})}
  signal copyFinished(string requestId,string error)
  function copyText(value){var id=uuid();send({op:"copy",kind:"text",text:value,requestId:id});return id}
  function copyImage(attachment){var id=uuid();send({op:"copy",kind:"image",environment:selectedEnvironment,thread:selectedThread,attachment:attachment,requestId:id});return id}
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
  property var clientSettings:({busy:false,error:"",requestId:"",data:{planModeEnabled:false}})
  function requestClientSettings(patch){
    if(clientSettings.busy)return
    var id=uuid();clientSettings=Object.assign({},clientSettings,{busy:true,error:"",requestId:id})
    send({op:"client-settings",action:patch?"update":"read",requestId:id,revision:(clientSettings.data || {}).revision || "",patch:patch || {}})
  }
  property var diffPreferenceUpdate:({desired:"",stage:"",error:""})
  readonly property var diffPreferences:({layout:diffPreferenceUpdate.desired || (clientSettings.data || {}).diffLayout || "stacked",ignoreWhitespace:(clientSettings.data || {}).diffIgnoreWhitespace!==false,wordWrap:(clientSettings.data || {}).wordWrap!==false,filesCollapsed:(clientSettings.data || {}).diffFilesCollapsed!==false})
  function setDiffLayout(layout){
    if(["stacked","split"].indexOf(layout)<0||clientSettings.busy)return false
    diffPreferenceUpdate={desired:layout,stage:worker.running?"read":"",error:worker.running?"":"disconnected"}
    if(worker.running)requestClientSettings()
    return worker.running
  }
  readonly property bool planModeEnabled:!!(clientSettings.data || {}).planModeEnabled
  readonly property var interactionProvider:(catalog.providers || []).find(function(p){return p.instanceId===(root.draft.modelSelection || root.selectedInfo.modelSelection || {}).instanceId}) || null
  readonly property bool interactionModeEnabled:planModeEnabled&&!!interactionProvider&&interactionProvider.showInteractionModeToggle!==false
  readonly property string interactionMode:interactionModeEnabled?(draft.interactionMode || selectedInfo.interactionMode || "default"):"default"
  property var planView:({plans:[],ready:null})
  property var planOperation:({busy:false,error:"",requestId:"",command:null})
  readonly property var readyPlan:!rewindPending&&interactionModeEnabled&&interactionMode==="plan"&&!selectedInfo.working&&!questions.length&&!draft.attachments.length?planView.ready:null
  readonly property var readyPlanView:Object.assign({},planView,{ready:readyPlan,error:planOperation.error || planView.error || "",loading:planOperation.busy,canCancel:planOperation.error==="draft_save_failed"&&!!planOperation.command&&planOperation.command.op==="manage"&&!managementRequest})
  function chooseInteractionMode(mode){
    if(!interactionModeEnabled||catalog.canWrite===false||providerSubagent||!selectedKey||draft.loading||draft.loadError||draft.busy||draft.attempt||draft.queueEdit||planOperation.busy||["default","plan"].indexOf(mode)<0)return
    patchDraft(selectedKey,{interactionMode:mode===selectedInfo.interactionMode?null:mode,sourcePlanRef:null})
  }
  function requestPlanFollowUp(plan,feedback,newThread){
    if(rewindPending||!readyPlan||!plan||plan.id!==readyPlan.id||providerSubagent||catalog.canWrite===false||draft.loading||draft.loadError||draft.busy||draft.attempt||draft.queueEdit||draft.attachments.length||planOperation.busy||managementBusy||managementRequest)return
    var id=uuid(),selection=JSON.parse(JSON.stringify(draft.modelSelection || selectedInfo.modelSelection || {}))
    planOperation={busy:true,error:"",requestId:id,key:selectedKey,draftText:draft.text,newThread:!!newThread,modelSelection:selection,command:null}
    send({op:"plan-followup",requestId:id,environment:selectedEnvironment,thread:selectedThread,planId:plan.id,modelSelection:selection,interactionMode:interactionMode,draftText:newThread?"":draft.text,hasAttachments:false})
  }
  function cancelPlanOperation(){
    if(!readyPlanView.canCancel||planOperation.busy)return
    var c=planOperation.command,key=c.environment+"/"+c.thread,d=drafts[key]
    if(d&&d.attempt&&d.attempt.requestId===c.requestId){patchDraft(key,{attempt:null,busy:false,error:""});flushDraft(key)}
    planOperation={busy:false,error:"",command:null}
  }
  function retryPlanOperation(){
    var op=planOperation,c=op.command;if(op.busy)return;if(!c){if(readyPlan)requestPlanFollowUp(readyPlan,draft.text,op.newThread);else fetchMessages();return}
    var key=c.environment+"/"+c.thread,d=drafts[key];if(!d||d.loading||d.loadError||d.busy)return
    planOperation=Object.assign({},op,{busy:true,error:""});patchDraft(key,{attempt:c,busy:true,error:""});flushDraft(key,c)
  }
  property var settingsResult:({busy:false,error:"",fields:[],environment:""})
  property var providerInstances:({busy:false,error:"",environment:"",instances:[],writable:false,requestId:""})
  function requestProviderInstances(environment,action,values){
    if(providerInstances.busy||!environment||environment==="*")return
    if(!worker.running){providerInstances={busy:false,error:"disconnected",environment:environment,instances:[],drivers:[],writable:false,requestId:""};return}
    var id=uuid();providerInstances={busy:true,error:"",environment:environment,instances:providerInstances.environment===environment?providerInstances.instances:[],writable:providerInstances.environment===environment&&providerInstances.writable,requestId:id,action:action}
    send(Object.assign({op:"provider-instances",environment:environment,action:action,requestId:id},values || {}))
  }
  property var connectionSettings:({busy:false,error:"",environments:[],revision:""})
  property var connectionSettingsCommand:null
  property bool connectionChanging:false
  function manageConnections(action,values){
    if(connectionSettings.busy)return
    var command=Object.assign({op:"connections-manage",action:action,revision:connectionSettings.revision,requestId:uuid()},values || {})
    connectionSettingsCommand={action:action,environmentId:command.environmentId,enabled:command.enabled};connectionSettings=Object.assign({},connectionSettings,{busy:true,error:"",requestId:command.requestId});send(command)
  }
  property var connectionCloud:({busy:false,error:"",environment:"local",status:null,device:null})
  signal connectionCloudFinished(string action,string error)
  function requestConnectionCloud(environment,action,values){
    if(environment!=="local"||connectionCloud.busy)return
    if(!worker.running){connectionCloud=Object.assign({},connectionCloud,{error:"disconnected"});return}
    var id=uuid();connectionCloud=Object.assign({},connectionCloud,{busy:true,error:"",warning:"",statusError:"",device:null,step:"",stage:"starting",environment:environment,requestId:id,action:action})
    send(Object.assign({op:"connection-cloud",environment:environment,action:action,requestId:id},values || {}))
  }
  function cancelConnectionCloud(id){if(connectionCloud.busy&&connectionCloud.requestId===id)send({op:"connection-cloud-cancel",requestId:id})}
  function confirmConnectionCloudInstall(id,approved){if(connectionCloud.busy&&connectionCloud.requestId===id)send({op:"connection-cloud-install",requestId:id,approved:approved})}
  property var connectionPreferences:({busy:false,error:"",client:{},github:{environments:[]}})
  readonly property string primaryEnvironmentId:connectionPreferences.primaryEnvironmentId || ""
  function manageConnectionPreferences(action,values){
    if(connectionPreferences.busy)return
    if(!worker.running){connectionPreferences=Object.assign({},connectionPreferences,{error:"disconnected"});return}
    var id=uuid(),command={op:"connections-manage",action:action==="read"?"preferences-read":action,requestId:id,revision:connectionPreferences.catalogRevision || "",clientRevision:(connectionPreferences.client || {}).revision || ""}
    if(action==="client-update")command.patch=values || {}
    else command=Object.assign(command,values || {})
    connectionPreferences=Object.assign({},connectionPreferences,{busy:true,error:"",requestId:id});send(command)
  }
  property var connectionNetwork:({busy:false,error:"",data:null})
  signal connectionNetworkFinished(string action,string error)
  function requestConnectionNetwork(action,values){
    if(connectionNetwork.busy)return
    if(!worker.running){connectionNetwork=Object.assign({},connectionNetwork,{error:"disconnected"});return}
    var id=uuid();connectionNetwork=Object.assign({},connectionNetwork,{busy:true,error:"",requestId:id,action:action})
    send(Object.assign({op:"connection-network",action:action,revision:connectionNetwork.data?connectionNetwork.data.revision:"",requestId:id},values || {}))
  }
  property var connectionAccess:({busy:false,error:"",environment:"",data:null,created:null})
  signal connectionAccessFinished(string action,string error)
  function requestConnectionAccess(environment,action,values){
    if(!environment)return
    if(connectionAccess.busy&&connectionAccess.environment===environment)return
    if(!worker.running){connectionAccess={busy:false,error:"disconnected",environment:environment,data:null,created:null};return}
    var id=uuid();connectionAccess={busy:true,error:"",environment:environment,requestId:id,action:action,data:connectionAccess.environment===environment?connectionAccess.data:null,created:action==="create-link"?null:connectionAccess.environment===environment?connectionAccess.created:null}
    send(Object.assign({op:"connection-access",environment:environment,action:action,requestId:id},values || {}))
  }
  function clearConnectionCredential(){connectionAccess=Object.assign({},connectionAccess,{created:null})}
  function dismissConnectionAccess(){connectionAccess={busy:false,error:"",environment:"",data:null,created:null,requestId:""}}
  function requestSettings(environment,key,value,scope,clear){
    if(settingsResult.busy&&key)return
    var id=uuid(),signature=scope?JSON.stringify(scope):""
    settingsResult=Object.assign({},settingsResult,{busy:true,error:"",requestId:id,environment:environment,scopeSignature:signature,errors:[],savedTargets:[],writable:false,fields:settingsResult.environment===environment&&settingsResult.scopeSignature===signature?settingsResult.fields:[],storageScopes:settingsResult.environment===environment&&settingsResult.scopeSignature===signature?settingsResult.storageScopes || []:[]})
    var command={op:"settings",action:clear?"clear":key?"update":"read",key:key || "",value:value,environment:environment,requestId:id}
    if(scope)command.scope=scope
    send(command)
  }
  property var prOperations:({})
  function requestPullRequests(action,values){var id=uuid(),all=Object.assign({},prOperations);all[action]={busy:true,requestId:id,error:"",data:null};prOperations=all;send(Object.assign({op:"pull-requests",environment:selectedEnvironment,action:action,requestId:id},values || {}))}
  function resetPullRequests(){prOperations=({})}
  property var scrollPolicy:({touchpadNative:false,mouseNative:false})
  property var favoritePending:({})
  property string favoriteError:""
  function refreshCatalog(){refreshCatalogIn(selectedEnvironment)}
  function refreshCatalogIn(environment){if(environment)send({op:"catalog",environment:environment})}
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
  Timer{interval:300;repeat:true;running:root.panelVisible&&root.scrollPolicy.trackUnderlay===true;onTriggered:root.probeScroll()}
  Timer{id:scrollProbeDelay;interval:150;onTriggered:if(root.panelVisible)root.probeScroll()}
  property var providers: []
  property var quotaSnapshot:({environments:[],localSources:[]})
  property var quotaNotices:[]
  property var quotaExternal:[]
  function presentQuotas(){var view=Usage.present(quotaSnapshot,Date.now());providers=view.providers;quotaNotices=view.notices;quotaExternal=view.external}
  function refreshQuotas(){send({op:"quotas-refresh"})}
  Timer{interval:60000;repeat:true;running:root.panelVisible;onTriggered:root.presentQuotas()}
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
    if(rewindPending||!selectedThread || queueOperation.busy || draft.busy || draft.attempt)return
    if(queueOperation.command && action)return
    var command=queueOperation.command || Object.assign({op:"queue",action:action,environment:selectedEnvironment,thread:selectedThread,requestId:uuid()},values || {})
    var all=Object.assign({},queueOperations);all[selectedKey]={busy:true,error:"",command:command};queueOperations=all;send(command)
  }
  function beginQueueEdit(item) {
    if(rewindPending||item.available===false||draft.busy||draft.attempt||draft.loading||draft.loadError||draft.queueEdit||queueOperation.command||draft.attachments.some(function(f){return f.uploading}))return
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
  // Capture sidebar order before Settle changes grouping. A selection epoch also
  // prevents an away-and-back navigation during the RPC from being overridden.
  property int selectionEpoch:0
  onSelectedKeyChanged:selectionEpoch++
  property var settleNavigation:null
  property var settleNavigationOrder:[]
  signal settleNavigationPlanning()
  signal settledLastThread(var thread)
  function planSettleNavigation(command){
    if(command.action!=="settle"||selectedKey!==command.environment+"/"+command.thread)return null
    settleNavigationOrder=state.threads || []
    settleNavigationPlanning()
    var order=settleNavigationOrder,at=order.findIndex(function(t){return t.key===root.selectedKey})
    var candidates=at<0?[]:order.slice(at+1).concat(order.slice(0,at)).map(function(t){return t.key})
    return {requestId:command.requestId,key:selectedKey,epoch:selectionEpoch,candidates:candidates,thread:Object.assign({},selectedInfo)}
  }
  function finishSettleNavigation(plan){
    if(!plan||plan.epoch!==selectionEpoch||plan.key!==selectedKey)return
    var next=null
    for(var i=0;i<plan.candidates.length;i++){
      var key=plan.candidates[i]
      next=(state.threads || []).find(function(t){return t.key===key&&!t.settled&&!t.snoozed&&!t.archivedAt&&!t.deletedAt&&!t.providerSubagent&&t.relationshipToParent!=="subagent"})
      if(next)break
    }
    if(next)selectThread(next.environmentId,next.id)
    else {clearSelection();settledLastThread(plan.thread)}
  }
  property var managementRequest: null
  property var folderBrowse:({busy:false,error:"",entries:[],parentPath:""})
  property var projectSources:({busy:false,providers:[]})
  property var pendingProjectOpen:null
  signal projectOpened(string environment,string projectId,string error)
  function openProject(environment,projectId,createThread){
    if(managementBusy||managementRequest)return
    pendingProjectOpen={environment:environment,projectId:projectId,requestId:"",createThread:createThread===true}
    managementError="";finishProjectOpen();refresh()
  }
  function finishProjectOpen(){
    var pending=pendingProjectOpen
    if(!pending||pending.requestId||managementBusy||managementRequest)return
    var project=(state.projects || []).find(function(p){return p.environmentId===pending.environment&&p.id===pending.projectId})
    if(!project)return
    var threads=(state.threads || []).filter(function(t){return t.environmentId===pending.environment&&t.projectId===pending.projectId&&!t.settled})
    if(threads.length&&!pending.createThread){pendingProjectOpen=null;selectThread(pending.environment,threads[0].id);projectOpened(pending.environment,pending.projectId,"");return}
    var selection=project.defaultModelSelection || (selectedInfo.projectId===pending.projectId&&!selectedInfo.settled&&selectedEnvironment===pending.environment?(draft.modelSelection || selectedInfo.modelSelection):null)
    if(!selection){var template=(state.threads || []).find(function(t){return t.environmentId===pending.environment&&t.projectId===pending.projectId&&!t.settled&&t.modelSelection});selection=template?template.modelSelection:null}
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
    if(!managementRequest)settleNavigation=planSettleNavigation(command)
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
    if (!all[key].loading && !all[key].loadError && ["text","attachments","modelSelection","runtimeMode","interactionMode","sourcePlanRef","delivery","attempt","queueEdit","questionnaires","approvals"].some(function(k){return values[k] !== undefined})) {
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
    // The native New Local command explicitly selects the project checkout;
    // it must not rewrite or be overridden by the saved workspace default.
    if(!request.sourceKey&&request.command.workspaceStrategy&&request.command.workspaceStrategy.type==="root"){
      manage(Object.assign({},request.command,{automatic:false,workspaceStrategy:{type:"root"}}));return
    }
    if(request.command.automatic===true&&!request.sourceKey){
      var id=uuid();workspaceOperation={busy:true,error:"",request:request,requestId:id,action:"routing"}
      send(Object.assign({op:"connection-route",requestId:id},request.command));return
    }
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
    if(rewindPending)return
    if(planOperation.busy&&!draft.attempt)return
    if(providerSubagent&&startNewThread!==true)return
    if(workspaceOperation.busy)return
    if((startNewThread===true||draft.attempt&&draft.attempt.op==="manage")&&(activeQuestion||managementBusy||(managementRequest&&(!draft.attempt||managementRequest.requestId!==draft.attempt.requestId))||draft.queueEdit||(!draft.attempt&&!selectedInfo.projectId)))return
    if(activeQuestion){questionSubmitRequested();return}
    if (!selectedThread || draft.loading || draft.loadError || draft.busy || draft.attachments.some(function(f) { return f.uploading || f.error })) return
    if(!draft.attempt&&interactionModeEnabled&&/^\/(plan|default)$/.test(draft.text.trim())){chooseInteractionMode(draft.text.trim().slice(1));patchDraft(selectedKey,{text:""});flushDraft(selectedKey);return}
    if (!draft.attempt && !draft.text.trim() && draft.attachments.length === 0) return
    var attempt = draft.attempt || {op: "send", environment: selectedEnvironment, thread: selectedThread,
      requestId: uuid(), text: draft.text, attachments: draft.attachments.map(function(f) { return f.attachment }),
      modelSelection: draft.modelSelection || selectedInfo.modelSelection, delivery: selectedInfo.activeRunId ? "queue" : draft.delivery}
    if(!draft.attempt&&draft.runtimeMode&&draft.runtimeMode!==selectedInfo.runtimeMode)attempt.runtimeMode=draft.runtimeMode
    if(!draft.attempt&&(interactionModeEnabled||selectedInfo.interactionMode==="plan"))attempt.interactionMode=interactionMode
    if(!draft.attempt&&draft.sourcePlanRef)attempt.sourcePlanRef=draft.sourcePlanRef
    if(startNewThread===true&&!draft.attempt&&!workspaceStrategy){
      beginNewThread({sourceKey:selectedKey,command:{environment:selectedEnvironment,projectId:selectedInfo.projectId,branch:selectedInfo.branch,worktreePath:selectedInfo.worktreePath}});return
    }
    if(startNewThread===true&&!draft.attempt)attempt=Object.assign({},attempt,{op:"manage",action:"new-thread",sendRequestId:uuid(),title:T3I18n.tr("New chat"),projectId:selectedInfo.projectId,runtimeMode:draft.runtimeMode || selectedInfo.runtimeMode,branch:selectedInfo.branch,worktreePath:selectedInfo.worktreePath,workspaceStrategy:workspaceStrategy,launchWithMessage:true,interactionMode:interactionMode,delivery:"auto"})
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
    Devices.lost(root);Profiles.lost(root)
    scriptShortcutPending=null;keybindingInvalidations=({})
    if(licenseView.busy)licenseView=Object.assign({},licenseView,{busy:false,error:"disconnected"})
    if(licenseNotice.busy)licenseNotice=Object.assign({},licenseNotice,{busy:false,error:"disconnected"})
    var all=Object.assign({},rewinds);Object.keys(all).forEach(function(key){if(["prepared","accepted","pending"].indexOf(all[key].state)>=0)all[key]=Object.assign({},all[key],{state:"uncertain",error:"disconnected"})});rewinds=all;rewindRequests=({});rewindRecoveries=({});rewindPreview={busy:false,error:"disconnected",requestId:""}
    if(workingReview.scope.requestId)workingReview=Object.assign({},workingReview,{loading:false,error:"disconnected"})
    var refs=Object.assign({},reviewRefs);["local","remote"].forEach(function(k){if(refs[k].busy)refs[k]=Object.assign({},refs[k],{busy:false,error:"disconnected"})});reviewRefs=refs
    if(keybindingOpen.busy)keybindingOpen=Object.assign({},keybindingOpen,{busy:false,error:"disconnected"})
    var configs=Object.assign({},keybindingConfigs);Object.keys(configs).forEach(function(id){configs[id]=Object.assign({},configs[id],{busy:false,error:"disconnected"})});keybindingConfigs=configs
    if(keybindingWrite.busy){var results=Object.assign({},keybindingWrite.results);Object.keys(results).forEach(function(id){if(results[id].state==="pending")results[id]={state:"uncertain",error:"disconnected"}});keybindingWrite=Object.assign({},keybindingWrite,{busy:false,results:results})}keybindingCommands=({})
    var archive=Object.assign({},archiveActions);Object.keys(archive).forEach(function(k){if(root.archiveBlocking(archive[k]))archive[k]=Object.assign({},archive[k],{state:"uncertain",error:"disconnected"})});archiveActions=archive;archiveCommands=({})
    var views=Object.assign({},archiveView.sessions);Object.keys(views).forEach(function(k){if(views[k].busy)views[k]=Object.assign({},views[k],{busy:false,error:"disconnected"})});archiveView=Object.assign({},archiveView,{sessions:views})
    gitCommands=({})
    if(gitView.scope.requestId)gitView=Object.assign({},gitView,{loading:false,error:"disconnected",operation:gitView.operation.state==="pending"?Object.assign({},gitView.operation,{state:"unknown"}):gitView.operation})
    if(clientSettings.busy){clientSettings=Object.assign({},clientSettings,{busy:false,error:"disconnected"});if(diffPreferenceUpdate.stage)diffPreferenceUpdate=Object.assign({},diffPreferenceUpdate,{stage:"",error:"disconnected"})}
    if(planOperation.busy)planOperation=Object.assign({},planOperation,{busy:false,error:planOperation.command?"rpc_outcome_unknown":"disconnected"})
    if(backendStart.busy)backendStart={busy:false,error:"rpc_outcome_unknown",status:""}
    if(terminalOperation.busy)terminalOperation=Object.assign({},terminalOperation,{busy:false,error:"rpc_outcome_unknown"})
    if(workspaceOperation.busy){workspaceOperation=Object.assign({},workspaceOperation,{busy:false,error:"rpc_outcome_unknown"});newThreadWorkspaceRequested(workspaceOperation.request)}
    if(connectionCloud.busy)connectionCloud=Object.assign({},connectionCloud,{busy:false,error:"rpc_outcome_unknown",device:null})
    if(connectionPreferences.busy)connectionPreferences=Object.assign({},connectionPreferences,{busy:false,error:"rpc_outcome_unknown"})
    if(connectionNetwork.busy)connectionNetwork=Object.assign({},connectionNetwork,{busy:false,error:"rpc_outcome_unknown"})
    if(connectionAccess.busy)connectionAccess=Object.assign({},connectionAccess,{busy:false,error:"rpc_outcome_unknown",created:null})
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
  function imageKey(a){return selectedEnvironment+"/"+(a.source?selectedThread+"/":"")+a.id}
  function requestImage(a){
    var key=imageKey(a);if(imageStates[key])return
    var all=Object.assign({},imageStates),id=uuid();all[key]={loading:true,url:"",error:"",requestId:id};imageStates=all
    send({op:"image",environment:selectedEnvironment,thread:selectedThread,requestId:id,attachment:a})
  }
  property var documentOpen:({busy:false,error:"",requestId:"",data:null,frameUrl:"",frameRevision:0})
  property var inlineDocuments:({})
  function openInlineDocument(attachment){
    var id=uuid(),all=Object.assign({},inlineDocuments)
    all[id]={busy:true,error:"",requestId:id,data:null,frameUrl:"",frameRevision:0};inlineDocuments=all
    send({op:"document-open",inline:true,environment:selectedEnvironment,thread:selectedThread,attachment:attachment,requestId:id,theme:DocumentTheme.theme(Color.popups.background,Color.popups.text,Color.accent,T3Palette.values)})
    return id
  }
  function openReviewDocument(snapshotId,layout,wrap,filePath){
    var id=uuid(),all=Object.assign({},inlineDocuments)
    all[id]={busy:true,error:"",requestId:id,data:null,frameUrl:"",frameRevision:0};inlineDocuments=all
    send({op:"document-open",inline:true,environment:selectedEnvironment,thread:selectedThread,reviewSnapshotId:snapshotId,filePath:filePath,layout:layout,wrap:wrap,requestId:id,theme:DocumentTheme.theme(Color.popups.background,Color.popups.text,Color.accent,T3Palette.values)})
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
    var project=(state.projects || []).find(function(p){return p.environmentId===root.selectedEnvironment&&p.id===root.selectedInfo.projectId}) || {}
    send({op:"document-open",environment:selectedEnvironment,thread:selectedThread,cwd:selectedInfo.worktreePath || project.workspaceRoot || "",path:path,attachment:attachment || null,requestId:id,labels:{source:T3I18n.tr("Source"),preview:T3I18n.tr("Preview"),close:T3I18n.tr("Close")},theme:DocumentTheme.theme(Color.popups.background,Color.popups.text,Color.accent,T3Palette.values)})
  }
  property TextEdit documentClipboardBuffer:TextEdit{visible:false;textFormat:TextEdit.PlainText}
  function documentInput(event){if(documentOpen.requestId)send({op:"document-input",requestId:documentOpen.requestId,event:event})}
  function closeDocument(){if(documentOpen.requestId)send({op:"document-close",requestId:documentOpen.requestId});documentOpen={busy:false,error:"",requestId:"",data:null,frameUrl:"",frameRevision:0}}
  signal toolInspectorFinished(string requestId,var result,string error)
  property var rewinds:({})
  property var rewindRequests:({})
  property var rewindRecoveries:({})
  property var rewindPreview:({busy:false,error:"",requestId:""})
  readonly property var rewindOperation:rewinds[selectedKey] || {}
  readonly property bool rewindPending:!!rewindOperation.requestId&&(["prepared","accepted","pending","uncertain","superseded"].indexOf(rewindOperation.state)>=0||rewindOperation.state==="complete"&&!rewindOperation.recovered)
  readonly property var rollbackProvider:(catalog.providers || []).find(function(p){return p.instanceId===(root.selectedInfo.providerInstanceId || (root.selectedInfo.modelSelection || {}).instanceId)}) || null
  readonly property bool rewindConnected:(state.environments || []).some(function(e){return e.id===root.selectedEnvironment&&e.state==="ready"})
  readonly property bool canRewind:!!selectedThread&&!providerSubagent&&rewindConnected&&catalog.canWrite===true&&!!rollbackProvider&&rollbackProvider.supportsConversationRollback!==false&&!selectedInfo.working&&!draft.busy&&!draft.attempt&&!draft.loading&&!draft.loadError&&!draft.queueEdit&&!loadingMessages
  signal rewindReady(var reference,bool canRestoreFiles)
  signal rewindReceipt(var receipt)
  signal rewindResume(var request,string state)
  signal rewindRestored(string key)
  function rememberRewind(r){
    if(!r||!r.requestId||!r.environment||!r.thread)return
    var key=r.environment+"/"+r.thread,old=rewinds[key]
    if(old&&old.requestId===r.requestId&&["complete","failed","reviewed"].indexOf(old.state)>=0&&old.state!==r.state)return
    if(old&&old.requestId===r.requestId&&old.recovered===true&&r.recovered!==true)return
    if(old&&old.requestId!==r.requestId&&["prepared","accepted","uncertain","complete","superseded"].indexOf(old.state)>=0&&(old.startedAt || 0)>(r.startedAt || 0))return
    if(old&&old.requestId!==r.requestId&&["prepared","accepted","uncertain","pending","superseded"].indexOf(old.state)>=0)rewindReceipt(Object.assign({},old,{state:"superseded",error:"revert_superseded"}))
    var all=Object.assign({},rewinds);all[key]=r;rewinds=all
    if(r.state==="complete"&&!r.recovered)recoverRewind(r)
    else rewindReceipt(r)
  }
  function requestRewindPreview(messageId){
    if(!canRewind||rewindPending||rewindPreview.busy)return
    var id=uuid();rewindPreview={busy:true,error:"",requestId:id,key:selectedKey,messageId:messageId}
    send({op:"rewind-preview",environment:selectedEnvironment,thread:selectedThread,requestId:id,messageId:messageId})
  }
  function requestRewind(request){
    var key=request.environment+"/"+request.thread,d=drafts[key]
    if(key!==selectedKey||!d||d.loading||d.loadError||d.attempt||d.busy||d.queueEdit)return
    if(rewindPending&&(rewindOperation.requestId!==request.requestId||request.retry!==true))return
    var command=Object.assign({op:"rewind-dispatch"},request)
    var all=Object.assign({},rewindRequests);all[request.requestId]=command;rewindRequests=all
    var states=Object.assign({},rewinds);states[key]=Object.assign({},(rewinds[key] || {}).requestId===request.requestId?rewinds[key]:{},request,{state:"prepared",recovered:false,startedAt:(rewinds[key] || {}).requestId===request.requestId?(rewinds[key].startedAt || Date.now()):Date.now()});rewinds=states
    flushDraft(key,command)
  }
  function refreshRewind(r){
    r=r || rewindOperation;if(!r.environment||!r.thread)return
    if(Object.keys(rewindRequests).some(function(id){var c=root.rewindRequests[id];return c.op==="rewind-observe"&&c.environment===r.environment&&c.thread===r.thread}))return
    var id=uuid(),command={op:"rewind-observe",environment:r.environment,thread:r.thread,operationId:r.requestId,requestId:id}
    var all=Object.assign({},rewindRequests);all[id]=command;rewindRequests=all;send(command)
  }
  function showPendingRewind(){var r=rewindOperation;if(!r.requestId)return;rewindResume(r,r.state==="complete"?(rewindRecoveries[selectedKey]?"recovering":"recovery_failed"):r.state==="superseded"?"superseded":r.state==="accepted"||r.state==="prepared"?"waiting":"uncertain")}
  function recoverRewind(r){
    var key=r.environment+"/"+r.thread;if(rewindRecoveries[key])return
    var d=drafts[key],id=uuid(),command={op:"rewind-recover",environment:r.environment,thread:r.thread,operationId:r.requestId,requestId:id,draft:d&&!d.loading?JSON.parse(JSON.stringify(d)):null}
    var all=Object.assign({},rewindRecoveries);all[key]=id;rewindRecoveries=all
    var states=Object.assign({},rewinds);states[key]=Object.assign({},states[key] || r,{recoveryError:""});rewinds=states
    if(d&&!d.loading){patchDraft(key,{busy:true});flushDraft(key)}
    var requests=Object.assign({},rewindRequests);requests[id]=command;rewindRequests=requests
    rewindReceipt(Object.assign({},r,{state:"recovering"}));send(command)
  }
  function reconcileRewind(r){
    r=r || rewindOperation;if(!r.requestId)return
    var id=uuid(),command={op:"rewind-resolve",environment:r.environment,thread:r.thread,operationId:r.requestId,requestId:id,confirmed:true}
    var all=Object.assign({},rewindRequests);all[id]=command;rewindRequests=all;send(command)
  }
  Timer{interval:1800;repeat:true;running:Object.keys(root.rewinds).some(function(key){return ["prepared","accepted","pending"].indexOf(root.rewinds[key].state)>=0});onTriggered:Object.keys(root.rewinds).forEach(function(key){var r=root.rewinds[key];if(["prepared","accepted","pending"].indexOf(r.state)>=0)root.refreshRewind(r)})}
  readonly property string primaryKeybindingEnvironment:typeof state.primaryEnvironmentId==="string"?state.primaryEnvironmentId:state.primaryEnvironmentId===null?"":(state.environments || []).some(function(e){return e.id==="local"})?"local":""
  function keybindingRulesFor(environment){var c=keybindingConfigs[environment];return c&&c.data?NativeKeys.nativeKeys.mergeWithDefaultKeybindings(c.data.keybindings):NativeKeys.nativeKeys.DEFAULT_RESOLVED_KEYBINDINGS}
  function ensureRuntimeKeybindings(force){
    if(!panelVisible||keybindingCaptureActive||keybindingWrite.busy||keybindingScope.environments.length)return
    var ids=[primaryKeybindingEnvironment,selectedEnvironment].filter(function(id,i,a){return id&&a.indexOf(id)===i&&(root.state.environments || []).some(function(e){return e.id===id&&e.state==="ready"})})
    ids.forEach(function(id){root.ensureKeybindings(id,!!force)})
  }
  onSelectedEnvironmentChanged:{if(panelVisible)ensureRuntimeKeybindings(false);Qt.callLater(function(){root.syncPreview(false)})}
  readonly property var keybindingWatchEnvironments:panelVisible?[primaryKeybindingEnvironment,selectedEnvironment].concat(keybindingScope.environments).filter(function(id,i,a){return id&&a.indexOf(id)===i&&(root.state.environments || []).some(function(e){return e.id===id&&e.state==="ready"})}).sort():[]
  property string keybindingWatchSignature:""
  property string keybindingWatchRequest:""
  property var keybindingInvalidations:({})
  onKeybindingWatchEnvironmentsChanged:syncKeybindingWatches()
  function syncKeybindingWatches(force){
    var signature=JSON.stringify(keybindingWatchEnvironments)
    if(!worker.running||!force&&signature===keybindingWatchSignature)return
    keybindingWatchSignature=signature;keybindingWatchRequest=uuid();keybindingInvalidations=({})
    send({op:"keybindings-watch",requestId:keybindingWatchRequest,environments:keybindingWatchEnvironments})
  }
  function invalidateKeybindings(data){
    if(!panelVisible||data.requestId!==keybindingWatchRequest||keybindingWatchEnvironments.indexOf(data.environment)<0)return
    var pending=Object.assign({},keybindingInvalidations);pending[data.environment]=true;keybindingInvalidations=pending
  }
  Timer{interval:120;repeat:true;running:root.panelVisible&&Object.keys(root.keybindingInvalidations).length>0&&!root.keybindingCaptureActive&&!root.keybindingWrite.busy;onTriggered:{
    var pending=Object.assign({},root.keybindingInvalidations)
    Object.keys(pending).forEach(function(id){var c=root.keybindingConfigs[id];if(c&&c.busy||Object.keys(root.keybindingCommands).some(function(key){return root.keybindingCommands[key].environment===id}))return;delete pending[id];root.ensureKeybindings(id,true)})
    root.keybindingInvalidations=pending
  }}
  Timer{interval:30000;repeat:true;running:root.panelVisible&&!root.keybindingScope.environments.length&&!root.keybindingCaptureActive&&!root.keybindingWrite.busy;onTriggered:root.ensureRuntimeKeybindings(true)}
  property var keybindingConfigs:({})
  // Metadata subscription is active only while the native browser view is visible.
  property bool previewVisible:false
  readonly property bool previewWorkerReady:worker.running
  property var previewStores:({})
  property var previewState:null
  property string browserWorkspaceId:""
  function mountBrowserWorkspace(){browserWorkspaceId=uuid();previewVisible=true;syncPreview(false);return browserWorkspaceId}
  function unmountBrowserWorkspace(owner){if(owner!==browserWorkspaceId)return;browserWorkspaceId="";previewVisible=false;syncPreview(false)}
  property bool browserPageMounted:false
  property string browserMountId:""
  property int browserPageWidth:900
  property int browserPageHeight:620
  property var browserCleanups:({})
  property var browserSurface:BrowserSurface.empty()
  function mountBrowserPage(width,height){browserMountId=uuid();browserPageMounted=true;BrowserSurface.resize(root,width,height);syncBrowserSurface(true);return browserMountId}
  function unmountBrowserPage(owner){if(owner&&owner!==browserMountId)return;browserMountId="";browserPageMounted=false;BrowserSurface.dismiss(root)}
  function resizeBrowserPage(width,height,owner){if(owner&&owner!==browserMountId)return;BrowserSurface.resize(root,width,height)}
  function browserPageInput(id,event,owner){if(owner&&owner!==browserMountId)return;BrowserSurface.input(root,id,event)}
  function browserControl(action){BrowserSurface.control(root,action)}
  function syncBrowserSurface(force){BrowserSurface.sync(root,!!force)}
  onPreviewStateChanged:Qt.callLater(function(){root.syncBrowserSurface(false)})
  onPreviewWatchChanged:Qt.callLater(function(){root.syncBrowserSurface(false)})
  property var previewCommands:({})
  property var previewOperations:({})
  function activatePreviewTab(tab){Previews.activate(root,tab)}
  function previewAction(action,options){return Previews.command(root,action,options)}
  function acknowledgePreviewOperation(){Previews.acknowledge(root)}
  property var previewWatch:({signature:"",requestId:"",environment:"",thread:"",originKey:"",busy:false,connected:false,error:"",canOperate:false,serverBrowser:false})
  function syncPreview(force){Previews.sync(root,!!force)}
  onPreviewVisibleChanged:Qt.callLater(function(){root.syncPreview(false)})
  onSelectedThreadChanged:Qt.callLater(function(){root.syncPreview(false)})
  property var browserImportCatalog:({busy:false,error:"",requestId:"",sources:[]})
  property var browserImportView:({environment:"",requestId:"",busy:false,error:"",data:null})
  property var browserImportOperation:({state:"",error:""})
  property var browserImportWizard:({open:false,step:{step:"configure"}})
  function requestBrowserImportSources(){ImportFlow.loadSources(root)}
  function openBrowserImport(source){return ImportFlow.open(root,source)}
  function chooseBrowserImportSource(directory){ImportFlow.chooseSource(root,directory)}
  function chooseBrowserImportTarget(kind,id){ImportFlow.chooseTarget(root,kind,id)}
  function refreshBrowserImport(retry){return ImportFlow.refresh(root,!!retry)}
  function runBrowserImport(){return ImportFlow.dispatch(root)}
  function closeBrowserImport(acknowledge){return ImportFlow.close(root,!!acknowledge)}
  property var browserProfileView:({signature:"",environments:[],requestId:"",busy:false,error:"",data:null})
  property var browserProfileOperation:({state:"",error:"",results:[]})
  property bool browserProfileNeedsRefresh:false
  function requestBrowserProfiles(targets,force){Profiles.select(root,targets,force)}
  function browserProfileAction(action,profile){return Profiles.dispatch(root,action,profile)}
  function acknowledgeBrowserProfile(){Profiles.acknowledge(root)}
  function dismissBrowserProfiles(){Profiles.dismiss(root)}
  Timer{interval:100;running:root.browserProfileNeedsRefresh&&!root.clientSettings.busy;onTriggered:{root.browserProfileNeedsRefresh=false;root.requestClientSettings()}}
  readonly property bool deviceWorkerReady:worker.running
  property var deviceView:({signature:"",targets:[],sessions:{},error:""})
  property var deviceOperations:({})
  property var deviceCommands:({})
  property var deviceLiveStates:({})
  property string deviceWatchRequest:""
  function requestDevices(targets,force){Devices.select(root,targets,force)}
  function dismissDevices(){Devices.dismiss(root)}
  function deviceAllowed(environment,action){return Devices.allowed(root,environment,action)}
  function deviceAction(action,input,environments){return Devices.dispatch(root,action,input,environments)}
  function acknowledgeDevice(environment){Devices.acknowledge(root,environment)}
  function refreshDevice(environment){Devices.read(root,environment)}
  property var licenseView:({busy:false,error:"",data:null,requestId:""})
  property var licenseNotice:({busy:false,error:"",manifestId:"",entryKey:"",noticeText:"",requestId:""})
  function requestLicenses(force){
    if(licenseView.busy||!force&&licenseView.data)return
    var id=uuid();licenseView={busy:worker.running,error:worker.running?"":"disconnected",data:licenseView.data,requestId:id}
    if(worker.running)send({op:"licenses-read",requestId:id})
  }
  function requestLicenseNotice(entryKey){
    var data=licenseView.data;if(!data)return
    if(licenseNotice.entryKey===entryKey&&licenseNotice.manifestId===data.manifestId&&(licenseNotice.busy||!licenseNotice.error&&licenseNotice.requestId))return
    var id=uuid();licenseNotice={busy:worker.running,error:worker.running?"":"disconnected",manifestId:data.manifestId,entryKey:entryKey,noticeText:"",requestId:id}
    if(worker.running)send({op:"licenses-notice",manifestId:data.manifestId,entryKey:entryKey,requestId:id})
  }
  function dismissLicenseNotice(){licenseNotice={busy:false,error:"",manifestId:"",entryKey:"",noticeText:"",requestId:""}}
  property var keybindingScope:({environments:[],signature:"",error:""})
  property var keybindingCommands:({})
  property var keybindingWrite:({busy:false,signature:"",results:{}})
  property var keybindingOpen:({busy:false,error:"",environment:"",requestId:""})
  function openKeybindings(environment,viewId){if(keybindingOpen.busy||!worker.running)return;var id=uuid();keybindingOpen={busy:true,error:"",environment:environment,viewId:viewId,requestId:id};send({op:"keybindings-open",environment:environment,viewId:viewId,requestId:id})}
  function dismissKeybindingResult(){if(!keybindingWrite.busy)keybindingWrite={busy:false,signature:"",results:{}}}
  property bool keybindingCaptureActive:false
  function ensureKeybindings(environment,force){
    if(!environment||keybindingConfigs[environment]&&keybindingConfigs[environment].busy||!force&&keybindingConfigs[environment]||Object.keys(keybindingCommands).some(function(id){return root.keybindingCommands[id].environment===environment}))return
    var id=uuid(),configs=Object.assign({},keybindingConfigs),prior=configs[environment]
    configs[environment]={busy:worker.running,error:worker.running?"":"disconnected",requestId:id,data:prior?prior.data:null};keybindingConfigs=configs;if(worker.running)send({op:"keybindings-read",environment:environment,requestId:id})
  }
  function requestKeybindings(targets,force){
    var ids=[],bad=false;(targets || []).forEach(function(t){if(!t||typeof t.environment!=="string"||!t.environment){bad=true;return}if(ids.indexOf(t.environment)<0)ids.push(t.environment)})
    if(bad)ids=[]
    keybindingScope={environments:ids,signature:JSON.stringify(ids),error:bad?"invalid_settings_scope":ids.length?"":"settings_scope_empty"}
    ids.forEach(function(id){root.ensureKeybindings(id,!!force)})
  }
  function dismissKeybindings(){keybindingScope={environments:[],signature:"",error:""};keybindingCaptureActive=false}
  function writeKeybinding(action,input){
    var ids=keybindingScope.environments
    if(!worker.running||keybindingWrite.busy||!ids.length||["upsert","remove"].indexOf(action)<0||!ids.every(function(id){var c=root.keybindingConfigs[id];return c&&!c.busy&&!c.error&&c.data&&c.data.writable&&c.data.viewId}))return false
    if(!input||!NativeKeys.nativeKeys.parseKeybindingShortcut(input.key)||input.when&&!NativeKeys.nativeKeys.parseWhenExpressionDraft(input.when).ok)return false
    var group=uuid(),commands=Object.assign({},keybindingCommands),results={},out=[]
    ids.forEach(function(environment){var id=root.uuid(),data=root.keybindingConfigs[environment].data,command={op:"keybindings-write",environment:environment,requestId:id,viewId:data.viewId,originKey:data.originKey,action:action,input:JSON.parse(JSON.stringify(input)),group:group};commands[id]=command;results[environment]={state:"pending",error:""};out.push(command)})
    keybindingCommands=commands;keybindingWrite={busy:true,group:group,signature:keybindingScope.signature,action:action,input:input,results:results}
    out.forEach(function(command){root.send(command)});return true
  }
  property var archiveView:({signature:"",targets:[],sessions:{},error:""})
  property var archiveCommands:({})
  property var archiveActions:({})
  function archiveKey(r){return r.environment+"/"+r.originKey+"/"+(r.threadId || r.id)}
  function archiveBlocking(r){return r&&["pending","accepted","uncertain"].indexOf(r.state)>=0}
  function dismissArchive(){archiveView={signature:"",targets:[],sessions:{},error:""}}
  function requestArchive(targets,force){
    var by={},bad=false,copy=JSON.parse(JSON.stringify(targets || []))
    copy.forEach(function(t){if(!t||typeof t.environment!=="string"||!t.environment||t.projectId!==null&&(typeof t.projectId!=="string"||!t.projectId)){bad=true;return}var g=by[t.environment];if(!g)g=by[t.environment]={environment:t.environment,projectIds:[]};if(t.projectId===null)g.projectIds=null;else if(g.projectIds&&g.projectIds.indexOf(t.projectId)<0)g.projectIds.push(t.projectId)})
    var groups=Object.keys(by).sort().map(function(k){var g=by[k];if(g.projectIds)g.projectIds.sort();return g}),signature=JSON.stringify(groups)
    if(!force&&signature===archiveView.signature)return
    if(bad)groups=[]
    var prior=archiveView,sessions={}
    groups.forEach(function(g){var id=root.uuid(),old=prior.signature===signature&&prior.sessions[g.environment];sessions[g.environment]={busy:worker.running,error:worker.running?"":"disconnected",requestId:id,scope:g,data:old?old.data:null}})
    archiveView={signature:signature,targets:copy,sessions:sessions,error:bad?"invalid_archive_scope":groups.length?"":"settings_scope_empty"}
    if(bad||!worker.running)return
    groups.forEach(function(g){root.send(Object.assign({op:"archive-read",requestId:sessions[g.environment].requestId},g))})
  }
  function requestArchiveAction(row,operation,confirmed){
    if(!worker.running||!row||!row.writable||["unarchive","delete"].indexOf(operation)<0||operation==="delete"&&!confirmed||archiveBlocking(archiveActions[archiveKey(row)]))return false
    var id=uuid(),command={op:"archive-action",environment:row.environment,thread:row.id,viewId:row.viewId,operation:operation,confirmed:!!confirmed,requestId:id,actionId:id}
    var commands=Object.assign({},archiveCommands);commands[id]=command;archiveCommands=commands
    var actions=Object.assign({},archiveActions);actions[archiveKey(row)]={state:"pending",actionId:id,environment:row.environment,originKey:row.originKey,threadId:row.id,projectId:row.projectId,title:row.title,operation:operation,error:""};archiveActions=actions;send(command);return true
  }
  function checkArchive(record,retry){
    if(!worker.running||!record.actionId)return
    if(Object.keys(archiveCommands).some(function(id){return root.archiveCommands[id].actionId===record.actionId}))return
    var id=uuid(),command={op:retry?"archive-action":"archive-state",environment:record.environment,thread:record.threadId,actionId:record.actionId,requestId:id,operation:record.operation,retry:!!retry}
    var commands=Object.assign({},archiveCommands);commands[id]=command;archiveCommands=commands;send(command)
  }
  property var reviewRefs:({query:"",local:{refs:[],busy:false,error:"",scope:{},nextCursor:null},remote:{refs:[],busy:false,error:"",scope:{},nextCursor:null}})
  function dismissReviewRefs(){reviewRefs={query:"",local:{refs:[],busy:false,error:"",scope:{},nextCursor:null},remote:{refs:[],busy:false,error:"",scope:{},nextCursor:null}}}
  function requestReviewRefs(query,kind,cursor){
    var origin=workingReview.scope,q=String(query || "").trim()
    if(!origin.requestId||workingReview.sourceKind!=="branch-range")return
    if(!kind){dismissReviewRefs();reviewRefs=Object.assign({},reviewRefs,{query:q});requestReviewRefs(q,"local",null);requestReviewRefs(q,"remote",null);return}
    if(["local","remote"].indexOf(kind)<0||q!==reviewRefs.query)return
    var prior=reviewRefs[kind],more=cursor!==null&&cursor!==undefined
    if(prior.busy||more&&cursor!==prior.nextCursor)return
    var scope=Object.assign({},origin,{requestId:uuid()}),all=Object.assign({},reviewRefs)
    all[kind]={busy:worker.running,error:worker.running?"":"disconnected",refs:more?prior.refs:[],scope:scope,nextCursor:more?prior.nextCursor:null,lastCursor:more?cursor:null};reviewRefs=all
    if(worker.running)send(Object.assign({op:"review-refs",refKind:kind,query:q,cursor:more?cursor:null},scope))
  }
  property var workingReview:({loading:false,error:"",data:null,scope:{},sourceKind:"working-tree",baseRef:"",ignoreWhitespace:true,filePath:""})
  function dismissWorkingReview(){dismissReviewRefs();workingReview={loading:false,error:"",data:null,scope:{},sourceKind:"working-tree",baseRef:"",ignoreWhitespace:true,filePath:""}}
  function requestWorkingReview(kind,base,whitespace,path){
    if(!canOpenGit)return
    var v=workingReview,k=kind || v.sourceKind || "working-tree",b=base===undefined?v.baseRef:base,ws=whitespace===undefined?(v.scope.requestId?v.ignoreWhitespace:diffPreferences.ignoreWhitespace):whitespace,p=path || ""
    if(v.loading&&v.filePath===p&&v.sourceKind===k&&v.baseRef===b&&v.ignoreWhitespace===ws)return
    if(k!=="branch-range")dismissReviewRefs()
    var same=v.sourceKind===k&&v.baseRef===b&&v.ignoreWhitespace===ws
    var scope={environment:selectedEnvironment,thread:selectedThread,projectId:selectedInfo.projectId,cwd:selectedInfo.worktreePath || gitProject.workspaceRoot,requestId:uuid()}
    workingReview={loading:worker.running,error:worker.running?"":"disconnected",data:same?v.data:null,scope:scope,sourceKind:k,baseRef:b,ignoreWhitespace:ws,filePath:p}
    if(worker.running)send(Object.assign({op:"working-review",sourceKind:k,baseRef:b,ignoreWhitespace:ws,filePath:p},scope))
  }
  property var gitView:({loading:false,error:"",data:null,scope:{},operation:{}})
  property var gitCommands:({})
  property var scriptShortcutPending:null
  readonly property bool canOpenGit:!!selectedInfo.projectId&&!!(selectedInfo.worktreePath || gitProject.workspaceRoot)
  readonly property var gitProject:(state.projects || []).find(function(p){return p.id===root.selectedInfo.projectId&&p.environmentId===root.selectedEnvironment}) || {}
  readonly property bool gitBusy:Object.keys(gitCommands).some(function(id){var c=root.gitCommands[id];return c.op==="git-action"&&c.environment===root.selectedEnvironment&&c.scope.cwd===(root.gitView.scope || {}).cwd})
  function dismissGit(){scriptShortcutPending=null;gitView={loading:false,error:"",data:null,scope:{},operation:{}}}
  function projectScriptId(command){return NativeKeys.nativeKeys.projectScriptIdFromCommand(command)}
  function requestScriptShortcut(command){
    var scriptId=projectScriptId(command)
    if(!scriptId||!canOpenGit||gitBusy||scriptShortcutPending||["pending","unknown"].indexOf(gitView.operation.state)>=0)return false
    requestGit()
    if(!gitView.loading)return false
    scriptShortcutPending={key:selectedKey,scriptId:scriptId,requestId:gitView.scope.requestId};return true
  }
  function finishScriptShortcut(){
    var pending=scriptShortcutPending,v=gitView
    if(!pending||v.loading||pending.requestId!==v.scope.requestId)return
    scriptShortcutPending=null
    if(pending.key!==selectedKey||v.error||!v.data)return
    if(!v.data.permissions||!v.data.permissions.terminalWrite){gitView=Object.assign({},v,{operationError:"permission_denied"});return}
    if(!(v.data.scripts || []).some(function(s){return s.id===pending.scriptId})){gitView=Object.assign({},v,{operationError:"git_script_unavailable"});return}
    requestGitAction({scope:v.scope,revision:v.data.revision,operation:"script",scriptId:pending.scriptId,confirmed:true})
  }
  function requestGit(){
    scriptShortcutPending=null
    if(!canOpenGit)return
    if(!worker.running){gitView=Object.assign({},gitView,{loading:false,error:"disconnected",data:null});return}
    var scope={environment:selectedEnvironment,thread:selectedThread,projectId:selectedInfo.projectId,cwd:selectedInfo.worktreePath || gitProject.workspaceRoot,requestId:uuid()}
    gitView={loading:true,error:"",data:null,scope:scope,operation:{}}
    send(Object.assign({op:"git-read"},scope))
  }
  function matchesGitScope(a,b){return ["environment","thread","projectId","cwd","requestId"].every(function(k){return a&&b&&a[k]===b[k]})}
  function requestGitAction(intent){
    var v=gitView
    if(!worker.running||v.loading||v.error||!v.data||gitBusy||["pending","unknown"].indexOf(v.operation.state)>=0||!matchesGitScope(intent.scope,v.scope)||intent.revision!==v.data.revision||selectedKey!==v.scope.environment+"/"+v.scope.thread)return
    var id=uuid(),scope=JSON.parse(JSON.stringify(v.scope)),captured=Object.assign({},intent,{actionId:id})
    var command={op:"git-action",environment:scope.environment,thread:scope.thread,requestId:id,scope:scope,intent:captured}
    var all=Object.assign({},gitCommands);all[id]=command;gitCommands=all
    gitView=Object.assign({},v,{operation:{state:"pending",actionId:id,operation:intent.operation,scope:scope},operationError:""});send(command)
  }
  function pollGitOperation(){var s=gitView.scope;if(!s.requestId||gitView.loading)return;send({op:"git-state",environment:s.environment,thread:s.thread,scope:s,requestId:s.requestId})}
  function reconcileGitOperation(){
    var v=gitView,op=v.operation || {};if(op.state!=="unknown"||!op.actionId||gitBusy)return
    var id=uuid(),command={op:"git-resolve",environment:v.scope.environment,thread:v.scope.thread,scope:v.scope,requestId:id,actionId:op.actionId,confirmed:true}
    var all=Object.assign({},gitCommands);all[id]=command;gitCommands=all;send(command)
  }
  function openGitTerminal(){var v=gitView,op=v.operation || {};if(!op.terminalId)return;var id=uuid(),command={op:"git-terminal",environment:v.scope.environment,thread:v.scope.thread,scope:v.scope,requestId:id,actionId:op.actionId};var all=Object.assign({},gitCommands);all[id]=command;gitCommands=all;send(command)}
  Timer{interval:1500;repeat:true;running:root.panelVisible&&!!root.gitView.scope.requestId&&root.gitView.operation.state==="pending";onTriggered:root.pollGitOperation()}
  property var diffView: ({loading:false,data:null,error:"",requestId:"",summary:{},ignoreWhitespace:true})
  function dismissDiff(){diffView={loading:false,data:null,error:"",requestId:"",summary:{},ignoreWhitespace:true}}
  function requestDiff(summary,ignoreWhitespace){
    var id=uuid(),ws=ignoreWhitespace===undefined?diffPreferences.ignoreWhitespace:!!ignoreWhitespace
    var captured=JSON.parse(JSON.stringify(summary || {}))
    diffView={loading:true,data:null,error:"",requestId:id,environment:selectedEnvironment,thread:selectedThread,fromTurnCount:captured.fromTurnCount,toTurnCount:captured.toTurnCount,summary:captured,ignoreWhitespace:ws}
    send({op:"diff",environment:selectedEnvironment,thread:selectedThread,requestId:id,fromTurnCount:captured.fromTurnCount,toTurnCount:captured.toTurnCount,summary:captured,ignoreWhitespace:ws})
  }
  function action(data) {
    if(data.op==="browser-import-sources"||data.op==="browser-import-read"||data.op==="browser-import-action"){ImportFlow.receipt(root,data);return}
    if(data.op==="preview-action"){Previews.receipt(root,data);return}
    if(data.op==="browser-profiles-read"||data.op==="browser-profiles-action"){Profiles.receipt(root,data);return}
    if(data.op==="devices-read"||data.op==="devices-action"){Devices.receipt(root,data);return}
    if(data.op==="licenses-read"){
      var v=licenseView;if(!v.busy||v.requestId!==data.requestId)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||!result.manifestId||!Array.isArray(result.entries)))error="invalid_license_manifest"
      if(!error&&(!v.data||v.data.manifestId!==result.manifestId))dismissLicenseNotice()
      licenseView=Object.assign({},v,{busy:false,error:error,data:error?null:result});return
    }
    if(data.op==="licenses-notice"){
      var v=licenseNotice;if(!v.busy||v.requestId!==data.requestId)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||result.manifestId!==v.manifestId||result.entryKey!==v.entryKey||typeof result.noticeText!=="string"))error="license_manifest_changed"
      licenseNotice=Object.assign({},v,{busy:false,error:error,noticeText:error?"":result.noticeText});return
    }
    if(data.op==="rewind-preview"){
      var p=rewindPreview;if(!p.busy||data.requestId!==p.requestId||data.environment+"/"+data.thread!==p.key||selectedKey!==p.key)return
      rewindPreview=Object.assign({},p,{busy:false,error:data.error || ""})
      if(!data.error&&data.result){var operation=data.result.operation;if(operation&&(["prepared","accepted","uncertain","pending","superseded"].indexOf(operation.state)>=0||operation.state==="complete"&&!operation.recovered)){rememberRewind(operation);showPendingRewind()}else if(data.result.reference)rewindReady(data.result.reference,data.result.canRestoreFiles===true)}return
    }
    if(data.op==="rewind-list"){if(!data.error&&data.result)(data.result.operations || []).forEach(function(r){root.rememberRewind(r);if(r.state!=="complete")root.refreshRewind(r)});return}
    if(["rewind-dispatch","rewind-observe","rewind-recover","rewind-resolve"].indexOf(data.op)>=0){
      var c=rewindRequests[data.requestId];if(!c||c.op!==data.op||c.environment!==data.environment||c.thread!==data.thread)return
      var requests=Object.assign({},rewindRequests);delete requests[data.requestId];rewindRequests=requests
      var key=c.environment+"/"+c.thread,r=rewinds[key] || c
      if(data.op==="rewind-recover"){
        var all=Object.assign({},rewindRecoveries);delete all[key];rewindRecoveries=all
        if(data.error){if(drafts[key])patchDraft(key,{busy:false,error:data.error});var states=Object.assign({},rewinds);states[key]=Object.assign({},r,{recoveryError:data.error});rewinds=states;rewindReceipt(Object.assign({},r,{state:"recovery_failed",error:data.error}));return}
        var saved=data.result || {};if(saved.key!==key||!saved.draft)return
        patchDraft(key,Object.assign({},saved.draft,{busy:false,loading:false,error:"",saveState:"saved",saveError:""}),false);resetUndo(key);rememberRewind(saved.operation)
        if(selectedKey===key){resetHistory();fetchMessages();rewindRestored(key)}return
      }
      if(data.error){
        if(data.op==="rewind-dispatch"){
          var uncertain=r.durable===true||["local_error","disconnected","rpc_outcome_unknown"].indexOf(data.error)>=0
          rememberRewind(Object.assign({},r,{state:uncertain?"uncertain":"rejected",error:data.error}));if(uncertain)refreshRewind(r)
        }else if(data.op==="rewind-observe"&&data.error==="revert_scope_mismatch"){refreshRewind({environment:c.environment,thread:c.thread})}
        else if(data.op==="rewind-observe"&&data.error!=="revert_already_pending"&&data.error!=="revert_busy")rememberRewind(Object.assign({},r,{state:"uncertain",error:data.error}))
        else if(data.op==="rewind-resolve")rememberRewind(Object.assign({},r,{error:data.error}))
        return
      }
      if(data.result&&data.result.state==="idle"){if(r.state==="uncertain")rememberRewind(Object.assign({},r,{state:"rejected",error:"revert_request_unavailable"}));return}
      if(data.result){rememberRewind(data.result);if(data.op==="rewind-dispatch"&&data.result.state==="accepted")refreshRewind(data.result)}return
    }
    if(data.op==="keybindings-open"){if(!keybindingOpen.busy||keybindingOpen.requestId!==data.requestId||keybindingOpen.environment!==data.environment)return;keybindingOpen=Object.assign({},keybindingOpen,{busy:false,error:data.error || (!data.result||data.result.environment!==data.environment||data.result.opened!==true?"editor_open_failed":"")});return}
    if(data.op==="keybindings-read"){
      var c=keybindingConfigs[data.environment];if(!c||!c.busy||c.requestId!==data.requestId)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||result.environment!==data.environment||!result.viewId||!result.originKey||!Array.isArray(result.keybindings)))error="keybindings_scope_changed"
      var configs=Object.assign({},keybindingConfigs);configs[data.environment]={busy:false,error:error,requestId:c.requestId,data:error?c.data:result};keybindingConfigs=configs;return
    }
    if(data.op==="keybindings-write"){
      var command=keybindingCommands[data.requestId];if(!command||command.environment!==data.environment)return
      var commands=Object.assign({},keybindingCommands);delete commands[data.requestId];keybindingCommands=commands
      var result=data.result,error=data.error || "",valid=result&&result.state==="complete"&&result.environment===command.environment&&result.data&&result.data.environment===command.environment&&result.data.originKey===command.originKey&&result.data.viewId&&Array.isArray(result.data.keybindings)
      if(!valid)error=error || result&&result.error || "keybindings_outcome_unknown"
      var c=keybindingConfigs[command.environment],configs=Object.assign({},keybindingConfigs)
      if(c&&c.data&&c.data.viewId===command.viewId){configs[command.environment]=Object.assign({},c,{busy:false,error:error,data:valid?result.data:c.data});keybindingConfigs=configs}
      if(keybindingWrite.group===command.group){var results=Object.assign({},keybindingWrite.results);results[command.environment]={state:valid?"complete":data.error?"failed":"uncertain",error:error};keybindingWrite=Object.assign({},keybindingWrite,{results:results,busy:Object.keys(results).some(function(id){return results[id].state==="pending"})})}return
    }
    if(data.op==="archive-read"){
      var v=archiveView,session=v.sessions[data.environment];if(!session||!session.busy||session.requestId!==data.requestId)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||result.environment!==data.environment||result.requestId!==data.requestId||JSON.stringify(result.projectIds)!==JSON.stringify(session.scope.projectIds)))error="archive_scope_changed"
      var sessions=Object.assign({},v.sessions);sessions[data.environment]=Object.assign({},session,{busy:false,error:error,data:error?session.data:result});archiveView=Object.assign({},v,{sessions:sessions})
      if(!error){var actions=Object.assign({},archiveActions);(result.operations || []).forEach(function(r){var key=root.archiveKey(r),prior=actions[key];if(prior&&root.archiveBlocking(prior)&&prior.actionId!==r.actionId)return;actions[key]=Object.assign({},r,{receivedAt:Date.now()})});archiveActions=actions}return
    }
    if(data.op==="archive-action"||data.op==="archive-state"){
      var command=archiveCommands[data.requestId];if(!command||command.environment!==data.environment||command.thread!==data.thread)return
      var commands=Object.assign({},archiveCommands);delete commands[data.requestId];archiveCommands=commands
      var key=Object.keys(archiveActions).find(function(k){return root.archiveActions[k].actionId===command.actionId}),prior=key&&archiveActions[key]
      if(!prior)return
      var result=data.result,error=data.error || ""
      if(result&&(result.actionId!==command.actionId||result.environment!==command.environment||result.threadId!==command.thread||result.projectId!==prior.projectId||result.originKey!==prior.originKey)){result=null;error="archive_scope_changed"}
      var actions=Object.assign({},archiveActions);actions[key]=Object.assign({},prior,result || {state:error==="local_error"||error==="archive_scope_changed"||data.op==="archive-state"?"uncertain":"failed",error:error},{receivedAt:Date.now()});archiveActions=actions
      if(archiveView.targets.length)requestArchive(archiveView.targets,true)
      if(result&&result.state==="complete")refresh()
      return
    }
    if(data.op==="review-refs"){
      var kind=["local","remote"].find(function(k){return root.reviewRefs[k].scope.requestId===data.requestId}),v=kind&&reviewRefs[kind]
      if(!v||!v.busy||v.scope.environment!==data.environment||v.scope.thread!==data.thread||!workingReview.scope.requestId||workingReview.sourceKind!=="branch-range")return
      if(!["environment","thread","projectId","cwd"].every(function(k){return v.scope[k]===root.workingReview.scope[k]}))return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||!matchesGitScope(v.scope,result.scope)||result.refKind!==kind||result.query!==reviewRefs.query))error="git_scope_changed"
      var rows=v.refs || [],next=v.nextCursor
      if(!error){var merged={};rows.concat(result.refs || []).forEach(function(r){merged[r.name]=r});rows=Object.keys(merged).map(function(k){return merged[k]});if(rows.length>4096)error="diff_limit_files";else next=result.nextCursor}
      var all=Object.assign({},reviewRefs);all[kind]=Object.assign({},v,{busy:false,error:error,refs:error?v.refs:rows,nextCursor:next});reviewRefs=all;return
    }
    if(data.op==="working-review"){
      var v=workingReview,s=v.scope
      if(!v.loading||!s.requestId||data.requestId!==s.requestId||data.environment!==s.environment||data.thread!==s.thread)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||!matchesGitScope(s,result.scope)))error="git_scope_changed"
      if(!error){
        var old=v.data,unchanged=old&&old.source&&result.source&&old.source.diffHash===result.source.diffHash
        result=Object.assign({},result,{files:(result.files || []).map(function(f){
          if(result.file&&result.file.path===f.path)return Object.assign({},f,result.file,{pending:false})
          var prior=unchanged&&(old.files || []).find(function(a){return a.path===f.path&&!a.pending})
          return prior || f
        })})
      }
      workingReview=Object.assign({},v,{loading:false,error:error,data:error?v.data:result});return
    }
    if(data.op==="git-read"){
      var v=gitView,s=v.scope
      if(!v.loading||!s.requestId||data.requestId!==s.requestId||data.environment!==s.environment||data.thread!==s.thread)return
      var result=data.result,error=data.error || ""
      if(!error&&(!result||!matchesGitScope(s,result.scope)))error="git_scope_changed"
      gitView=Object.assign({},v,{loading:false,error:error,data:error?null:result,operation:error?{}:result.operation || {}});finishScriptShortcut();return
    }
    if(data.op==="git-state"){
      var v=gitView,s=v.scope;if(data.requestId!==s.requestId||data.environment!==s.environment||data.thread!==s.thread)return
      if(!data.error&&data.result){if(v.operation.state==="pending"&&v.operation.actionId&&data.result.actionId!==v.operation.actionId)return;gitView=Object.assign({},v,{operation:data.result});if(v.operation.state==="pending"&&["succeeded","submitted","failed"].indexOf(data.result.state)>=0)requestGit()}return
    }
    if(["git-action","git-resolve","git-terminal"].indexOf(data.op)>=0){
      var command=gitCommands[data.requestId];if(!command||command.environment!==data.environment||command.thread!==data.thread)return
      var all=Object.assign({},gitCommands);delete all[data.requestId];gitCommands=all
      if(matchesGitScope(command.scope,gitView.scope)){
        gitView=Object.assign({},gitView,{operation:data.error&&data.op==="git-action"?{}:data.result&&data.op!=="git-terminal"?data.result:gitView.operation,operationError:data.error || ""})
        if(!data.error&&data.result&&["succeeded","submitted","failed","reviewed"].indexOf(data.result.state)>=0)requestGit()
        else if(data.error)pollGitOperation()
      }return
    }
    if(data.op==="client-settings"){
      if(!clientSettings.busy||data.requestId!==clientSettings.requestId)return
      clientSettings=Object.assign({},clientSettings,{busy:false,error:data.error || "",data:data.result || clientSettings.data})
      var update=diffPreferenceUpdate
      if(update.stage){
        if(data.error)diffPreferenceUpdate=Object.assign({},update,{stage:"",error:data.error})
        else if(update.stage==="read"){diffPreferenceUpdate=Object.assign({},update,{stage:"write"});requestClientSettings({diffLayout:update.desired})}
        else diffPreferenceUpdate={desired:"",stage:"",error:""}
      }return
    }
    if(data.op==="plan-followup"){
      var op=planOperation;if(!op.busy||data.requestId!==op.requestId||op.command)return
      if(data.environment+"/"+data.thread!==op.key||selectedKey!==op.key||draft.text!==op.draftText||draft.attachments.length||draft.attempt){planOperation={busy:false,error:"plan_scope_changed",command:null};return}
      if(data.error){planOperation=Object.assign({},op,{busy:false,error:data.error});return}
      var prepared=data.result || {}
      if(prepared.environment!==selectedEnvironment||prepared.thread!==selectedThread||!prepared.text){planOperation=Object.assign({},op,{busy:false,error:"invalid_plan_scope"});return}
      if(op.newThread){
        var command={op:"manage",action:"new-thread",environment:selectedEnvironment,thread:selectedThread,requestId:uuid(),sendRequestId:uuid(),text:prepared.text,attachments:[],modelSelection:prepared.modelSelection,interactionMode:"default",sourcePlanRef:prepared.sourcePlanRef,planFollowUp:true,planOriginKey:selectedKey,navigateAfterSend:true,projectId:prepared.projectId,branch:prepared.branch,worktreePath:prepared.worktreePath,title:prepared.title,runtimeMode:"full-access",delivery:"auto"}
        planOperation=Object.assign({},op,{busy:true,command:command});patchDraft(selectedKey,{attempt:command,busy:true,error:""});flushDraft(selectedKey,command)
      }else{
        planOperation=Object.assign({},op,{busy:false,error:""});patchDraft(selectedKey,{text:prepared.text,interactionMode:prepared.interactionMode,sourcePlanRef:prepared.sourcePlanRef || null,error:""});submit()
      }
      return
    }
    if(data.op==="terminal-open"){
      if(data.requestId!==terminalOperation.requestId)return
      var here=terminalOperation.key===selectedKey
      terminalOperation=Object.assign({},terminalOperation,{busy:false,error:data.error || ""})
      if(!data.error&&here)terminalOpened()
      return
    }
    if(data.op==="connection-route"){
      if(data.requestId!==workspaceOperation.requestId||workspaceOperation.action!=="routing")return
      var request=workspaceOperation.request
      workspaceOperation={busy:false,error:data.error || ""}
      if(data.error){managementError=data.error;return}
      request=Object.assign({},request,{command:Object.assign({},request.command,data.result,{automatic:false,branch:null,worktreePath:null})})
      beginNewThread(request);return
    }
    if(data.op==="settings"&&data.requestId===workspaceOperation.requestId){
      var op=workspaceOperation,request=op.request
      workspaceOperation=Object.assign({},op,{busy:false,requestId:"",error:data.error || ""})
      if(request.sourceKey&&request.sourceKey!==selectedKey)return
      if(data.error){newThreadWorkspaceRequested(request);return}
      request=Object.assign({},request,{startFromOrigin:!!data.result.startFromOrigin})
      if(request.command.useProjectDefaults===true&&!request.sourceKey){
        var defaults=Object.assign({},request.command)
        if(data.result.defaultModelSelection)defaults.modelSelection=data.result.defaultModelSelection
        if(data.result.defaultRuntimeMode)defaults.runtimeMode=data.result.defaultRuntimeMode
        request=Object.assign({},request,{command:defaults})
      }
      if(op.action==="write")confirmNewThread(request,data.result.mode==="worktree",true)
      else if(data.result.mode)confirmNewThread(request,data.result.mode==="worktree",true)
      else newThreadWorkspaceRequested(request)
      return
    }
    if(data.op==="connection-network"){
      if(data.requestId!==connectionNetwork.requestId)return
      var action=connectionNetwork.action
      connectionNetwork=Object.assign({},connectionNetwork,{busy:false,error:data.error || "",data:data.result || connectionNetwork.data})
      if(action==="apply"){refreshBackendStatus();recoverLocalBackend();refresh()}
      connectionNetworkFinished(action,data.error || "");return
    }
    if(data.op==="connection-access"){
      if(data.requestId!==connectionAccess.requestId)return
      var action=connectionAccess.action,next=Object.assign({},connectionAccess,{busy:false,error:data.error || ""})
      if(!data.error){if(action==="read")next.data=data.result;else if(action==="create-link")next.created=data.result.mutation}
      connectionAccess=next
      connectionAccessFinished(action,data.error || "")
      return
    }
    if(data.op==="connections-manage"){
      if(data.requestId===connectionPreferences.requestId){
        connectionPreferences=Object.assign({},connectionPreferences,{busy:false,error:data.error || ""},data.result || {});return
      }
      if(data.requestId!==connectionSettings.requestId)return
      var command=connectionSettingsCommand
      connectionSettings=Object.assign({},connectionSettings,{busy:false,error:data.error || ""},data.result || {})
      if(!data.error&&command&&command.action!=="read"){
        if((command.environmentId===selectedEnvironment&&(command.action==="remove"||(command.action==="remove-route"&&!(data.result.environments || []).some(function(e){return e.id===root.selectedEnvironment}))||(command.action==="enabled"&&!command.enabled)))||(command.action==="local-enabled"&&!command.enabled&&selectedEnvironment==="local")){connectionChanging=true;clearSelection();connectionChanging=false}
        refresh()
      }
      connectionsManaged(command?command.action:"",data.error || "");if(!data.error&&connectionPreferences.requestId)manageConnectionPreferences("read",{});return
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
    if(data.op==="settings"){if(settingsResult.busy&&data.requestId===settingsResult.requestId)settingsResult=Object.assign({},settingsResult,{busy:false,error:data.error || ""},data.result || {});return}
    if(data.op==="provider-instances"){
      if(providerInstances.busy&&data.requestId===providerInstances.requestId&&data.environment===providerInstances.environment){providerInstances=Object.assign({},providerInstances,{busy:false,error:data.error || ""},data.result || {});if(!data.error&&providerInstances.action!=="read")refreshCatalogIn(data.environment)}
      return
    }
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
    if(data.op==="image"){var key=Object.keys(imageStates).find(function(k){return root.imageStates[k].requestId===data.requestId});if(!key)return;var all=Object.assign({},imageStates);all[key]={loading:false,url:data.result?data.result.url || "":"",width:data.result?data.result.width || 0:0,height:data.result?data.result.height || 0:0,error:data.error || "",requestId:data.requestId};imageStates=all;return}

    if(data.op==="tool-inspector"){toolInspectorFinished(data.requestId,data.result || null,data.error || "");return}
    if(data.op==="diff"){
      var v=diffView
      if(!v.loading||!v.requestId||data.requestId!==v.requestId||data.environment!==v.environment||data.thread!==v.thread||selectedEnvironment!==v.environment||selectedThread!==v.thread)return
      var result=data.result,err=data.error || ""
      if(!err&&(!result||result.fromTurnCount!==v.fromTurnCount||result.toTurnCount!==v.toTurnCount))err="invalid_diff_range"
      diffView=Object.assign({},v,{loading:false,data:err?null:result,error:err});return
    }
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
      var parked=settleNavigation&&settleNavigation.requestId===data.requestId?settleNavigation:null
      var sourceKey=creation.environment+"/"+creation.thread
      var source=drafts[sourceKey]
      var moving=source&&source.attempt&&source.attempt.op==="manage"&&source.attempt.requestId===creation.requestId
      managementBusy = false; managementError = data.error || ""
      var localRejection=["invalid_workspace","invalid_project","invalid_request_id","invalid_title","invalid_message","invalid_attachments","image_limit_10MiB","image_total_limit_80MiB"].indexOf(data.error)>=0
      var retryable=data.error==="rpc_outcome_unknown" || (!!creation.workspaceStrategy&&!!data.error&&!localRejection)
      if(creation.planFollowUp===true&&data.error)planOperation=Object.assign({},planOperation,{busy:false,error:data.error,command:creation})
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
          patchDraft(destination,{text:creation.planFollowUp?attempt.text:source.text,attachments:creation.planFollowUp?[]:source.attachments,modelSelection:attempt.modelSelection,interactionMode:attempt.interactionMode,sourcePlanRef:attempt.sourcePlanRef || null,delivery:"auto",attempt:attempt,busy:true,loading:false,error:""})
          if(creation.planFollowUp===true)planOperation=Object.assign({},planOperation,{busy:true,error:"",command:attempt})
          // Save the destination before clearing the source or sending anything.
          flushDraft(destination,attempt)
        }
      }
      if (!retryable) {managementRequest = null;settleNavigation=null}
      if (!data.error) {
        if(data.result.action==="project-remove" && selectedInfo.projectKey===data.environment+"/"+data.result.projectId){var next=state.threads.find(function(t){return t.projectKey!==root.selectedInfo.projectKey});if(next)selectThread(next.environmentId,next.id);else clearSelection()}
        if(["archive","delete-thread"].indexOf(data.result.action)>=0 && selectedKey===data.environment+"/"+data.result.threadId){var nextThread=state.threads.find(function(t){return t.key!==root.selectedKey});if(nextThread)selectThread(nextThread.environmentId,nextThread.id);else clearSelection()}
        if(data.result.action==="settle")finishSettleNavigation(parked)
        refresh(); if (data.result.action === "new-thread"&&creation.planFollowUp!==true&&(!moving||selectedKey===sourceKey)) selectThread(data.environment,data.result.threadId)
      }
      managementFinished(data)
      if(pendingProjectOpen&&pendingProjectOpen.requestId===data.requestId){var opened=pendingProjectOpen;if(data.error!=="rpc_outcome_unknown")pendingProjectOpen=null;projectOpened(opened.environment,opened.projectId,data.error || "")}
      if(!data.error&&["add-project","create-project","clone-project"].indexOf(data.result.action)>=0){var environment=data.environment,projectId=data.result.projectId;Qt.callLater(function(){root.openProject(environment,projectId,true)})}
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
      var sentAttempt=d.attempt
      if(planOperation.command&&planOperation.command.requestId===data.requestId)planOperation=Object.assign({},planOperation,{busy:false,error:data.error || "",command:data.error?sentAttempt:null})
      if(data.error && data.error!=="rpc_outcome_unknown")forgetQueued(key,"omarchy-"+data.requestId)
      if (data.error) patchDraft(key, {busy: false, error: data.error, attempt: data.error === "rpc_outcome_unknown" ? d.attempt : null})
      else {
        var acceptedInteraction=d.attempt.interactionMode
        if(acceptedInteraction){snapshotAboutToChange();state=Object.assign({},state,{threads:state.threads.map(function(t){return t.key===key?Object.assign({},t,{interactionMode:acceptedInteraction}):t})});snapshotUpdated()}
        var acceptedMode=d.attempt.runtimeMode
        if(acceptedMode){snapshotAboutToChange();state=Object.assign({},state,{threads:state.threads.map(function(t){return t.key===key?Object.assign({},t,{runtimeMode:acceptedMode}):t})});snapshotUpdated()}
        patchDraft(key, {text: "", attachments: [], busy: false, attempt: null, runtimeMode:null,interactionMode:null,sourcePlanRef:null,error: ""},false); resetUndo(key);flushDraft(key); refresh()
        if(sentAttempt.navigateAfterSend&&selectedKey===sentAttempt.planOriginKey)selectThread(data.environment,data.thread)
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
    send({op: "visible", value: panelVisible});syncPreview(false);Devices.visibility(root);syncKeybindingWatches(true);if(panelVisible){if(browserProfileView.environments.length)requestBrowserProfiles(browserProfileView.environments,true);probeScroll();ensureSidebarCatalogs();ensureRuntimeKeybindings(true)}
    if (!panelVisible) { dismissDiff();clearInlineDocuments();liveConnected=false;documentOpen=({busy:false,error:"",requestId:""});imageStates=({});requestedSerial++; activity = []; loadingMessages = false; persistViewNow(); flushDrafts() }
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
  function clearSelection(){clearInlineDocuments();closeDocument();documentOpen=({busy:false,error:"",requestId:""});dismissWorkingReview();dismissGit();rewindPreview={busy:false,error:"",requestId:""};threadWillChange();resetHistory();requestedSerial++;send({op:"clear-select"});selectedEnvironment="";selectedThread="";selectedKey="";selectedTitle="";selectedProject="";planView=({plans:[],ready:null});planOperation=({busy:false,error:"",command:null});transcript=[];activity=[];questions=[];approvals=[];tasks=null;queue=({items:[]});loadingMessages=false;messageError="";liveConnected=false;persistView.restart()}
  function selectThread(env, thread) {
    dismissWorkingReview();dismissGit();rewindPreview={busy:false,error:"",requestId:""}
    if (selectedEnvironment === env && selectedThread === thread) { fetchMessages(); return }
    clearInlineDocuments();closeDocument();favoriteError="";liveConnected=false;documentOpen=({busy:false,error:"",requestId:""});imageStates=({});dismissDiff();dismissSuggestions();threadWillChange()
    selectedEnvironment = env; selectedThread = thread; selectedKey = env + "/" + thread
    var found = state.threads.find(function(t) { return t.key === root.selectedKey })
    selectedTitle = found ? (found.title || thread) : thread
    selectedProject = found ? (found.project || env) : env
    if (!drafts[selectedKey]) {
      patchDraft(selectedKey, {loading:true,modelSelection: found ? found.modelSelection : null})
      send({op:"draft-load",key:selectedKey,projectId:draftProject(selectedKey)})
    }
    if (!catalogs[env]) send({op: "catalog", environment: env})
    resetHistory();planView=({plans:[],ready:null});planOperation=({busy:false,error:"",command:null});transcript = []; activity = [];tasks=null;questions=[];approvals=[];queue=({items:[],held:false,canReorder:false,canSteer:false}); messageError = ""; fetchMessages()
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
      if(event.type==="browser-surface"){BrowserSurface.receive(root,data)}
      else if(event.type==="preview-state"){Previews.receive(root,data)}
      else if(event.type==="devices-state"){Devices.receiveState(root,data)}
      else if(event.type==="keybindings-changed"){invalidateKeybindings(data)}
      else if(event.type==="connection-cloud"){
        if(data.requestId!==connectionCloud.requestId)return
        var next=Object.assign({},connectionCloud,{busy:!!data.busy,stage:data.stage || "",error:data.error || ""})
        if(data.stage==="authorization")next.device=data.data
        if(data.stage==="warning")next.warning=(data.data || {}).code || ""
        if(data.stage==="installing")next.step=(data.data || {}).step || ""
        if(!data.busy){next.device=null;next.status=data.status || null;next.statusError=data.statusError || ""}
        connectionCloud=next
        if(!data.busy){connectionCloudFinished(data.action,data.error || "");if(data.action!=="read")refresh()}
      }
      else if(event.type==="prompt-stash"){
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
        if(data.error==="draft_recovery_changed"&&data.recovery){
          var pendingRecovery=Object.assign({},pendingSends);delete pendingRecovery[data.revision];pendingSends=pendingRecovery
          if(existing&&existing.attempt&&submission&&existing.attempt.requestId===submission.requestId)patchDraft(data.key,{attempt:null,busy:false})
          recoverRewind(data.recovery);return
        }
        if(submission) {
          var pending=Object.assign({},pendingSends);delete pending[data.revision];pendingSends=pending
          if(submission.op==="rewind-dispatch"){
            if(data.error){var key=submission.environment+"/"+submission.thread;var r=rewinds[key] || submission;rememberRewind(Object.assign({},r,{state:"rejected",error:"draft_save_failed"}));var requests=Object.assign({},rewindRequests);delete requests[submission.requestId];rewindRequests=requests}
            else send(submission)
          }
          else if(submission.op==="approval"){

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
            if(planOperation.command&&planOperation.command.requestId===submission.requestId)planOperation=Object.assign({},planOperation,{busy:false,error:"draft_save_failed"})
            patchDraft(data.key,{busy:false,error:"draft_save_failed"})
            if(submission.moveSource)patchDraft(submission.moveSource,{busy:false,error:"draft_save_failed"})
          }
          else if (existing && existing.busy && existing.attempt && existing.attempt.requestId === submission.requestId) {
            if(submission.op==="manage")manage(submission)
            else {
              if(submission.moveSource){
                var source=drafts[submission.moveSource]
                if(source&&source.attempt&&source.attempt.requestId===submission.creationRequestId){if(source.attempt.planFollowUp)patchDraft(submission.moveSource,{attempt:null,busy:false,error:""});else{patchDraft(submission.moveSource,{text:"",attachments:[],attempt:null,busy:false,error:""});resetUndo(submission.moveSource)}flushDraft(submission.moveSource)}
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
      else if(event.type==="document-ready" && inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{rendererReady:true,error:data.error || ""})}
      else if(event.type==="document-size" && inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{contentHeight:data.height})}
      else if(event.type==="document-frame" && inlineDocuments[data.requestId]){updateInlineDocument(data.requestId,{frameUrl:data.url || inlineDocuments[data.requestId].frameUrl,frameRevision:data.revision,frameWidth:data.width,frameHeight:data.height,error:data.error || ""})}
      else if(event.type==="document-frame" && data.requestId===documentOpen.requestId){documentOpen=Object.assign({},documentOpen,{frameUrl:data.url || documentOpen.frameUrl,frameRevision:data.revision || documentOpen.frameRevision,frameWidth:data.width || documentOpen.frameWidth,frameHeight:data.height || documentOpen.frameHeight,error:data.error || ""})}
      else if (event.type === "action") action(data)
      else if (event.type === "backend-status") {acceptBackendStatus(data)}
      else if (event.type === "snapshot") { snapshotAboutToChange(); state = data; stale = data.stale; error = "";ensureSidebarCatalogs()
        if(data.clientPreferences&&!clientSettings.busy)clientSettings=Object.assign({},clientSettings,{data:data.clientPreferences,error:""})
        var current = state.threads.find(function(t) { return t.key === root.selectedKey })
        if (current) { selectedTitle = current.title; selectedProject = current.project }
        finishLocalRecovery();snapshotUpdated() }
      else if (event.type === "quotas") { quotaSnapshot=data;presentQuotas();quotaError = "" }
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
        planView=data.result.planView || {plans:[],ready:null}
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
      if(root.gitView.scope.requestId)root.requestGit()
      root.send({op:"rewind-list",requestId:root.uuid()})
      root.requestClientSettings();root.send({op: "visible", value: root.panelVisible}); root.send({op: "search", query: root.query})
      if(root.browserImportWizard.open&&root.browserImportOperation.state!=="uncertain")ImportFlow.refresh(root,false);
      root.syncPreview(true);Devices.visibility(root);if(root.browserProfileView.environments.length)root.requestBrowserProfiles(root.browserProfileView.environments,true);root.syncKeybindingWatches(true);root.ensureRuntimeKeybindings(true)
      if (root.selectedEnvironment) root.send({op: "catalog", environment: root.selectedEnvironment})
      if(root.panelVisible)root.probeScroll()
      if (root.panelVisible && root.selectedThread) root.fetchMessages()
      if(root.projectDrafts.command&&root.projectDrafts.command.action!=="list")root.retryProjectDrafts()
      Object.keys(root.drafts).forEach(function(key){ if(root.projectDraftTransferKey(key))return;if(root.drafts[key].loading)root.send({op:"draft-load",key:key,projectId:root.draftProject(key)});else root.flushDraft(key) })
    }
    stdout: SplitParser { onRead: function(line) { root.receive(line) } }
    onExited: { ImportFlow.lost(root);root.browserCleanups=({}); BrowserSurface.dismiss(root);Previews.lost(root); root.connectionLost(); root.stale = true; root.error = "disconnected"; reconnect.restart() }
  }
  Timer { id: reconnect; interval: 5000; onTriggered: worker.running = true }
}
