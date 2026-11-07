#!/usr/bin/env node
// Capture and execute the actual native session header callback; no global UA override.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {stripTypeScriptTypes} from 'node:module';
const [source, output] = process.argv.slice(2);
const revision = '4cb6945dc60ed03f567dc5daafd083e52308e82b';
const manifest = JSON.parse(fs.readFileSync(source + '/SOURCE.json', 'utf8'));
if (manifest.revision !== revision) throw Error('Pinned native input required');
for (const [name, hash] of Object.entries(manifest.files))
  if (crypto.createHash('sha256').update(fs.readFileSync(source + '/' + name)).digest('hex') !== hash)
    throw Error('Changed native input: ' + name);
const text = fs.readFileSync(source + '/apps/desktop/src/preview/BrowserSession.ts', 'utf8');
const patterns = text.match(/^const TEAMS_URL_PATTERNS = .*;$/m)?.[0];
const start = text.indexOf('browserSession.webRequest.onBeforeSendHeaders(');
const end = text.indexOf('const next = new Map(sessions);', start);
if (!patterns || start < 0 || end < start) throw Error('Native extraction changed');
let filter, callback;
new Function('browserSession', stripTypeScriptTypes(patterns + '\n' + text.slice(start, end)))({
  webRequest: {onBeforeSendHeaders: (selected, handler) => {filter = selected; callback = handler;}}
});
if (!callback) throw Error('Native callback missing');
const headers = [{}, {Accept:'text/html'}, {'User-Agent':'Mozilla/5.0'},
  ...['User-Agent','user-agent','USER-AGENT','uSeR-aGeNt'].flatMap(name => [
    'Mozilla/5.0 Electron/40.0.0 Chrome/144.0.0',
    'Mozilla/5.0 Electron/nightly+build Chrome/144.0.0',
    'Electron/40.0.0 Chrome/144.0.0',
    'A Electron/1 Electron/2 tail',
    'A Electron/ token',
    'A electron/1 tail',
    'A Electron/1\tB',
    'A Electron/1\u00a0B',
    'A Electron/1\u0085B',
    'A Electron/1\ufeffB',
    '',
  ].map(value => ({Accept:'text/html',[name]:value,'X-Private-Fixture':'synthetic'}))),
  {'User-Agent':'first Electron/1 value','user-agent':'second Electron/2 value'},
  JSON.parse('{"__proto__":"header-value","User-Agent":"A Electron/1 B"}')];
const cases = headers.map(requestHeaders => {
  let expected;
  callback({requestHeaders}, result => {expected = result;});
  if (expected === undefined) throw Error('Native callback did not settle');
  return {requestHeaders,expected};
});
fs.writeFileSync(output, JSON.stringify({revision,sources:manifest.files,filter,cases},null,2)+'\n');
console.error('NATIVE_TEAMS_HEADERS_PASS: '+cases.length+' actual callback cases');
