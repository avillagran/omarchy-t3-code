// Link complete original search algorithms to the admitted local V8 runtime.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {createRequire, stripTypeScriptTypes} from 'node:module';
import {pathToFileURL, fileURLToPath} from 'node:url';

if (!fs.existsSync('/run/.containerenv') || process.versions.node !== '26.13.0')
  throw Error('Private pinned build runtime required');
const [native, compiler, output] = process.argv.slice(2);
if (!native || !compiler || !output) throw Error('Expected native, compiler and output directories');
const revision = 'ade473018d32003658b7d38693e083ee2e314bd8';
const sourceFiles = {
  'apps/web/src/components/Sidebar.logic.ts': '0389fec57b596f9a7f0c16c1416b86cfaabf766419d3dd9b2e3e6068863a39c5',
  'apps/web/src/components/Sidebar.logic.test.ts': '3b082310c441c8187d98021f2785ec42ef15afc19c6a1c85cdaf456d8c7ce7b1',
  'packages/shared/src/paletteSearch.ts': '1cf17bf43e0bf3c0c508b67bb707caf234c3e1eac6d068a7d5875d8d0633c2db',
  'packages/shared/src/paletteSearch.test.ts': '2160115ffc05f020a856e997a41b9d369afafe86a0c7ee5e2c4cddf157cc735e',
  'packages/shared/src/git.ts': '2a17d67d3a1c00366e998d33bc57797094ede743339bef5520fa9887add23a1c',
  LICENSE: '935d8f2af0c703f9c39517ee57cc4930b19d02d533be930b63f0e82f93614b43',
  'packages/shared/src/threadPullRequests.ts': 'a894c17a83ecd3762e9e8ade439373128efd2da9e9898869d14b0a5d164482b7',
  'packages/client-runtime/src/state/threadSort.ts': '52a7ff2bc25fad376b67dc84bdda5bc01536230aa58eef5f264f202550c2b805',
  'packages/client-runtime/src/state/threadSearch.ts': '687ef946a9ef0da92befded329ddc68816e06f7f5957d2f505d9124f6aaf781a',
  'apps/web/src/components/CommandPalette.logic.ts': '68f65fa1b148c5c2fc64ac7f3bb3681283c60d6f877374a49b902976c9ebd5c1',
  'apps/web/src/components/chat/threadFindStore.ts': '13445a219a80e8d657e9cff1f8674b0f8eb07f7227028a44c233e656d9555842',
};
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const source = {};
for (const [name, expected] of Object.entries(sourceFiles)) {
  const bytes = fs.readFileSync(path.join(native, name));
  if (hash(bytes) !== expected) throw Error('Native source changed: ' + name);
  source[name] = bytes.toString();
}
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const lock = fs.readFileSync(path.join(root, 'tools/voice-compiler/package-lock.json'));
if (hash(lock) !== hash(fs.readFileSync(path.join(compiler, 'package-lock.json'))))
  throw Error('Compiler lock changed');
const require = createRequire(path.join(compiler, 'package.json'));
const Babel = await import(pathToFileURL(require.resolve('@babel/core')).href);
const parse = text => Babel.parseSync(stripTypeScriptTypes(text, {mode: 'strip'}), {
  sourceType: 'module', configFile: false, babelrc: false,
});
const allowedGlobals = new Set([
  'Array', 'Object', 'String', 'Number', 'Math', 'Date', 'JSON', 'Map', 'Set',
  'WeakMap', 'RegExp', 'Infinity', 'undefined', 'Boolean', 'Uint8Array', 'Error',
]);
const modules = [];
function link(file, name, selected, bindings, extraExports = []) {
  const body = [], exports = [], names = [];
  for (const original of parse(source[file]).program.body) {
    if (original.type === 'ImportDeclaration') continue;
    const declaration = original.type === 'ExportNamedDeclaration' ? original.declaration : original;
    if (!declaration) {
      if (selected && original.type === 'ExportNamedDeclaration' &&
          original.specifiers.every(specifier => !selected.includes(specifier.exported.name))) continue;
      throw Error('Unexpected native re-export: ' + file);
    }
    const declared = declaration.type === 'FunctionDeclaration' ? [declaration.id.name]
      : declaration.type === 'VariableDeclaration' ? declaration.declarations.map(item => {
        if (item.id.type !== 'Identifier') throw Error('Unexpected destructured native declaration');
        return item.id.name;
      }) : [];
    if (selected && !declared.some(key => selected.includes(key))) continue;
    if (selected && declared.some(key => !selected.includes(key)))
      throw Error('Partial native declaration selected');
    body.push(declaration); names.push(...declared);
    if (original.type === 'ExportNamedDeclaration') exports.push(...declared);
  }
  if (selected && (selected.length !== names.length || selected.some(key => !names.includes(key))))
    throw Error('Native declaration closure changed: ' + file);
  const program = Babel.parseSync('', {sourceType: 'script', configFile: false, babelrc: false});
  program.program.body = body;
  const text = Babel.transformFromAstSync(program, null, {
    sourceType: 'script', configFile: false, babelrc: false, comments: true,
  }).code;
  const exposed = [...new Set([...exports, ...extraExports])];
  const linked = 'const ' + name + '=(()=>{\n' + bindings + '\n' + text +
    '\nreturn Object.freeze({' + exposed.join(',') + '});\n})();\n';
  modules.push({file, name, declarations: names, exports: exposed});
  return linked;
}
const git = parse(source['packages/shared/src/git.ts']);
const prefix = git.program.body.flatMap(node => {
  const declaration = node.type === 'ExportNamedDeclaration' ? node.declaration : node;
  return declaration?.type === 'VariableDeclaration'
    ? declaration.declarations.filter(item => item.id.name === 'WORKTREE_BRANCH_PREFIX') : [];
});
if (prefix.length !== 1 || prefix[0].init.type !== 'StringLiteral' || prefix[0].init.value !== 't3')
  throw Error('Native branch prefix changed');
