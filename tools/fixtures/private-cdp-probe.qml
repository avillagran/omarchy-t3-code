import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
    width: 640; height: 480; visible: true
    Component.onCompleted: {
        const script = WebEngine.script();
        script.injectionPoint = WebEngineScript.DocumentCreation;
        script.worldId = WebEngineScript.MainWorld;
        script.sourceCode = "Object.defineProperty(globalThis,'__o3PrivateCdpMinimal',{value:true});" + privateBridgeSource;
        inspector.userScripts.insert(script);
        page.devToolsView = inspector;
    }
    WebEngineView {
        id: page; anchors.fill: parent
        url: "data:text/html,<title>private-probe</title><p>isolated</p>"
    }
    WebEngineView {
        width: 100; height: 100; visible: false
        url: "data:text/html,<title>unrelated-private-page</title>"
    }
    WebEngineView {
        id: inspector; width: 600; height: 400; visible: false
        onJavaScriptConsoleMessage: function(level,message,line,source) {
            console.log("INSPECTOR_CONSOLE",message);
        }
    }
    Timer {
        interval: 1500; running: true; repeat: false
        onTriggered: {
            console.log("TARGET",page.devToolsId);
            inspector.runJavaScript(`(() => {
                const send = command => __o3PrivateCdp.send(JSON.stringify(command));
                send({id:1,method:'Runtime.evaluate',params:{expression:'document.title',returnByValue:true}});
                send({id:2,method:'Target.getTargetInfo',params:{}});
                send({id:3,method:'Runtime.enable',params:{}});
                send({id:4,method:'Runtime.evaluate',params:{expression:'[typeof InspectorFrontendHost,typeof DevToolsAPI,typeof __o3PrivateCdp]',returnByValue:true}});
                send({id:5,method:'Runtime.evaluate',params:{expression:'debugger; "not-paused"',returnByValue:true}});
                return 'SENT';
            })()`,function(value) { console.log("SEND_RESULT",value); });
        }
    }
    Timer {
        interval: 2500; running: true; repeat: false
        onTriggered: inspector.runJavaScript("JSON.stringify(__o3PrivateCdp.drain())",function(value) {
            console.log("PRIVATE_RESULT",value);
        })
    }
}
