// Execute the complete pinned original describe block, with assertion/type adapters.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const {stripTypeScriptTypes} = require('node:module');
const original = fs.readFileSync('/original/Sidebar.logic.test.ts', 'utf8');
const start = original.indexOf('describe("searchSidebarThreads", () => {');
const end = original.indexOf('\ndescribe(', start + 1);
assert.ok(start >= 0 && end > start);
const body = stripTypeScriptTypes(original.slice(start, end), {mode: 'strip'});
let callbacks = 0, assertions = 0;
const context = vm.createContext({module: {exports: {}}});
vm.runInContext(fs.readFileSync('/case/NativeSearchRuntime.cjs', 'utf8'), context);
context.localEnvironmentId = 'local';
// The fixtures contain already-valid opaque IDs; branding has no runtime search behavior.
context.ThreadId = {make: value => value};
context.describe = (name, run) => { assert.equal(name, 'searchSidebarThreads'); run(); };
context.it = (_name, run) => { run(); callbacks++; };
context.expect = actual => ({toEqual: expected => { assert.deepStrictEqual(actual, expected); assertions++; }});
vm.runInContext('const {searchSidebarThreads,threadSearchMatchKey}=module.exports;\n' + body, context);
assert.equal(callbacks, 1);
assert.equal(assertions, 1);
console.log('ORIGINAL_SIDEBAR_SEARCH_PASS ' + JSON.stringify({callbacks, assertions,
    wholeOriginalDescribeBlock: true, electron: process.versions.electron}));
