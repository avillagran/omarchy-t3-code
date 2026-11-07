#!/usr/bin/env node
// Complete native turn-start and provider-default bodies; only effect boundaries are inert.
import fs from 'node:fs';
import vm from 'node:vm';
import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [source,output]=process.argv.slice(2),revision='4cb6945dc60ed03f567dc5daafd083e52308e82b';
const provenance=JSON.parse(fs.readFileSync(source+'/SOURCE.json','utf8'));
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
if(provenance.revision!==revision)throw Error('Wrong pinned source');
for(const [file,digest] of Object.entries(provenance.files))if(hash(fs.readFileSync(source+'/'+file))!==digest)throw Error('Changed source '+file);
const require=createRequire(source+'/package.json');
const Effect=await import(pathToFileURL(require.resolve('effect/Effect')).href);
if(JSON.parse(fs.readFileSync(require.resolve('effect/package.json'),'utf8')).version!=='4.1.2')throw Error('Wrong Effect');
const model=await import(pathToFileURL(source+'/packages/contracts/src/model.ts').href);
const instances=await import(pathToFileURL(source+'/packages/contracts/src/providerInstance.ts').href);
const settings=await import(pathToFileURL(source+'/packages/contracts/src/settings.ts').href);
const read=file=>fs.readFileSync(source+'/'+file,'utf8');
function between(text,start,end){const a=text.indexOf(start),b=text.indexOf(end,a);if(a<0||b<=a)throw Error('Extraction boundary changed '+start);return text.slice(a,b)}
const erase=text=>stripTypeScriptTypes(text,{mode:'strip'}).replace(/^export /gm,'');
const commands=read('packages/client-runtime/src/operations/commands.ts'),rpc=read('packages/contracts/src/rpc.ts'),orchestration=read('packages/contracts/src/orchestrationV2.ts');
const methods=between(orchestration,'export const ORCHESTRATION_V2_WS_METHODS =','\n} as const;')+'\n} as const;';
const ws=between(rpc,'export const WS_METHODS =','\n} as const;')+'\n} as const;';
const context=vm.createContext({Effect});
vm.runInContext(erase(methods+'\n'+ws),context);
let calls=[];
context.request=(tag,payload)=>Effect.sync(()=>{calls.push({tag,payload:JSON.parse(JSON.stringify(payload))});if(tag==='assets.persistChatAttachments')return {attachments:payload.attachments.map((item,i)=>({type:'file',id:'stored-'+i,name:item.name,mimeType:item.mimeType,sizeBytes:3}))};if(tag==='orchestration.getThreadProjection')return {thread:{projectId:'project',title:'Existing',modelSelection:{instanceId:'codex',model:'existing'},branch:null,worktreePath:null}};return {accepted:true}});
vm.runInContext(erase(between(commands,'const allocateCommandId =','\nconst mutateProject =')+'\n'+between(read('packages/shared/src/composerContextReferences.ts'),'export function remapComposerContextAttachments(','\n}')+'\n}\n'+between(commands,'export const startThreadTurn =','\nexport const interruptThreadTurn =')),context);
const start=vm.runInContext('startThreadTurn',context),starts=[];
const choice={instanceId:'codex',model:'fixture',options:[]};
for(const shape of ['new-root','new-branch','new-existing','new-worktree','existing-worktree','existing'])for(const title of [false,true])for(const override of [false,true])for(const attachment of [false,true]){
 let input={commandId:'omarchy-00000000-0000-4000-8000-000000000001',threadId:'thread',message:{messageId:'message',role:'user',text:'Private message',attachments:attachment?[{type:'file',id:'retained',name:'retained.txt',mimeType:'text/plain',sizeBytes:3}]:[]},runtimeMode:'approval-required',interactionMode:'default',dispatchMode:'auto'};
 if(title)input.titleSeed='Authored title';if(override)input.modelSelection={instanceId:'claudeAgent',model:'other',options:[]};
 if(shape.startsWith('new-'))input.bootstrap={createThread:{projectId:'project',title:'New thread',modelSelection:choice,runtimeMode:'approval-required',interactionMode:'default',branch:shape==='new-branch'?'topic':null,worktreePath:shape==='new-existing'?'/fixture/worktree':null,createdAt:'2026-11-07T12:00:00Z'}};
 if(shape==='new-worktree'||shape==='existing-worktree')input.bootstrap={...input.bootstrap,prepareWorktree:{projectCwd:'/fixture/project',baseBranch:'main',startFromOrigin:true},runSetupScript:true};
 calls=[];await Effect.runPromise(start(input));starts.push({input,calls});
}
const providerText=read('apps/web/src/providerInstances.ts'),displayText=read('packages/client-runtime/src/state/providerInstanceDisplay.ts');
const providerContext=vm.createContext({DEFAULT_MODEL_BY_PROVIDER:model.DEFAULT_MODEL_BY_PROVIDER,PROVIDER_DISPLAY_NAMES:model.PROVIDER_DISPLAY_NAMES,defaultInstanceIdForDriver:instances.defaultInstanceIdForDriver,resolveProjectProviderInstanceEnabled:settings.resolveProjectProviderInstanceEnabled,pipe:(value,...fns)=>fns.reduce((value,fn)=>fn(value),value)});
vm.runInContext(erase(between(displayText,'function humanizeSlug(','\nexport function providerInstanceInitials(')+'\n'+between(displayText,'export function normalizeProviderAccentColor(','\nexport function providerAccentForegroundColor(')+'\n'+providerText.slice(providerText.indexOf('export function isProviderInstancePickerReady('))),providerContext);
const select=vm.runInContext('resolveProjectDefaultProviderModelSelection',providerContext),providerCases=[];
for(const kind of ['codex','claudeAgent','antigravity','future'])for(const shape of ['default-first','custom-first','error-first','unavailable','disabled','custom-model','empty-models','project-disabled','project-enable']){
 const providers=[{instanceId:kind,driver:kind,displayName:'Fixture',enabled:true,installed:true,status:'ready',availability:'available',models:[{slug:'native',isDefault:true,isCustom:false},{slug:'custom',isCustom:true}]},{instanceId:'custom-instance',driver:kind,enabled:true,installed:true,status:'ready',availability:'available',models:[{slug:'other',isDefault:true,isCustom:false}]},{instanceId:'backup',driver:'claudeAgent',enabled:true,installed:true,status:'ready',availability:'available',models:[{slug:'backup-model',isDefault:true,isCustom:false}]}];
 let config={providerInstances:{[kind]:{driver:kind,enabled:true},'custom-instance':{driver:kind,enabled:true},backup:{driver:'claudeAgent',enabled:true}},projectSettingsOverrides:{project:{providerInstances:{}}}};
 if(shape==='custom-first')providers.reverse();if(shape==='error-first')providers[0].status='error';if(shape==='unavailable')providers[0].availability='unavailable';if(shape==='disabled')config.providerInstances[kind].enabled=false;if(shape==='custom-model')providers[0].models=[{slug:'custom-only',isCustom:true}];if(shape==='empty-models')providers[0].models=[];if(shape==='project-disabled')config.projectSettingsOverrides.project.providerInstances[kind]={enabled:false};if(shape==='project-enable'){config.providerInstances[kind].enabled=false;config.projectSettingsOverrides.project.providerInstances[kind]={enabled:true}};
 providerCases.push({providers,settings:config,projectId:'project',selection:select(providers,config,'project')});
}
const Schema=await import(pathToFileURL(require.resolve('effect/Schema')).href),Equal=await import(pathToFileURL(require.resolve('effect/Equal')).href);
if(!Schema.is(instances.ProviderInstanceId)('codex'))throw Error('Duplicate Effect identity');
const sharedModel=await import(pathToFileURL(source+'/packages/shared/src/model.ts').href);
let stickyState={stickyModelSelectionByProvider:{},stickyActiveProvider:null};
const stickyContext=vm.createContext({Equal,ProviderDriverKind:instances.ProviderDriverKind,isProviderDriverKind:Schema.is(instances.ProviderDriverKind),createModelSelection:sharedModel.createModelSelection,normalizeModelSlug:sharedModel.normalizeModelSlug,normalizeCustomModelSlug:sharedModel.normalizeCustomModelSlug,set:change=>{const patch=change(stickyState);stickyState={...stickyState,...patch}}});
const store=read('apps/web/src/composerDraftStore.ts');
vm.runInContext(erase(between(store,'function normalizeProviderDriverKind(','\ntype NormalizedModelSelection')+'\nconst actions={'+between(store,'        setStickyModelSelection: (modelSelection) => {','        applyStickyState:')+'};'),stickyContext);
const sticky=vm.runInContext('actions.setStickyModelSelection',stickyContext),stickyCases=[];
for(const selection of [{instanceId:'codex',model:'first',options:[{id:'reasoningEffort',value:'high'}]},{instanceId:'codex',model:'second'},{instanceId:'claudeAgent',model:'other'},{instanceId:'codex',model:'third',options:[]},{instanceId:'claudeAgent',model:'next',options:[{id:'fastMode',value:true}]},{instanceId:'custom-instance',model:'custom'},{instanceId:'codex',model:'fourth'},{instanceId:'claudeAgent',model:'last'}]){
 sticky(selection);stickyCases.push({selection,state:JSON.parse(JSON.stringify(stickyState))});
}
const projectSettings=await import(pathToFileURL(source+'/packages/shared/src/projectSettings.ts').href),projectFiles=await import(pathToFileURL(source+'/packages/shared/src/t3ProjectFile.ts').href);
const workspaceCases=[];
for(const configured of [null,'local','worktree'])for(const override of ['absent',null,'local','worktree'])for(const contents of [null,'{"defaultThreadEnvMode":"worktree",}','// private fixture\n{"defaultThreadEnvMode":"local"}'])for(const projectHasThreads of [false,true]){
 const raw={providerInstances:{codex:{driver:'codex',enabled:true}},projectSettingsOverrides:{},projectSettingsFolded:true,defaultModelSelection:choice,defaultRuntimeMode:'approval-required',defaultThreadEnvMode:configured,newWorktreesStartFromOrigin:true,worktreeSubmodules:null,textGenerationModelSelection:choice};
 if(override!=='absent')raw.projectSettingsOverrides.project={defaultThreadEnvMode:override};
 const project={id:'project',workspaceRoot:'/workspace/new/child'},file=contents===null?null:projectFiles.parseT3ProjectFile(contents);
 const resolved=projectSettings.resolveProjectSettings(raw,'project',project,file),input={projectSettings:resolved,workspaceRoot:project.workspaceRoot,newProjectsRoot:'/workspace/new',projectHasThreads};
 workspaceCases.push({raw,project,contents,file,input,expected:{defaultModelSelection:resolved.settings.defaultModelSelection,defaultRuntimeMode:resolved.settings.defaultRuntimeMode,mode:resolved.settings.defaultThreadEnvMode,modeSource:resolved.sources.defaultThreadEnvMode,startFromOrigin:resolved.settings.newWorktreesStartFromOrigin},mode:projectSettings.resolveNewThreadEnvMode(input)});
}
fs.writeFileSync(output,JSON.stringify({revision,effectVersion:'4.1.2',sources:provenance.files,constants:{defaultModels:model.DEFAULT_MODEL_BY_PROVIDER,providerDisplayNames:model.PROVIDER_DISPLAY_NAMES},starts,providerCases,stickyCases,workspaceCases},null,2)+'\n');
console.log(JSON.stringify({starts:starts.length,providerCases:providerCases.length,stickyCases:stickyCases.length,workspaceCases:workspaceCases.length,effect:require.resolve('effect/Effect')}));
