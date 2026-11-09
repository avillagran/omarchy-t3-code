// Run only inside the reviewed private Node container with read-only source.
// This evaluates the whole pinned original helper, not an expected-value copy.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { stripTypeScriptTypes } from 'node:module';
import vm from 'node:vm';
import path from 'node:path';
const [nativeRoot, clientRoot, output] = process.argv.slice(2);
if (!nativeRoot || !clientRoot || !output) throw new Error('native-root client-root private-output required');
const original = await readFile(path.join(nativeRoot, 'apps/web/src/browser/BrowserProfileReporter.ts'), 'utf8');
const sha = value => createHash('sha256').update(value).digest('hex');
assert.equal(sha(original), 'ec8dbd094cf6bff0eb545478e57ed8b3255f80d13193a9b9141e967505112045');
const suffix = original.slice(original.indexOf('export function takeDueProfileReports('));
const native = stripTypeScriptTypes(suffix, {mode:'strip'}).replace('export function', 'function');
const local = await readFile(path.join(clientRoot, 'ui/js/NativeBrowserProfileReporting.js'), 'utf8');
assert.doesNotMatch(local, /const due\s*:/, 'Qt helper must not retain a TypeScript local annotation');
function load(code) {
  const scope = vm.createContext({});
  vm.runInContext(code, scope);
  return scope;
}
function trace(helper) {
  const reported = new Map(), targets = new Map();
  const a = {}, b = {}, profiles = [{id:'work', name:'Work', kind:'persistent'}];
  let input = {profiles, defaultProfileId:'work', recordingFrameRate:30};
  const values = [];
  const step = name => values.push({name, due:Array.from(helper.takeDueProfileReports(reported, targets, input))});
  step('no-targets');
  targets.set('hidden-a', a); targets.set('background-b', b); step('all-operable-environments');
  step('failed-reports-remain-recorded');
  input = {...input}; step('unrelated-settings-shallow-update');
  input = {...input, profiles:[...profiles]}; step('explicit-equal-content-array-replacement');
  step('stable-array-reference');
  targets.set('hidden-a', {}); step('actual-config-identity-change');
  targets.delete('background-b'); step('target-revoked');
  targets.set('background-b', b); step('target-becomes-operable-again');
  input = {...input, recordingFrameRate:60}; step('recording-frame-rate-change');
  input = {...input, defaultProfileId:'default'}; step('default-profile-change');
  return values;
}
const reference = trace(load(native)), actual = trace(load(local));
assert.deepEqual(actual, reference);
assert.deepEqual(reference.map(v => v.due), [[], ['hidden-a','background-b'], [], [],
  ['hidden-a','background-b'], [], ['hidden-a'], [], ['background-b'],
  ['hidden-a','background-b'], ['hidden-a','background-b']]);
// Exercise the actual consumer boundary: no placeholder hydration, no panel
// state, reused cloned IPC arrays, explicit replacement, physical socket loss.
const consumer = load(local), sent = [];
const backend = {profileReportTargets:new Map(), profileReports:new Map(),
  profileReportInput:null, previewWorkerReady:true, send:value => sent.push(value)};
consumer.target(backend, {environment:'a', operable:true, sessionId:'socket-a', configId:'config-a'});
assert.equal(sent.length, 0);
consumer.settings(backend, {planModeEnabled:false}); assert.equal(sent.length, 0);
const settings = {browserProfiles:[], browserDefaultProfileId:'default', browserRecordingFrameRate:30};
consumer.settings(backend, settings); assert.equal(sent.length, 1);
consumer.settings(backend, {...settings, browserProfiles:[]}); assert.equal(sent.length, 1);
consumer.settings(backend, {...settings, browserProfiles:[]}, true); assert.equal(sent.length, 2);
consumer.session(backend, {environment:'a', write:true, state:'closed', sessionId:'older-socket'});
assert.equal(backend.profileReportTargets.size, 1);
consumer.session(backend, {environment:'a', write:true, state:'closed', sessionId:'socket-a'});
assert.equal(backend.profileReportTargets.size, 0);
consumer.target(backend, {environment:'a', operable:true, sessionId:'socket-b', configId:'config-b'});
assert.equal(sent.length, 3);
consumer.lost(backend);
assert.equal(backend.profileReportInput, null); assert.equal(backend.profileReportTargets.size, 0);
await writeFile(output, JSON.stringify({sourceSha256:sha(original), consumerSha256:sha(local),
  traces:reference, consumerAssertions:10, runtime:process.version, status:'passed'}, null, 2)+'\n', {flag:'wx'});
