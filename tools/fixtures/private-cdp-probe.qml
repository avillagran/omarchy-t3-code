import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
 Component.onCompleted: {
  let script = WebEngine.script();
  script.injectionPoint = WebEngineScript.DocumentCreation;
  script.worldId = WebEngineScript.MainWorld;
  script.sourceUrl = Qt.resolvedUrl('../../ui/browser/private-cdp-bridge.js');
  inspector.userScripts.insert(script);
  page.devToolsView = inspector;
 }
 width:640; height:480; visible:true
 WebEngineView { id:page; anchors.fill:parent; url:'data:text/html,<title>private-probe</title><p>isolated</p>' }
 WebEngineView { id:inspector;

  width:600;height:400;visible:false
  onJavaScriptConsoleMessage:function(level,message,line,source){console.log('INSPECTOR_CONSOLE',message)}
 }
 Timer { interval:2500;repeat:false;running:true;onTriggered:{
  inspector.runJavaScript('JSON.stringify(__o3PrivateCdp.drain())',function(value){console.log('PRIVATE_RESULT',value)});
 }}
 Timer { interval:1500;repeat:false;running:true;onTriggered:{
  console.log('TARGET',page.devToolsId);
  inspector.runJavaScript(`(() => {
    __o3PrivateCdp.send(JSON.stringify({id:1,method:'Runtime.evaluate',params:{expression:'document.title',returnByValue:true}}));
    __o3PrivateCdp.send(JSON.stringify({id:2,method:'Target.getTargetInfo',params:{}}));
    return 'SENT';
  })()`,function(value){console.log('SEND_RESULT',value)});
 }}
}
