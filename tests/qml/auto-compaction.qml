import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import qs.Commons
ShellRoot{
 id:root;property bool done:false
 function check(ok,msg){if(!ok){console.error("T3_AUTO_COMPACTION_QML_FAIL: "+msg);Qt.exit(1)}}
 Timer{interval:400;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 QtObject{
  id:fixture
  property var viewState:({autoCompactEnabled:true,autoCompactMinTokens:20000,autoCompactFallbackMinutes:60})
  property string selectedEnvironment:"fixture"
  property string selectedKey:"fixture/thread"
  property var catalog:({providers:[{instanceId:"codex",name:"Codex"}]})
  property var compactionStates:({"fixture/thread":{status:"scheduled",estimated:true,dueAt:Date.now()+60000}})
  function configureCompaction(patch){viewState=Object.assign({},viewState,patch)}
 }
 Window{
  id:win;visible:true;width:700;height:600;color:Color.popups.background
  Rectangle{anchors.fill:parent;color:Color.popups.background}
  Test.T3CompactionSettings{id:settings;bridge:fixture}
  TestCase{
   id:checks;when:false
   function runChecks(){
    win.requestActivate();settings.open();wait(220)
    root.check(findChild(settings.contentItem,"autoCompactPriority").height>0,"native TTL priority explanation is visible")
    root.check(settings.visible&&settings.minutes===60,"estimated default is one hour")
    var enabled=findChild(settings.contentItem,"autoCompactEnabled"),thread=findChild(settings.contentItem,"autoCompactThread")
    root.check(enabled.checked&&thread.checked,"global and thread defaults enabled")
    mouseClick(enabled,12,enabled.height/2);root.check(fixture.viewState.autoCompactEnabled===false,"global opt-out")
    mouseClick(enabled,12,enabled.height/2);root.check(fixture.viewState.autoCompactEnabled===true,"global re-enable")
    mouseClick(thread,12,thread.height/2);root.check(fixture.viewState.autoCompactExcludedThreads[0]==="fixture/thread","thread exclusion")
    mouseClick(thread,12,thread.height/2);root.check(fixture.viewState.autoCompactExcludedThreads.length===0,"thread re-enable")
    root.check(settings.statusText("scheduled")==="Programada","translated state")
    root.check(settings.statusText("uncertain")==="Resultado desconocido","uncertain never shown as success")
    settings.setMinutes(30);root.check(fixture.viewState.autoCompactFallbackMinutes===30,"default lifetime changes")
    settings.providerControl.currentIndex=1;wait(40);settings.setMinutes(5)
    root.check(fixture.viewState.autoCompactProviderMinutes["fixture/codex"]===5&&fixture.viewState.autoCompactFallbackMinutes===30,"provider overrides are scoped to the environment")
    settings.statusText("unsupported")
    win.contentItem.grabToImage(function(result){result.saveToFile("/tmp/t3-auto-compaction.png")});wait(150)
    settings.close();win.visible=false;root.done=true;console.log("T3_AUTO_COMPACTION_QML_PASS: settings, global/thread opt-out, TTL and translated status")
   }
  }
 }
}
