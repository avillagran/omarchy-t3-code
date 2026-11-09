// Run only in a private container. Pure metadata, no provider or account store.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../ui/js/NativeProviderAuth.js'), 'utf8');
const native = vm.createContext({});
vm.runInContext(source, native);
const method = (id, canRemove) => ({id, name: '<b>Connection</b>', type: 'agent', description: null, ...(canRemove === undefined ? {} : {canRemove})});
const profiles = native.profileMethods({methods: [method('chatgpt-profile:a', true), method('chatgpt-profile:b', false), method('chatgpt'), method('chatgpt-profile:future', 'true')]});
assert.deepEqual(Array.from(profiles, value => value.id), ['chatgpt-profile:a', 'chatgpt-profile:b']);
assert.equal(profiles[0].name, '<b>Connection</b>');
assert.equal(native.selectedId('', profiles), 'chatgpt-profile:a');
assert.equal(native.selectedId('chatgpt-profile:b', profiles), 'chatgpt-profile:b');
assert.equal(native.selectedId('chatgpt-change-account', profiles), 'chatgpt-change-account');
assert.equal(native.selectedId('chatgpt-profile:deleted', profiles), 'chatgpt-profile:a');
assert.equal(native.selectedId('', []), 'chatgpt-change-account');
assert.equal(native.removable('chatgpt-profile:a', profiles), true);
assert.equal(native.removable('chatgpt-profile:b', profiles), false);
assert.equal(native.removable('chatgpt-change-account', profiles), false);
assert.equal(native.removable('chatgpt-profile:a', native.profileMethods({methods:[method('chatgpt-profile:a')]})), false);
const poisoned = JSON.parse('{"id":"chatgpt-profile:poisoned","name":"Connection","type":"agent","description":null,"__proto__":{"canRemove":true},"constructor":{"canRemove":true},"future":{"canRemove":true}}');
const inherited = Object.assign(Object.create({canRemove:true}), method('chatgpt-profile:inherited'));
for (const candidate of [poisoned, inherited]) {
    const projected = native.profileMethods({methods:[candidate]});
    assert.equal(projected.length, 1);
    assert.equal(native.removable(candidate.id, projected), false);
    for (const key of ['__proto__', 'constructor', 'future', 'canRemove']) assert.equal(Object.hasOwn(projected[0], key), false);
}
for (const phase of ['starting', 'waiting', 'verifying']) assert.equal(native.busy({state:{phase}}), true);
assert.equal(native.busy({busy:true,state:{phase:'idle'}}), true);
assert.equal(native.busy({busy:false,state:{phase:'idle'}}), false);
assert.equal(native.profileMethods({methods:Array(33).fill(method('chatgpt-profile:a',true))}).length, 0);
console.log('NATIVE_PROVIDER_AUTH_METADATA_PASS');
