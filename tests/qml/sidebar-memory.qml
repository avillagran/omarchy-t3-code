import QtQuick
import QtQuick.Window
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
// Fixed-size delegate workload, isolated from servers, accounts and histories.
ShellRoot {
 QtObject {
  id:fixture
  property var state:({projects:[]})
  property var catalogs:({fixture:{canWrite:true,capabilities:{threadSettlement:true,threadSnooze:true,threadPinning:true,threadAutoSettleOptOut:true,threadTitleRegeneration:true}}})
  property var drafts:({})
  property string selectedKey:""
  property bool managementBusy:false
  property var managementRequest:null
  signal managementFinished(var data)
  function ensureCatalog(env){}
  function selectThread(env,id){}
 }
 Window {
  visible:true;width:400;height:1000;color:"#06192a"
  Column {
   width:400
   Repeater {
    model:Number(Quickshell.env("OT3_BENCH_ROWS") || 24)
    Test.T3ThreadRow {
     required property int index
     width:400;bridge:fixture;panelVisible:false
     thread:({key:"fixture/"+index,id:String(index),environmentId:"fixture",title:"Thread "+index,project:"Example project",branch:"main",canSnooze:true})
    }
   }
  }
 }
 Timer{interval:3500;running:true;onTriggered:console.log("SIDEBAR_MEMORY_READY")}
 Timer{interval:5000;running:true;onTriggered:Qt.quit()}
}
