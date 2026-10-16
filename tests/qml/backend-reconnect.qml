import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 function check(ok,msg){if(!ok){console.error("BACKEND_RECONNECT_QML_FAIL "+msg);Qt.exit(1)}}
 Timer{interval:250;running:true;onTriggered:{var b=Test.T3Backend
  b.selectedEnvironment="local";b.selectedThread="fixture";b.selectedKey="local/fixture"
  b.drafts={"local/fixture":{text:"Do not lose this draft",attachments:[]}};b.transcript=[{id:"retained",text:"retained"}]
  b.catalogs={local:{error:"offline",models:[]},node:{models:[{id:"remote"}]}}
  b.localBackend={running:false,owner:"stopped"};b.state={threads:[],environments:[],working:0}
  b.action({op:"backend-control",result:{status:"starting",local:{running:false,owner:"stopped"}}})
  check(b.localRecoveryAttempts===30,"activation waits for listener even without runtime PID")
  b.receive(JSON.stringify({type:"snapshot",data:{threads:[],environments:[{id:"local",state:"error"}],working:0,stale:true}}))
  check(b.localRecoveryAttempts>0,"offline snapshot must not finish recovery")
  b.acceptBackendStatus({running:true,owner:"headless",pid:123})
  check(b.localRecoveryAttempts===30,"runtime appearance wakes recovery")
  b.receive(JSON.stringify({type:"snapshot",data:{threads:[],environments:[{id:"local",state:"ready"}],working:0,stale:false}}))
  check(b.localRecoveryAttempts===0,"ready snapshot ends short retries")
  check(!b.catalogs.local&&b.catalogs.node.models.length===1,"only local stale catalog invalidated")
  check(b.draft.text==="Do not lose this draft"&&b.transcript[0].id==="retained","draft and visible content retained")
  b.acceptBackendStatus({running:true,owner:"headless",pid:123});check(b.localRecoveryAttempts===0,"unchanged status does not reload chat")
  b.acceptBackendStatus({running:true,owner:"headless",pid:124});check(b.localRecoveryAttempts===30,"restarted PID reconnects")
  b.action({op:"backend-control",result:{status:"stopped",local:{running:false,owner:"stopped"}}});
  console.log("BACKEND_RECONNECT_QML_PASS: delayed listener, PID transition, cache renewal, unchanged ownership and draft preservation");Qt.quit()
 }}
}
