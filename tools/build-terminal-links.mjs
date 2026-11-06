#!/usr/bin/env node
// Retain the pinned link algorithm; Qt supplies PCRE matches where its JS engine lacks groups.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {stripTypeScriptTypes} from 'node:module';
const [checkout]=process.argv.slice(2);
if(!checkout)throw Error('usage: build-terminal-links.mjs T3_CHECKOUT');
const revision='c55db3e9c72fdf584f4b54192fb39beb88667b59';
const file='apps/web/src/terminal-links.ts';
const original=execFileSync('git',['-C',checkout,'show',`${revision}:${file}`],{encoding:'utf8'});
const matching='packages/shared/src/keybindingMatching.ts';
const platformSource=execFileSync('git',['-C',checkout,'show',`${revision}:${matching}`],{encoding:'utf8'});
const surfacePath='apps/web/src/terminal/ghostty/surface.ts';
const surfaceSource=execFileSync('git',['-C',checkout,'show',`${revision}:${surfacePath}`],{encoding:'utf8'});
const corePath='apps/web/src/terminal/ghostty/core.ts';
const coreSource=execFileSync('git',['-C',checkout,'show',`${revision}:${corePath}`],{encoding:'utf8'});
function block(name){const at=surfaceSource.search(new RegExp('(?:export )?function '+name+'\\('));if(at<0)throw Error(name);return surfaceSource.slice(at,surfaceSource.indexOf('\n}',at)+2);}
const surfaceFunctions=['terminalCellText','terminalRowText','terminalColumnOffset','terminalColumnAtOffset','terminalLinkAtPositionWithRange'].map(block).join('\n');
const wide=coreSource.match(/export const GHOSTTY_CELL_WIDE = \{[^]*?\} as const;/)[0];
const pathsPath='packages/shared/src/path.ts',fileLinksPath='packages/shared/src/fileLinks.ts';
const pathsSource=execFileSync('git',['-C',checkout,'show',`${revision}:${pathsPath}`],{encoding:'utf8'});
const fileLinksSource=execFileSync('git',['-C',checkout,'show',`${revision}:${fileLinksPath}`],{encoding:'utf8'});
function from(source,name){const at=source.search(new RegExp('(?:export )?function '+name+'\\('));if(at<0)throw Error(name);return source.slice(at,source.indexOf('\n}',at)+2);}
let split=from(fileLinksSource,'splitFilePathPosition');
split=split.replace(/return \{\n    path: ([^]*?),\n    \.\.\.\(line > 0 \? \{ line \} : \{\}\),\n    \.\.\.\(column !== undefined && column > 0 \? \{ column \} : \{\}\),\n  \};/,'return Object.assign({path: $1}, line > 0 ? {line} : {}, column !== undefined && column > 0 ? {column} : {});');
if(split.includes('...'))throw Error('Path-position adapter needs review');
const pathFunctions=['isWindowsDrivePath','isUncPath','isWindowsAbsolutePath','isAbsolutePath','isNetworkPath'].map(name=>from(pathsSource,name)).join('\n');
const fileFunctions=['formatFilePathPosition','isWindowsPathStyle','joinPath','inferHomeFromCwd','resolvePathLinkTarget'].map(name=>from(fileLinksSource,name)).join('\n');
const positions=fileLinksSource.match(/const POSITION_SUFFIX_CAPTURE_PATTERN[^;]+;/)[0]+'\n'+fileLinksSource.match(/const POSITION_HASH_PATTERN[^;]+;/)[0];
const platform=platformSource.match(/export function isMacPlatform\([^]*?\n\}/)[0];
let source=stripTypeScriptTypes(platform+'\n'+original.replace(/^import[^;]+;\n/,'')+'\n'+wide+'\n'+surfaceFunctions+'\n'+positions+'\n'+pathFunctions+'\n'+split+'\n'+fileFunctions);
// Keep JavaScript's whitespace set without PCRE's Unicode \d expansion.
function qtPattern(pattern) {
 const space=String.raw`\x{0009}-\x{000d}\x{0020}\x{00a0}\x{1680}\x{2000}-\x{200a}\x{2028}\x{2029}\x{202f}\x{205f}\x{3000}\x{feff}`;
 let output='',inside=false;
 for(let index=0;index<pattern.length;index++){
  const value=pattern[index];
  if(value==='\\'&&index+1<pattern.length){
   const next=pattern[++index];
   if(next==='s')output+=inside?space:'['+space+']';
   else if(next==='S'){if(inside)throw Error('Unsupported negated whitespace inside class');output+='[^'+space+']';}
   else output+='\\'+next;
  }else{if(value==='[')inside=true;else if(value===']')inside=false;output+=value;}
 }
 return output;
}
// QV4's RegExp.source adds an extra escape before slashes. Serialize source
// patterns directly so the PCRE bridge receives the upstream pattern unchanged.
source=source.replace(/const (URL_PATTERN|FILE_PATH_PATTERN|QUOTED_FILE_PATH_PATTERN|REQUEST_ROUTE_PATTERN|SCHEME_URL_PATTERN)\s*=\s*(\/[^]*?\/[gimuys]*);/g,(_,name,literal)=>{
 const expression=Function('return '+literal)();
 return 'const '+name+' = '+JSON.stringify({source:qtPattern(expression.source),flags:expression.flags})+';';
});
source=source.replace('formatFilePathPosition({ ...position, path: resolvedPath })','formatFilePathPosition(Object.assign({}, position, {path: resolvedPath}))').replace(/^export /gm,'').replace(/line\.matchAll\((\w+)\)/g,'nativeMatches(line, $1.source, $1.flags)').replace(/\.toSorted\(/g,'.slice().sort(').replace(/\.at\(-1\)/g,'.slice(-1)[0]').replace('next.replaceAll("/", "\\\\")','next.split("/").join("\\\\")').replace(/\.trimEnd\(\)/g,'.replace(/[\\u0009-\\u000d\\u0020\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000\\ufeff]+$/, \"\")');
source=source.replace(/[ \t]+$/gm,'');
const output='.pragma library\n// Original T3 Code '+revision+'.\nfunction create(nativeMatches) {\n'+source+'\nreturn {extractTerminalLinks:extractTerminalLinks,isTerminalUrl:isTerminalUrl,collectWrappedTerminalLinkLine:collectWrappedTerminalLinkLine,isTerminalLinkActivation:isTerminalLinkActivation,terminalLinkAtPositionWithRange:terminalLinkAtPositionWithRange,resolvePathLinkTarget:resolvePathLinkTarget,isNetworkPath:isNetworkPath};\n}\n';
fs.writeFileSync('ui/js/NativeTerminalLinks.js',output);
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
fs.writeFileSync('ui/js/NativeTerminalLinks.sources.json',JSON.stringify({revision,files:{[file]:hash(original),[matching]:hash(platformSource),[surfacePath]:hash(surfaceSource),[corePath]:hash(coreSource),[pathsPath]:hash(pathsSource),[fileLinksPath]:hash(fileLinksSource)},adaptations:['TypeScript erasure and module factory for Qt JS','RegExp matchAll uses a Qt PCRE matcher with named groups and UTF16 offsets','Array.toSorted becomes copying sort','RegExp source/flags are serialized directly to avoid Qt slash re-escaping','Whitespace classes use the exact JavaScript set while decimal escapes remain ASCII','Array.at(-1) becomes copying slice; String.trimEnd uses the exact JavaScript whitespace set','File position object spread becomes Object.assign; slash replaceAll becomes split/join'],outputSha256:hash(output)},null,2)+'\n');
console.log('Generated pinned terminal-link helpers');
