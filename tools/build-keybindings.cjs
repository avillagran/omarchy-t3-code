#!/usr/bin/env node
// Build-only TypeScript erasure. No npm dependencies or Node runtime in the plugin.
const vm=require('node:vm');
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),cp=require('node:child_process'),{stripTypeScriptTypes}=require('node:module');
const repo=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(repo,'../t3code-nightly'));
const pinned='5a4e9980533705774dc3f45b35cacf76c3d78dbc';
if(cp.execFileSync('git',['-C',source,'rev-parse','HEAD'],{encoding:'utf8'}).trim()!==pinned)throw Error('Native keybindings source must be pinned to '+pinned);
const files=['packages/contracts/src/keybindings.ts','packages/shared/src/keybindings.ts','apps/web/src/components/usage/usageShortcuts.ts','apps/web/src/components/settings/KeybindingsSettings.logic.ts','apps/web/src/components/settings/KeybindingsSettings.tsx','apps/web/src/keybindings.ts','packages/contracts/src/editor.ts','apps/web/src/projectScripts.ts'];
const inputs=files.map(f=>{const text=fs.readFileSync(path.join(source,f),'utf8');if(cp.execFileSync('git',['-C',source,'show',pinned+':'+f],{encoding:'utf8'})!==text)throw Error('Native input differs from pinned revision: '+f);return text;});
const hashes=Object.fromEntries(files.map((f,i)=>[f,crypto.createHash('sha256').update(inputs[i]).digest('hex')]));
function erase(s){return stripTypeScriptTypes(s,{mode:'strip'}).replace(/^import\s[\s\S]*?;\s*$/gm,'').replace(/^export /gm,'');}
const constants=[...inputs[0].matchAll(/export const MAX_[A-Z_]+ = \d+;/g)].map(m=>m[0]);
const arrays=[...inputs[0].matchAll(/(?:export )?const (?:THREAD_JUMP_KEYBINDING_COMMANDS|MODEL_PICKER_JUMP_KEYBINDING_COMMANDS|THREAD_KEYBINDING_COMMANDS|MODEL_PICKER_KEYBINDING_COMMANDS|STATIC_KEYBINDING_COMMANDS) = \[[\s\S]*?\] as const;/g)].map(m=>m[0]);
if(arrays.length!==5)throw Error('Native command declarations changed');
const usage=inputs[2].slice(0,inputs[2].indexOf('/** Resolves page shortcuts'));
const tree=inputs[4].slice(inputs[4].indexOf('function flattenWhenChildren('),inputs[4].indexOf('/** Warning glyph'));
if(!tree||tree.includes('<Tooltip>'))throw Error('Native condition helper boundaries changed');
const runtime=inputs[5].slice(inputs[5].indexOf('function resolvePlatform('),inputs[5].indexOf('export function formatShortcutKeyLabel('));
const runtimeQt=runtime.replace('  return {\n    terminalFocus: false,','  return Object.assign({\n    terminalFocus: false,').replace('    ...options?.context,\n  };','  }, options?.context);');
const scriptId=inputs[7].slice(inputs[7].indexOf('export function projectScriptIdFromCommand('),inputs[7].indexOf('export function nextProjectScriptId('));
if(!runtime||!scriptId)throw Error('Native runtime helper boundaries changed');
const format=inputs[5].slice(inputs[5].indexOf('export function formatShortcutKeyLabel('),inputs[5].indexOf('export function shortcutLabelForCommand('));
if(!format)throw Error('Native shortcut formatter boundaries changed');
const editors=inputs[6].slice(inputs[6].indexOf('export const EDITORS ='),inputs[6].indexOf('export const EditorId ='));
if(!editors||editors.includes('Schema.'))throw Error('Native editor definition boundaries changed');
const declarations=erase([...constants,...arrays].join('\n'))+'\n'+erase(inputs[1])+'\n'+erase(usage)+'\n'+erase(inputs[3])+'\n'+erase(tree)+'\n'+erase(format)+'\n'+erase(editors)+'\n'+erase(runtimeQt)+'\n'+erase(scriptId);
const exportedNames=[...inputs[1].matchAll(/^export (?:function|const) (\w+)/gm),...inputs[3].matchAll(/^export (?:function|const) (\w+)/gm)].map(m=>m[1]);
const unique=[...new Set(exportedNames)].concat(['STATIC_KEYBINDING_COMMANDS','flattenWhenChildren','buildWhenExpressionGroup','conditionParts','setConditionIdentifier','setConditionNegated','defaultWhenCondition','defaultWhenGroup','formatShortcutKeyLabel','formatShortcutLabel','EDITORS','evaluateWhenNode','matchesWhenClause','shortcutConflictKey','effectiveShortcutsForCommand','resolveShortcutCommand','resolveChatShortcutCommand','projectScriptIdFromCommand']);
const header='// Generated from pinned T3 Code '+pinned+'. MIT, see licenses/T3-Code-MIT.txt.\n// Regenerate: node tools/build-keybindings.cjs <native-checkout>\n';
// Qt's JS engine lacks toSorted/flatMap; copying and map/concat preserve native semantics.
const qtDeclarations=declarations.replace(/\.toSorted\(/g,'.slice().sort(')
 .replace('return [...KEYBINDING_GROUPS, OTHER_KEYBINDING_GROUP].flatMap(', 'return flattenNativeGroups([...KEYBINDING_GROUPS, OTHER_KEYBINDING_GROUP], ')
 .replace('const parsePrimary =', 'let parseOr;\n  const parsePrimary =')
 .replace('const parseOr =', 'parseOr =');
const scriptIdLimit=Number(inputs[0].match(/MAX_SCRIPT_ID_LENGTH = (\d+);/)[1]);
const compatibility='const isElectron=true;\nconst scriptIdLimit='+scriptIdLimit+';\nconst SCRIPT_RUN_COMMAND_PATTERN={parts:[{literal:"script."},null,{literal:".run"}]};\nfunction isScriptRunCommand(command){const id=command.slice(7,-4);return command.startsWith("script.")&&command.endsWith(".run")&&id.length>0&&id.length<=scriptIdLimit&&/^[a-z0-9][a-z0-9-]*$/.test(id); }\nfunction flattenNativeGroups(groups,mapper){return [].concat.apply([],groups.map(mapper));}\n';
const script=header+'var nativeKeys=(function(){\n"use strict";\n'+compatibility+qtDeclarations+'\nreturn {'+unique.join(',')+'};\n})();\n';
const sandbox={};vm.createContext(sandbox);vm.runInContext(script,sandbox);
fs.writeFileSync(path.join(repo,'src/native_editors.json'),JSON.stringify(Array.from(sandbox.nativeKeys.EDITORS).map(e=>({id:e.id,label:e.label})),null,2)+'\n');
fs.writeFileSync(path.join(repo,'src/keybinding_commands.json'),JSON.stringify(Array.from(sandbox.nativeKeys.STATIC_KEYBINDING_COMMANDS),null,2)+'\n');
fs.writeFileSync(path.join(repo,'ui/js/NativeKeybindings.js'),'.pragma library\n'+script);
fs.writeFileSync(path.join(repo,'ui/js/NativeKeybindings.sources.json'),JSON.stringify({commit:pinned,files:hashes,adaptations:['TypeScript type erasure and import removal; identical native constants and dependency order','Array.toSorted replaced with Array.slice().sort; group flatMap replaced with map/concat for Qt JS compatibility','Declare recursive parseOr binding before its closures to avoid Qt used-before-declared warning','Native context object spread becomes Object.assign for Qt JS compatibility','Qt desktop supplies the native desktop context default; script command schema predicate uses the pinned literal/id constraint without the Effect runtime'],outputSha256:crypto.createHash('sha256').update('.pragma library\n'+script).digest('hex')},null,2)+'\n');
console.log('Native keybindings: '+unique.length+' exported helpers/defaults, '+Buffer.byteLength(script)+' bytes');
