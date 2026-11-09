// Whole original platform module, original language declarations and actual Effect.
import fs from 'node:fs';
import vm from 'node:vm';
import {createRequire, stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [native, dependencies, compiler, driver] = process.argv.slice(2);
if (!fs.existsSync('/run/.containerenv') || process.versions.node !== '26.13.0')
  throw Error('Private pinned runtime required');
const require = createRequire(dependencies+'/package.json');
if (require('effect/package.json').version !== '4.1.2') throw Error('Canonical Effect required');
const Schema = await import(pathToFileURL(require.resolve('effect/Schema')).href);
const compilerRequire = createRequire(compiler+'/package.json');
const Babel = await import(pathToFileURL(compilerRequire.resolve('@babel/core')).href);
let current;
const context = vm.createContext({Uint8Array, fetch: (...args) => current.fetch(...args)});
function synthetic(exports) {
  return new vm.SyntheticModule(Object.keys(exports), function() {
    for (const [name, value] of Object.entries(exports)) this.setExport(name, value);
  }, {context});
}
function source(name, text) {
  return new vm.SourceTextModule(stripTypeScriptTypes(text, {mode:'strip'}), {identifier:name, context});
}
const transcription = source('transcription', fs.readFileSync(native+'/packages/client-runtime/src/voice-input/transcription.ts', 'utf8'));
const schema = synthetic(Schema);
await transcription.link(spec => {
  if (spec !== 'effect/Schema') throw Error('Unexpected transcription import');
  return schema;
});
await transcription.evaluate();
const sharedText = stripTypeScriptTypes(fs.readFileSync(native+'/packages/shared/src/speech.ts', 'utf8'), {mode:'strip'});
const sharedAst = Babel.parseSync(sharedText, {sourceType:'module', configFile:false, babelrc:false});
const wanted = new Set(['effectiveSpeechLanguage', 'cloudSpeechLanguages']);
const selected = sharedAst.program.body.filter(node => {
  const declaration = node.declaration;
  return declaration && wanted.has(declaration.id?.name ?? declaration.declarations?.[0]?.id?.name);
});
if (selected.length !== 2) throw Error('Whole original language declarations required');
const shared = source('shared', selected.map(node => sharedText.slice(node.start, node.end)).join('\n'));
const transcriptionApi = transcription.namespace;
const forwarded = ['requireCloudSpeechKey','transcribeCloudPcm','getEnvironmentSpeechStatus',
  'getEnvironmentSpeechStreamUrl','openSpeechStream','transcribeEnvironmentPcm'];
const runtime = synthetic(Object.fromEntries([
  ...forwarded.map(name => [name, (...args) => current[name](...args)]),
  ['VoiceTranscriptionError', transcriptionApi.VoiceTranscriptionError],
  ['throwIfVoiceTranscriptionAborted', transcriptionApi.throwIfVoiceTranscriptionAborted],
]));
const capture = synthetic({createBrowserPcmCapture: (...args) => current.createBrowserPcmCapture(...args)});
const run = synthetic({runEnvironmentSpeech: (...args) => current.runEnvironmentSpeech(...args)});
const platform = source('platform', fs.readFileSync(native+'/apps/web/src/speech/browserVoiceInput.ts', 'utf8'));
const imports = new Map([
  ['effect/Schema', schema], ['@t3tools/shared/speech', shared],
  ['@t3tools/client-runtime/voice-input', runtime], ['./browserPcmCapture', capture],
  ['./runEnvironmentSpeech', run],
]);
await platform.link(spec => {
  if (!imports.has(spec)) throw Error('Unreviewed platform import '+spec);
  return imports.get(spec);
});
await platform.evaluate();
vm.runInThisContext(fs.readFileSync(driver, 'utf8'), {filename:driver});
const result = await runVoicePlatformOracle(deps => {
  current = deps;
  return {...platform.namespace, ...transcriptionApi, AbortController,
    errorPredicate:Schema.is(transcriptionApi.VoiceTranscriptionError)};
});
process.stdout.write(JSON.stringify(result)+'\n');
