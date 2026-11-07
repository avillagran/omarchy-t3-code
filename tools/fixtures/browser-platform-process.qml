import QtQuick
import Quickshell
import Quickshell.Io
ShellRoot {
 id:root
 property int phase:0
 property string binding:""
 function check(value){if(!value){console.error("BROWSER_GUI_PROCESS_FAIL");Qt.exit(1);throw Error("GUI process admission")}}
 Loader{id:platform;active:true;source:"T3BrowserClipboardPlatform.qml"}
 Process {
  id:worker
  command:["/fixture/worker-probe"]
  environment:platform.item?({O3_GUI_CLIPBOARD_ENDPOINT:platform.item.endpoint}):({})
  stdinEnabled:true
  running:platform.status===Loader.Ready&&platform.item.available
  onStarted:{root.binding=platform.item.bindWorker(worker.processId);root.check(root.binding.length>0)}
  stdout:SplitParser{onRead:function(line){
   if(root.phase===0){root.check(line==="admitted"&&platform.item.workerAdmitted);root.phase=1;worker.write("alive\n")}
   else if(root.phase===1){root.check(line==="true");platform.item.retireWorker("stale-binding");root.check(platform.item.workerAdmitted);platform.item.retireWorker(root.binding);root.check(!platform.item.workerAdmitted);root.phase=2;worker.write("alive\n")}
   else if(root.phase===2){root.check(line==="false");root.phase=3;worker.write("quit\n")}
   else root.check(false)
  }}
  onExited:{root.check(root.phase===3&&!platform.item.workerAdmitted);console.log("BROWSER_GUI_PROCESS_PASS actual-Quickshell-Process loader parent-credentials pid binding retirement");Qt.quit()}
 }
 Timer{interval:8000;running:true;onTriggered:root.check(false)}
}
