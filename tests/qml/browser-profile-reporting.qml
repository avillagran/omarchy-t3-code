import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client

ShellRoot {
 id: root
 readonly property var bridge: Client.T3Backend
 property var waiting: []
 function check(value, message) { if (!value) throw Error(message) }
 function reports() { return bridge.state.profileFixture ? bridge.state.profileFixture.reports : [] }
 function action(name) { bridge.send({op:'profile-fixture', action:name}) }
 function waitFor(predicate, message) { return new Promise(function(resolve, reject) {
  root.waiting = root.waiting.concat([{predicate:predicate, message:message, resolve:resolve, reject:reject, until:Date.now()+3000}])
 }) }
 Timer {interval:15;repeat:true;running:true;onTriggered:{
  var next=[];root.waiting.forEach(function(item){try{
   if(item.predicate())item.resolve();else if(Date.now()>item.until)item.reject(Error(item.message));else next.push(item)
  }catch(error){item.reject(error)}});root.waiting=next
 }}
 function run() {
  var b=bridge
  return waitFor(function(){return b.profileReportTargets.size===2},'targets before hydration').then(function(){
   check(!b.panelVisible&&!b.previewVisible&&!b.selectedThread,'actual singleton stays hidden/unselected')
   check(b.profileReportInput===null&&reports().length===0,'placeholder settings never report')
   action('hydrate');return waitFor(function(){return reports().length===2},'all operable hidden environments')
  }).then(function(){
   check(reports().every(function(report){return report.environment==='a'||report.environment==='b'}),'denied target excluded')
   check(reports().every(function(report){return report.input.profiles.length===1&&report.input.profiles[0].id==='work'}),'raw user profiles only')
   action('clone');return waitFor(function(){return b.state.profileFixture.stage==='cloned'},'actual cloned settings snapshot')
  }).then(function(){
   check(reports().length===2,'cloned sidebar settings preserve profile reference/no retry')
   b.requestClientSettings({browserRecordingFrameRate:60})
   return waitFor(function(){return reports().length===4},'actual successful settings response reports both')
  }).then(function(){
   check(reports().slice(2).every(function(report){return report.input.recordingFrameRate===60}),'actual frame rate field')
   action('stale-close');return waitFor(function(){return b.state.profileFixture.stage==='stale-close'},'old socket close delivered')
  }).then(function(){
   check(b.profileReportTargets.size===2&&reports().length===4,'stale close cannot retire current target')
   action('config-change');return waitFor(function(){return reports().length===5},'actual changed config identity')
  }).then(function(){
   check(reports()[4].environment==='a'&&reports()[4].configId==='config-a2','only changed environment reports')
   action('close');return waitFor(function(){return b.profileReportTargets.size===1&&b.state.profileFixture.stage==='closed'},'physical closed socket target removed')
  }).then(function(){
   check(reports().length===5,'socket closure does not replay obsolete request')
   action('reopen');return waitFor(function(){return reports().length===6},'fresh socket admission')
  }).then(function(){
   check(reports()[5].sessionId==='socket-a2','reconnect uses new actual target receipt')
   action('revoke');return waitFor(function(){return b.profileReportTargets.size===1&&b.state.profileFixture.stage==='revoked'},'scope target withdrawal')
  }).then(function(){action('grant');return waitFor(function(){return reports().length===7},'scope target reappearance')}).then(function(){
   check(reports()[6].environment==='b','reappearance reports once')
   b.requestClientSettings({browserProfiles:[{id:'work',name:'Work',kind:'persistent'}]})
   return waitFor(function(){return reports().length===9},'explicit equal-content profile replacement')
  }).then(function(){
   console.log('BROWSER_PROFILE_REPORTING_QML_PASS '+JSON.stringify({reports:9, hiddenEnvironments:2,
    hydration:true, clonedSnapshots:true, actualSettings:true, staleClose:true, newSocket:true,
    explicitEqualReplacement:true, scope:'Actual production singleton with fixed metadata peer; no Rust authority, HTTP/WS or server reporter lifetime claim'}))
   Qt.quit()
  })
 }
 Timer {interval:200;running:true;onTriggered:root.run().catch(function(error){console.error('BROWSER_PROFILE_REPORTING_QML_FAIL '+error+' '+error.stack);Qt.exit(1)})}
 Timer {interval:16000;running:true;onTriggered:{console.error('BROWSER_PROFILE_REPORTING_QML_FAIL deadline');Qt.exit(1)}}
}