let code = '// Original T3 Code ' + revision + '. MIT; licenses/T3-Code-MIT.txt.\n' +
  '// Generated by tools/build-native-search-runtime.mjs.\n"use strict";\n';
code += link('packages/shared/src/paletteSearch.ts', 'NativePalette', null,
  'const WORKTREE_BRANCH_PREFIX="t3";');
code += link('packages/shared/src/threadPullRequests.ts', 'NativePullRequests', null, '');
code += link('packages/client-runtime/src/state/threadSort.ts', 'NativeThreadSort', null, '');
code += link('packages/client-runtime/src/state/threadSearch.ts', 'NativeThreadSearch', [
  'sentThreadOpens', 'localThreadOpens', 'localThreadOpensVersion', 'claimThreadOpen',
  'threadSearchSubject', 'subjectCache', 'buildThreadSearchSubject', 'LATE_ROOM',
  'nextShownOrder', 'visibleThreadSearch', 'threadSearchMatchKey',
], 'const {foldSearchQuery,THREAD_OPEN_DEDUPE_MS}=NativePalette;\n' +
   'const {threadSearchPullRequests}=NativePullRequests;\n' +
   'const {getThreadSortTimestamp}=NativeThreadSort;');
code += link('apps/web/src/components/CommandPalette.logic.ts', 'NativeCommandPalette', [
  'RECENT_THREAD_LIMIT', 'rankPaletteItem', 'scorePaletteItem',
  'filterCommandPaletteGroups', 'buildNestedSearchItems',
], 'const {foldSearchQuery,PALETTE_SECONDARY_LIMIT,PALETTE_THREAD_LIMIT,' +
   'rankSearchGroups,scoreSearchTerms,scoreThreadSearch}=NativePalette;');
code += link('apps/web/src/components/chat/threadFindStore.ts', 'NativeThreadFind', [
  'findForMessageMatch',
], 'const {messagePhrase,messageSearchTokens,findSearchTokenRanges}=NativePalette;');
code += link('apps/web/src/components/Sidebar.logic.ts', 'NativeSidebar', [
  'EMPTY_CONTENT_MATCH_KEYS', 'searchSidebarThreads',
], 'const {foldSearchQuery,rankSearchItems,scoreThreadSearch}=NativePalette;\n' +
   'const {threadSearchSubject,threadSearchMatchKey}=NativeThreadSearch;');
code += 'module.exports=Object.freeze({...NativePalette,...NativeThreadSearch,...NativeCommandPalette,...NativeThreadFind,...NativeSidebar});\n';
const undeclared = new Set();
Babel.traverse(Babel.parseSync(code, {sourceType: 'script', configFile: false, babelrc: false}), {
  ReferencedIdentifier(site) {
    const name = site.node.name;
    if (!site.scope.getBinding(name) && !allowedGlobals.has(name) && name !== 'module')
      undeclared.add(name);
  },
});
if (undeclared.size) throw Error('Unlinked native runtime dependencies: ' + [...undeclared].join(','));
fs.mkdirSync(output, {recursive: true});
fs.writeFileSync(path.join(output, 'NativeSearchRuntime.cjs'), code, {flag: 'wx'});
fs.writeFileSync(path.join(output, 'NativeSearchRuntime.sources.json'), JSON.stringify({
  revision, sourceFiles, compilerLockSha256: hash(lock), node: process.versions.node,
  outputSha256: hash(code), modules,
  adaptations: [
    'TypeScript erasure, exact native branch-prefix linkage and explicit original module dependency linkage',
    'Complete paletteSearch, threadPullRequests and threadSort modules; original pure declaration closures for threadSearch, command-palette filtering matched-message find navigation and sidebar search',
    'No algorithm, cache, regular expression, numeric scoring, native limit or ordering substitution',
  ],
  scope: 'Local V8 search algorithms and original subject/open/order helpers; Effect search subscriptions and UI catalog consumers are separate integrations',
}, null, 2) + '\n', {flag: 'wx'});
console.log('NATIVE_SEARCH_RUNTIME_GENERATED ' + JSON.stringify({bytes: Buffer.byteLength(code), modules: modules.length}));
