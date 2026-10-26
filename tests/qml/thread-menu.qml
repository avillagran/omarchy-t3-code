import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property bool done:false
 Timer{interval:450;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 QtObject{
  id:fixture
  property var state:({projects:[{key:"remote/p",workspaceRoot:"/home/node/project"}]})
  property var catalogs:({remote:{canWrite:true,capabilities:{threadSettlement:true,threadSnooze:true,threadPinning:true,threadAutoSettleOptOut:true,threadTitleRegeneration:true}},other:{canWrite:false}})
  property var drafts:({})
  property bool managementBusy:false
  property var managementRequest:null
  property var actions:[]
  property string selectedKey:"other/thread"
  property int selections:0
  signal managementFinished(var data)
  function selectThread(env,id){selections++}
  function ensureCatalog(env){}
  function manage(data){managementRequest=Object.assign({requestId:"test"},data);actions=actions.concat([managementRequest])}
  function discardDraft(key){}
 }
 Window{
  visible:true;width:800;height:600;color:Color.popups.background
  Test.T3ThreadRow{id:row;width:330;bridge:fixture;thread:({id:"one",key:"remote/one",projectKey:"remote/p",environmentId:"remote",project:"Project",title:"Original target",branch:"main",canSnooze:true})}
  TestCase{id:checks;when:false
   function runChecks(){
    verify(!row.actionsLoaded,"idle row allocates no action controls");verify(!findChild(row,"t3ThreadContextMenu"));
    mouseClick(row,180,47,Qt.RightButton);var menu=findChild(row,"t3ThreadContextMenu");verify(menu.visible);compare(fixture.selections,0,"context menu does not select thread");verify(findChild(row,"threadPin").visible);verify(findChild(row,"threadRegenerate").visible);compare(row.actionPath,"/home/node/project");
    row.thread={id:"other",key:"other/other",environmentId:"other",projectKey:"other/p",title:"Recycled row"};findChild(row,"threadPin").triggered();compare(fixture.actions[0].environment,"remote");compare(fixture.actions[0].thread,"one","recycled row cannot retarget a context action");compare(fixture.actions[0].action,"pin");fixture.managementRequest=null;menu.close();
    row.confirmAction("thread-rename");findChild(row,"threadRenameInput").text="Renamed";verify(!row.releaseActions(),"an open editor cannot expire");compare(findChild(row,"threadRenameInput").text,"Renamed");row.submitAction();compare(fixture.actions[1].thread,"one");compare(fixture.actions[1].title,"Renamed");fixture.managementRequest=null;fixture.managementFinished({requestId:"test"});
    row.confirmAction("delete-thread");compare(fixture.actions.length,2,"opening Delete performs no mutation");row.submitAction();compare(fixture.actions[2].action,"delete-thread");verify(fixture.actions[2].confirmed);compare(fixture.actions[2].thread,"one");fixture.managementRequest=null;fixture.managementFinished({requestId:"test"});
    wait(250);verify(row.releaseActions(),"closed action controls can unload");verify(!row.actionsLoaded);row.openSnooze(false);verify(row.snoozePopup.visible,"controls recreate after unload");row.snoozePopup.close();
    console.log("THREAD_MENU_QML_PASS: native options, remote target, no selection, recycling snapshot, rename and explicit deletion");root.done=true
   }
  }
 }
}
