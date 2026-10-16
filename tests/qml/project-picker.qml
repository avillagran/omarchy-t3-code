import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PROJECT_PICKER_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var state:({environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"NODE",state:"ready"}]})
  property var folderBrowse:({busy:false,error:"",entries:[],parentPath:""})
  property bool managementBusy:false
  property var managementRequest:null
  property string managementError:""
  property var sent:null
  signal managementFinished(var data)
  function browseFolders(environment,path){folderBrowse={environment:environment,parentPath:path.replace(/\/$/,""),entries:[{name:"Repo",fullPath:"/home/node/Repo"}],busy:false,error:""}}
  function manage(values){sent=values}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:850;implicitHeight:720
  Test.T3AddProject{id:picker;bridge:b}
 }
 TestCase{id:keys;when:false}
 Timer{interval:240;running:true;repeat:true;onTriggered:{
  if(phase===0){picker.begin();phase=1
  }else if(phase===1){check(picker.visible&&picker.step===0,"embedded environment step");picker.chooseEnvironment("node");phase=2
  }else if(phase===2){check(picker.step===1,"source selection before filesystem");keys.mouseClick(keys.findChild(picker,"projectLocalFolder"));phase=3
  }else if(phase===3){check(picker.step===2&&b.folderBrowse.environment==="node","browse selected remote host");picker.browsePath("/home/node/Repo");phase=4
  }else if(phase===4){check(keys.findChild(picker,"projectOpenFolder").enabled,"folder can be confirmed");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="add-project"&&b.sent.environment==="node"&&b.sent.workspaceRoot==="/home/node/Repo","native remote path, no local OS dialog or URL");b.managementRequest=b.sent;picker.back();check(picker.step===2,"pending command locks navigation");b.managementRequest=null;picker.back();check(picker.step===1,"back navigation");picker.openFolders();b.folderBrowse={environment:"local",parentPath:"/other",entries:[],busy:false};check(!keys.findChild(picker,"projectOpenFolder").enabled,"late result from other host cannot create project");b.managementFinished({result:{action:"add-project"}});check(!picker.visible,"success closes only embedded modal");console.log("PROJECT_PICKER_QML_PASS: environment/source/folder steps, native remote paths, pending guards and embedded close");Qt.quit()}
 }}
}
