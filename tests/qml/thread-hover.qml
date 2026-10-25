import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property bool finished:false
 Timer{interval:450;running:true;onTriggered:test.runChecks()}
 Timer{interval:200;running:root.finished;onTriggered:Qt.quit()}
 function check(ok,msg){if(!ok){console.error("T3_THREAD_HOVER_QML_FAIL: "+msg);Qt.exit(1)}}
 QtObject{
  id:fixture
  property var state:({projects:[{key:"fixture/project"}]})
  property var drafts:({"fixture/one":{text:"Saved draft",attachments:[]}})
  property var catalogs:({fixture:{canWrite:true,capabilities:{threadSettlement:true,threadSnooze:true},models:[{instanceId:"codex",model:"m",displayName:"Fixture model",driverKind:"codex"}],providers:[{instanceId:"codex",name:"Codex",driverKind:"codex"}]}})
  property string selectedKey:"fixture/other"
  property bool managementBusy:false
  property var managementRequest:null
  property int selected:0
  property int ensured:0
  property var actions:[]
  function selectThread(env,id){selected++}
  function ensureCatalog(env){ensured++}
  function manage(data){actions=actions.concat([data])}
  function discardDraft(key){var all=Object.assign({},drafts);all[key]={text:"",attachments:[]};drafts=all}
 }
 Window{
  id:win;visible:true;width:800;height:500;color:Color.popups.background
  Rectangle{anchors.fill:parent;color:Color.popups.background}
  Test.T3ThreadRow{id:row;x:20;y:30;width:280;bridge:fixture;thread:({key:"fixture/one",id:"one",environmentId:"fixture",environmentName:"LAN node",project:"Project",projectKey:"fixture/project",title:"A complete title with useful context",provider:"codex",model:"m",branch:"feature/sidebar",canSnooze:true});providerGlyph:"openai";stateLabel:"";timeLabel:"2m"}
  ListView{id:scroller;x:20;y:260;width:280;height:100;clip:true;model:3;delegate:Test.T3ThreadRow{required property int index;width:280;bridge:fixture;thread:({key:"fixture/list-"+index,id:"list-"+index,environmentId:"fixture",title:"Scrollable thread",project:"Project",provider:"codex",model:"m",canSnooze:true})}}
  TestCase{
   id:test;name:"ThreadHover";when:false
   function runChecks(){
    verify(waitForRendering(row));win.requestActivate();wait(250)
    mouseMove(win.contentItem,700,450);wait(100)
    root.check(Math.abs(row.height-Style.space(82))<0.1,"native 78+4 row geometry")
    mouseMove(row,80,45);wait(620)
    root.check(row.previewVisible,"delayed detail card opens")
    root.check(row.catalog.models[0].displayName==="Fixture model","model comes from native catalog")
    root.check(row.settleControl.visible&&row.snoozeControl.visible&&row.discardControl.visible,"hover actions revealed "+JSON.stringify({hovered:row.hovered,show:row.showActions,settle:row.settleControl.visible,snooze:row.snoozeControl.visible,discard:row.discardControl.visible,draft:row.hasDraft,canSettle:row.canSettle,canSnooze:row.canSnooze}))
    root.check(fixture.selected===0,"hover never selects")
    win.contentItem.grabToImage(function(result){result.saveToFile("/tmp/t3-thread-hover.png")});wait(180)
    mouseMove(row.previewPopup.contentItem,40,20);wait(200);root.check(row.previewVisible,"pointer can enter the detail card")
    mouseMove(row,80,75);wait(80)
    mouseClick(row.settleControl,row.settleControl.width/2,row.settleControl.height/2)
    root.check(fixture.actions.length===1&&fixture.actions[0].action==="settle","settle uses native management")
    root.check(fixture.actions[0].thread==="one"&&fixture.actions[0].environment==="fixture","scoped action")
    root.check(fixture.selected===0,"action does not navigate")
    mouseMove(row,80,45);wait(520)
    mouseClick(row.snoozeControl,row.snoozeControl.width/2,row.snoozeControl.height/2);wait(100)
    root.check(row.snoozePopup.visible&&!row.previewVisible,"snooze menu replaces tooltip")
    root.check(row.snoozePopup.choices.length>=3&&row.presets(new Date(2026,9,20,10)).map(function(p){return p.label}).join()==="In 1 hour,In 3 hours,This evening,Tomorrow,Next week","original presets")
    var first=row.snoozePopup.itemAt(0);wait(180);first.forceActiveFocus();keyClick(Qt.Key_Return);wait(80)
    root.check(fixture.actions.length===2&&fixture.actions[1].action==="snooze"&&Date.parse(fixture.actions[1].snoozedUntil)>Date.now(),"preset dispatches future native snooze")
    root.check(row.presets(new Date(2026,9,18,10)).filter(function(p){return p.label==="Next week"}).length===0,"Sunday Monday duplicate removed")
    mouseMove(row,80,45);wait(80)
    mouseClick(row.discardControl,row.discardControl.width/2,row.discardControl.height/2)
    root.check(fixture.drafts["fixture/one"].text===""&&fixture.selected===0,"discard stays on current chat")
    row.thread=Object.assign({},row.thread,{settled:true,canSnooze:false});wait(60)
    root.check(!row.snoozeControl.visible,"blocked work cannot snooze")
    mouseClick(row.settleControl,row.settleControl.width/2,row.settleControl.height/2)
    root.check(fixture.actions.length===3&&fixture.actions[2].action==="unsettle","settled action reverses")
    mouseMove(win.contentItem,700,450);wait(160);mouseMove(row,80,row.height/2);wait(620);root.check(row.previewVisible,"preview can reopen")
    row.pooled=true;root.check(!row.previewVisible,"pooling closes preview")
    row.pooled=false;row.panelVisible=false;root.check(!row.previewVisible,"closed panel has no popup")
    var listed=scroller.itemAtIndex(0);mouseMove(listed,80,45);wait(620);root.check(listed.previewVisible,"list delegate opens detail card")
    scroller.contentY=25;root.check(!listed.previewVisible,"scroll closes anchored preview")
    win.visible=false;root.finished=true;console.log("T3_THREAD_HOVER_QML_PASS: geometry, preview, scoped actions, snooze presets, draft and recycling")
   }
  }
 }
}
