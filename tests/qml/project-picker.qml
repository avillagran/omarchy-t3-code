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
  property var projectSources:({})
  property var sent:null
  function loadProjectSources(environment){projectSources={environment:environment,newProjectsRoot:"/home/node/Projects",cloneTracking:true,providers:[{kind:"github",status:"available",auth:{status:"authenticated"}},{kind:"gitlab",status:"missing",auth:{status:"unknown"}}]}}
  signal managementFinished(var data)
  function browseFolders(environment,path){folderBrowse={environment:environment,parentPath:path.replace(/\/$/,""),entries:[{name:"Repo",fullPath:"/home/node/Repo"},{name:"Other",fullPath:"/home/node/Other"}],busy:false,error:""}}
  function manage(values){sent=values}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:850;implicitHeight:720
  Test.T3AddProject{id:picker;bridge:b}
 }
 TestCase{id:keys;when:false;parent:win.contentItem}
 Timer{interval:240;running:true;repeat:true;onTriggered:{
  if(phase===0){picker.begin();phase=1
  }else if(phase===1){check(picker.visible&&picker.step===0,"embedded environment step");picker.chooseEnvironment("node");phase=2
  }else if(phase===2){check(picker.step===1,"source selection before filesystem");check(picker.sourceButton("new").enabled,"name-only project available from server config");check(picker.sourceButton("github").enabled,"authenticated provider available");check(!picker.sourceButton("gitlab").enabled,"missing provider requires setup");keys.mouseClick(picker.sourceButton("local"));phase=3
  }else if(phase===3){check(picker.step===2&&b.folderBrowse.environment==="node","browse selected remote host");picker.browsePath("/home/node/");var path=keys.findChild(picker,"projectFolderPath");path.forceActiveFocus();path.cursorPosition=path.text.length;phase=31
  }else if(phase===31){keys.keyClick(Qt.Key_R);check(picker.filteredFolders.length===2,"filter matches folder substrings");keys.keyClick(Qt.Key_E);check(picker.filteredFolders.length===1&&picker.filteredFolders[0].name==="Repo","typing filters without Enter or host roundtrip");phase=30
  }else if(phase===30){check(b.folderBrowse.parentPath==="/home/node","filter stays in parent folder");keys.keyClick(Qt.Key_Return);phase=4
  }else if(phase===4){check(keys.findChild(picker,"projectOpenFolder").enabled,"folder can be confirmed");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="add-project"&&b.sent.environment==="node"&&b.sent.workspaceRoot==="/home/node/Repo","native remote path, no local OS dialog or URL");b.managementRequest=b.sent;picker.back();check(picker.step===2,"pending command locks navigation");b.managementRequest=null;picker.back();check(picker.step===1,"back navigation");picker.openFolders();b.folderBrowse={environment:"local",parentPath:"/other",entries:[],busy:false};check(!keys.findChild(picker,"projectOpenFolder").enabled,"late result from other host cannot create project");b.managementFinished({environment:"node",result:{action:"add-project"}});check(!picker.visible,"success closes only embedded modal");picker.begin();picker.chooseEnvironment("node");picker.chooseSource("github");keys.findChild(picker,"projectRepository").text="owner/repo";keys.mouseClick(keys.findChild(picker,"projectChooseDestination"));phase=5
  }else if(phase===5){picker.browsePath("/home/node");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="clone-project"&&b.sent.source==="github"&&b.sent.repository==="owner/repo"&&b.sent.workspaceRoot==="/home/node/repo","provider clone uses selected environment and destination child");picker.back();picker.back();picker.chooseSource("new");keys.findChild(picker,"projectName").text="Fresh";keys.mouseClick(keys.findChild(picker,"projectCreateNew"));check(b.sent.action==="create-project"&&b.sent.name==="Fresh","new repository uses native name-only operation");console.log("PROJECT_PICKER_QML_PASS: environment/source/folder steps, native remote paths, pending guards and embedded close");Qt.quit()}
 }}
}
