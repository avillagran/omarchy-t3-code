// Private original-engine baseline. No production query fallback or DOM heuristic.
'use strict';
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const readline = require('node:readline');
const {app, BrowserWindow} = require('electron');
const topLevel = process.env.O3_CONTEXT_TOP_LEVEL === '1';
assert.equal(process.versions.electron, '44.10.1');
fs.mkdirSync('/tmp/owned-native-context-profile', {mode:0o700});
app.setPath('userData', '/tmp/owned-native-context-profile');
app.setPath('sessionData', '/tmp/owned-native-context-profile');
let services, window, server, stage = 'app-ready', failed = false;
function reportFailure(errorStage, error) {
  if (failed) return;
  failed = true;
  process.stdout.write(JSON.stringify({kind:'fixture-error',stage:errorStage,
    errorType:['Error','TypeError','AssertionError'].includes(error?.name)?error.name:'other'})+'\n');
  // The private parent retires its native popup role before closing this view.
  // Reporting an error must not independently trigger the unsafe teardown.
}
async function close(code) {
  if (services) await services.close();
  if (window && !window.isDestroyed()) window.destroy();
  if (server) server.close();
  app.exit(code);
}
app.whenReady().then(async () => {
  services = await require('./native-electron-services.cjs').create({
    event: () => {},
    showMessageBox: async () => {throw Error('Unexpected native permission request');},
  });
  stage = 'native-session';
  const partition = await services.getPartition('native-context', true, 'profile');
  const session = await services.getSession('native-context', true, 'profile');
  stage = 'native-window';
  window = new BrowserWindow({show: true, useContentSize: true,
    width: 640, height: 480, title: 'Owned native context baseline',
    webPreferences: {webviewTag: true, sandbox: true, nodeIntegration: false,
      contextIsolation: !topLevel, ...(topLevel ? {session} : {})}});
  window.on('page-title-updated', event => event.preventDefault());
  window.webContents.on('will-attach-webview', (_event, preferences, params) => {
    assert.equal(params.partition, partition);
    preferences.sandbox = true;
    preferences.nodeIntegration = false;
    preferences.nodeIntegrationInSubFrames = false;
    preferences.contextIsolation = false;
    preferences.disableHtmlFullscreenWindowResize = true;
    delete preferences.preload;
  });
  let guest;
  if (topLevel) guest = window.webContents;
  else {
    const attachment = new Promise(resolve => window.webContents.once('did-attach-webview', (_event, guest) => resolve(guest)));
    stage = 'host-load';
    await window.loadURL('data:text/html,' + encodeURIComponent('<html><body style="margin:0"><webview partition="'+partition+'" src="about:blank" allowpopups="true" webpreferences="contextIsolation=false,sandbox=true,nodeIntegration=false" style="display:flex;width:100vw;height:100vh"></webview></body></html>'));
    guest = await attachment;
  }
  stage = 'guest-admission';
  if (!topLevel) assert.equal(guest.hostWebContents, window.webContents);
  assert.equal(guest.session, session);
  assert.equal(guest.getType(), topLevel ? 'window' : 'webview');
  if (process.env.O3_CONTEXT_SOURCE_INPUT === '1') guest.debugger.attach('1.3');
  const inputs = {keys:0, mouse:0};
  guest.on('before-input-event', () => inputs.keys++);
  guest.on('before-mouse-event', () => inputs.mouse++);
  guest.on('render-process-gone', (_event, details) => {
    process.stdout.write(JSON.stringify({kind:'renderer-gone',reason:details.reason,exitCode:details.exitCode})+'\n');
  });
  // The page supplies a fixed synthetic initial value; no queried text is
  // reconstructed into an IME context, and no selection is changed by script.
  server = http.createServer((_request, response) => {
    response.setHeader('Content-Type', 'text/html; charset=utf-8');
    response.end('<!doctype html><title>Owned native context baseline</title><textarea autofocus style="position:absolute;left:20px;top:30px;width:320px;height:90px">A😀日本Z</textarea><script>window.pointerCount=0;document.addEventListener("pointerdown",()=>window.pointerCount++)</script>');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  stage = 'guest-load';
  await guest.loadURL('http://127.0.0.1:'+server.address().port+'/');
  window.show(); window.focus(); guest.focus();
  assert.equal(guest.getLastWebPreferences().sandbox, true);
  assert.equal(guest.getLastWebPreferences().nodeIntegration, false);
  process.stdout.write(JSON.stringify({kind:'engine-ready',focusedFrame:!!guest.focusedFrame})+'\n');
  readline.createInterface({input:process.stdin}).on('line', async line => {
    if (failed && line !== 'quit') return;
    if (line === 'inspect') {
      const result = await guest.executeJavaScript('({focused:document.hasFocus(),active:document.activeElement?.tagName,matchesSynthetic:["A😀日本Z","A😀日本ZQ"].includes(document.querySelector("textarea").value),units:document.querySelector("textarea").value.length,start:document.querySelector("textarea").selectionStart,end:document.querySelector("textarea").selectionEnd,pointers:window.pointerCount})');
      const host = topLevel ? null : await window.webContents.executeJavaScript('(()=>{const r=document.querySelector("webview").getBoundingClientRect();return {width:innerWidth,height:innerHeight,view:[r.x,r.y,r.width,r.height]}})()');
      process.stdout.write(JSON.stringify({kind:'diagnostic',focusedFrame:!!guest.focusedFrame,
        windowFocused:window.isFocused(),parentFocused:window.webContents.isFocused(),guestFocused:guest.isFocused(),
        bounds:window.getBounds(),contentBounds:window.getContentBounds(),inputs:{...inputs},host,...result})+'\n');
      return;
    }
    if (line === 'capture') {
      const image = await guest.capturePage();
      process.stdout.write(JSON.stringify({kind:'capture',empty:image.isEmpty(),...image.getSize()})+'\n');
      return;
    }
    if (line === 'source-click' && process.env.O3_CONTEXT_SOURCE_INPUT === '1') {
      for (const type of ['mousePressed', 'mouseReleased']) {
        await services.humanInput(guest, guest.debugger, 'Input.dispatchMouseEvent',
          {type,x:50,y:50,button:'left',clickCount:1,buttons:type==='mousePressed'?1:0});
      }
      process.stdout.write(JSON.stringify({kind:'source-click-complete'})+'\n');
      return;
    }
    if (line !== 'quit') return reportFailure('unexpected-control');
    close(0);
  });
}).catch(error => {
  reportFailure(stage,error);
});
process.on('unhandledRejection', error => reportFailure('async-control',error));
process.on('uncaughtException', error => reportFailure('uncaught-control',error));
setTimeout(() => reportFailure('deadline'), 20000).unref();
