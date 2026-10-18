#!/usr/bin/env node
// Cover the native wizard's computed labels and original failure-copy lookup.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const sandbox = {};
vm.runInNewContext(fs.readFileSync('ui/js/NativeBrowserImport.js', 'utf8').replace(/^\.pragma library\n/, ''), sandbox);
const required = new Set(JSON.parse(fs.readFileSync('translations/browser-import-dynamic-keys.json')));
for (const copy of Object.values(sandbox.nativeImport.failureCopy)) required.add(copy);
// Include the adjacent native profile manager as well as the wizard.
for (const file of ['ui/qml/T3BrowserImportWizard.qml', 'ui/qml/T3BrowserProfiles.qml']) {
  for (const match of fs.readFileSync(file, 'utf8').matchAll(/\btr\(("(?:[^"\\]|\\.)*")/g)) required.add(JSON.parse(match[1]));
}
const catalog = JSON.parse(fs.readFileSync('translations/i18n.json'));
for (const [locale, entries] of Object.entries(catalog)) {
  for (const key of required) assert.equal(typeof entries[key], 'string', `${locale}: missing import copy: ${key}`);
}
console.log(`Browser import translation checks passed: ${Object.keys(catalog).length} locales × ${required.size} required labels/errors`);
