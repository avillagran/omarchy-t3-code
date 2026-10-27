import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import qs.Commons
ShellRoot {
 id:root;property bool done:false
 function check(ok,msg){if(!ok){console.error("AGENT_MEMORY_QML_FAIL: "+msg);Qt.exit(1)}}
 QtObject{
  id:fixture
  property var viewState:({})
  property var catalog:({providers:[{instanceId:"work",name:"Codex"}]})
  property string selectedEnvironment:"fixture"
  property string selectedKey:"fixture/thread"
  property var compactionStates:({})
  function configureCompaction(patch){viewState=Object.assign({},viewState,patch)}
 }
 Window{
  id:win;visible:true;width:800;height:520;color:Color.popups.background
  Test.T3AgentMemorySettings{id:settings;bridge:fixture;anchors.fill:parent;anchors.margins:24}
  TestCase{id:checks;when:false
   function runChecks(){
    root.check(settings.value===5,"release is active at five minutes by default")
    var enabled=findChild(settings,"idleReleaseEnabled")
    root.check(enabled.checked,"enabled without a server setting")
    mouseClick(enabled,12,enabled.height/2);root.check(fixture.viewState.idleReleaseEnabled===false,"disable locally")
    mouseClick(enabled,12,enabled.height/2);root.check(fixture.viewState.idleReleaseEnabled===true,"enable locally")
    settings.setMinutes(10);root.check(fixture.viewState.idleReleaseMinutes===10,"save global timeout")
    findChild(settings,"idleReleaseProvider").currentIndex=1;wait(30)
    settings.setMinutes(null);root.check(fixture.viewState.idleReleaseProviderMinutes["fixture/work"]===null,"per-instance opt out")
    settings.setMinutes(5);root.check(settings.value===5,"per-instance timeout")
    root.check(findChild(settings,"harnessCompactionExplanation").text.length>40,"explain harness-specific compaction")
    win.contentItem.grabToImage(function(result){result.saveToFile("/tmp/ot3-agent-memory.png")});wait(180)
    console.log("AGENT_MEMORY_QML_PASS: active defaults, opt-out, provider overrides and harness explanation");root.done=true
   }
  }
 }
 Timer{interval:400;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
}
