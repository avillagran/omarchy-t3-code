#!/usr/bin/env node
// Replay the complete native clock with injected wall time and timer services.
import fs from'node:fs';import path from'node:path';import crypto from'node:crypto';import{execFileSync}from'node:child_process';import{stripTypeScriptTypes}from'node:module';import{pathToFileURL}from'node:url';
const[checkout,output]=process.argv.slice(2);if(!output)throw Error('usage: generate-thread-clock-fixtures.mjs CHECKOUT OUTPUT');
const revision='4cb6945dc60ed03f567dc5daafd083e52308e82b',filename='packages/client-runtime/src/clock.ts';fs.mkdirSync(output,{recursive:true});
const original=execFileSync('git',['-C',checkout,'show',revision+':'+filename],{encoding:'utf8'}),source=stripTypeScriptTypes(original);
const nowStart=Date.parse('2026-11-07T12:34:56.789Z');let now=nowStart,serial=0;const timers=new Map(),events=[];
function FakeDate(value){return new Date(value===undefined?now:value)}FakeDate.now=()=>now;
const schedule=(callback,delay)=>{const id=++serial;timers.set(id,{callback,delay});return id},cancel=id=>timers.delete(id);
globalThis.__nativeThreadClock={Date:FakeDate,setTimeout:schedule,clearTimeout:cancel};
const target=path.join(output,'native-clock.mjs');fs.writeFileSync(target,'const {Date,setTimeout,clearTimeout}=globalThis.__nativeThreadClock;\n'+source);const native=await import(pathToFileURL(target).href);
function snapshot(step){events.push({step,now,value:native.minuteTick.getSnapshot(),timers:[...timers.values()].map(timer=>timer.delay),calls:[...calls]})}
const calls=[];snapshot('cold');let unsubscribeA=native.minuteTick.subscribe(()=>calls.push('a'));snapshot('first-subscribe');const unsubscribeB=native.minuteTick.subscribe(()=>calls.push('b'));snapshot('second-subscribe');now+=1000;snapshot('cached-between-ticks');
function fire(){const[id,timer]=timers.entries().next().value;timers.delete(id);timer.callback()}
now=Date.parse('2026-11-07T12:35:00.000Z');fire();snapshot('minute-boundary');unsubscribeA();snapshot('one-reader-left');unsubscribeB();snapshot('last-reader-gone');now=Date.parse('2026-11-07T12:38:00.000Z');snapshot('unsubscribed-catches-up');
let unsubscribeC;unsubscribeA=native.minuteTick.subscribe(()=>{calls.push('reentrant-a');unsubscribeA();unsubscribeC=native.minuteTick.subscribe(()=>calls.push('reentrant-c'))});now=Date.parse('2026-11-07T12:39:00.000Z');fire();snapshot('reentrant-resubscribe');unsubscribeC();snapshot('reentrant-last-reader-gone');
let helper='.pragma library\n.import \"NativeLiveSet.js\" as Collections\n// Complete native clock module; the owner supplies actual timer and wall-clock services.\nfunction create(setTimeout,clearTimeout,Date){\nconst Set=Collections.LiveSet;\n'+source.replace(/^export /gm,'')+'\nreturn {minuteTick:minuteTick};\n}\n';
helper=helper.replace(/[ \t]+$/gm,'');
fs.writeFileSync(path.join(output,'NativeThreadClock.js'),helper);fs.writeFileSync(path.join(output,'clock-fixtures.json'),JSON.stringify({revision,sourceSha256:crypto.createHash('sha256').update(original).digest('hex'),nowStart,events},null,2)+'\n');fs.writeFileSync(path.join(output,'CLOCK-SOURCE.json'),JSON.stringify({revision,source:filename,sourceSha256:crypto.createHash('sha256').update(original).digest('hex'),execution:'Whole native clock.ts with injected Date and timer services',qtPlatformService:{path:'ui/js/NativeLiveSet.js',sha256:crypto.createHash('sha256').update(fs.readFileSync(new URL('../ui/js/NativeLiveSet.js',import.meta.url))).digest('hex')},adaptations:['TypeScript erasure','Qt factory injects timer and Date services','Only minuteTick is exposed','Factory-local live listener Set adapts Qt reentrant iteration'],outputSha256:crypto.createHash('sha256').update(helper).digest('hex')},null,2)+'\n');console.log(JSON.stringify({clockTraces:events.length}));
