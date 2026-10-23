import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
// Low disk space notice: shown below the threshold, Clean up now runs the local
// storage cleanup, Worktrees requests the report and lists target/ sizes, and
// closing hides it until space recovers.
ShellRoot {
 id:root
 property int settings:0
 function check(ok,message){if(!ok){console.error("DISK_SPACE_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 QtObject {
  id:bridge
  property var diskSpace:({low:true,freeBytes:11*1073741824,thresholdBytes:15*1073741824})
  property var storageCleanup:({busy:false,running:false,error:"",report:null})
  property var diskReport:({busy:false,error:"",result:null})
  property bool diskNoticeDismissed:false
  property bool diskCleanupRan:false
  property var cleanups:[]
  property int reports:0
  function cleanLocalStorage(){diskCleanupRan=true;cleanups=cleanups.concat(["local"]);storageCleanup=({busy:true,running:true,error:"",report:null})}
  function requestDiskReport(){reports++;diskReport=({busy:false,error:"",result:{worktrees:[{path:"/h/.t3/worktrees/ot3/old-branch",title:"Old work",targetBytes:3*1073741824,merged:true,hasChanges:false},{path:"/h/.t3/worktrees/ot3/wip",targetBytes:0,merged:false,hasChanges:true}]}})}
 }
 FloatingWindow { id:win;visible:true;implicitWidth:480;implicitHeight:400
  Test.T3DiskSpaceNotice { id:notice;width:parent.width;bridge:bridge;onSettingsRequested:root.settings++ }
 }
 TestCase { id:keys;parent:win.contentItem;when:false }
 function texts(item,out){out=out||[];if(item.text!==undefined&&typeof item.text==="string"&&item.visible)out.push(item.text);for(var i=0;i<item.children.length;i++)texts(item.children[i],out);return out}
 Timer { interval:150;running:true;onTriggered:{
  check(notice.visible,"shown below the threshold");
  check(keys.findChild(notice,"t3DiskSpaceTitle").text.indexOf("11")>=0,"free space in the title");
  keys.mouseClick(keys.findChild(notice,"t3DiskSpaceCleanup"));
  check(bridge.cleanups.length===1&&bridge.cleanups[0]==="local","Clean up now runs the local cleanup");
  check(!keys.findChild(notice,"t3DiskSpaceCleanup").enabled,"no second run while busy");
  keys.mouseClick(keys.findChild(notice,"t3DiskSpaceSettings"));check(root.settings===1,"Settings opens the storage page");
  keys.mouseClick(keys.findChild(notice,"t3DiskSpaceWorktrees"));check(bridge.reports===1,"Worktrees requests the report on demand");
  var shown=texts(notice).join("\n");
  check(shown.indexOf("Old work · ot3/old-branch")>=0&&shown.indexOf(Test.T3I18n.tr("{size} in target/",{size:"3.0 GB"}))>=0,"worktree rows with target size: "+shown.replace(/\n/g," | "));
  check(shown.indexOf(Test.T3I18n.tr("Merged"))>=0&&shown.indexOf(Test.T3I18n.tr("Local changes"))>=0,"merged and local change flags");
  keys.mouseClick(keys.findChild(notice,"t3DiskSpaceClose"));check(!notice.visible,"closing hides it");
  bridge.diskNoticeDismissed=false;bridge.diskSpace=({low:false,freeBytes:40*1073741824,thresholdBytes:15*1073741824});check(!notice.visible,"hidden above the threshold");
  console.log("DISK_SPACE_QML_PASS: low disk notice, local cleanup, settings link, on-demand worktree report and dismissal");Qt.quit()
 }}
 Timer { interval:5000;running:true;onTriggered:{console.error("DISK_SPACE_QML_FAIL timeout");Qt.exit(1)} }
}
