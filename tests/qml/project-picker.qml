import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PROJECT_PICKER_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var state:({projects:[{id:"same",environmentId:"local",title:"Existing app",workspaceRoot:"/home/local/app"},{id:"same",environmentId:"node",title:"Existing app",workspaceRoot:"/srv/app"},{id:"offline",environmentId:"gone",title:"Disconnected",workspaceRoot:"/offline"}],environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"NODE",state:"ready"}]})
  property var folderBrowse:({busy:false,error:"",entries:[],parentPath:""})
  property bool managementBusy:false
  property var managementRequest:null
  property string managementError:""
  property var projectSources:({})
  property var pendingProjectOpen:null
  property var sent:null
  function loadProjectSources(environment){projectSources={environment:environment,newProjectsRoot:"/home/node/Projects",cloneTracking:true,providers:[{kind:"github",status:"available",auth:{status:"authenticated"}},{kind:"gitlab",status:"missing",auth:{status:"unknown"}}]}}
  signal managementFinished(var data)
  signal projectOpened(string environment,string projectId,string error)
  function openProject(environment,projectId,createThread){sent={action:"open-project",environment:environment,projectId:projectId,createThread:createThread===true}}
  function browseFolders(environment,path){folderBrowse={environment:environment,parentPath:path.replace(/\/$/,""),entries:[{name:"Repo",fullPath:"/home/node/Repo"},{name:"Other",fullPath:"/home/node/Other"}],busy:false,error:""}}
  function manage(values){sent=values}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:850;implicitHeight:720
  Test.T3AddProject{id:picker;bridge:b}
 }
 TestCase{id:keys;when:false;parent:win.contentItem}
 Timer{interval:240;running:true;repeat:true;onTriggered:{
  if(phase===0){picker.begin();phase=10
  }else if(phase===10){check(picker.existingProjects.length===2,"existing projects shown immediately across ready hosts only");keys.findChild(picker,"projectSearch").text="NODE /srv";phase=11
  }else if(phase===11){check(picker.existingProjects.length===1&&picker.existingProjects[0].environmentId==="node","project search matches environment and path tokens");keys.mouseClick(picker.existingButton("node","same"));check(b.sent.action==="open-project"&&b.sent.environment==="node"&&b.sent.projectId==="same"&&b.sent.createThread,"existing row starts a new chat in scoped project without registering");check(picker.visible&&picker.openingProject,"existing project waits for navigation acknowledgement");b.managementError="model_unavailable";b.projectOpened("node","same","model_unavailable");check(picker.visible&&!picker.openingProject,"existing open error allows retry");keys.findChild(picker,"projectSearch").text="";phase=12
  }else if(phase===12){keys.mouseClick(picker.existingButton("node","same"));b.projectOpened("local","same","");check(picker.visible,"other environment acknowledgement cannot close picker");b.projectOpened("node","same","");check(!picker.visible,"existing project success closes picker");picker.begin();check(keys.findChild(picker,"projectSearch").text==="","fresh opening resets search");picker.chooseEnvironment("local");check(picker.existingProjects.length===1&&picker.existingProjects[0].environmentId==="local","environment step scopes existing projects");picker.close();picker.begin();phase=1
  }else if(phase===1){check(picker.visible&&picker.step===0,"embedded environment step");picker.chooseEnvironment("node");phase=2
  }else if(phase===2){check(picker.step===1,"source selection before filesystem");check(picker.sourceButton("new").enabled,"name-only project available from server config");check(picker.sourceButton("github").enabled,"authenticated provider available");check(!picker.sourceButton("gitlab").enabled,"missing provider requires setup");picker.contentItem.grabToImage(function(r){r.saveToFile("/tmp/ot3-project-source-preview.png")});phase=20
  }else if(phase===20){keys.mouseClick(picker.sourceButton("local"));phase=3
  }else if(phase===3){check(picker.step===2&&b.folderBrowse.environment==="node","browse selected remote host");picker.browsePath("/home/node/");var path=keys.findChild(picker,"projectFolderPath");path.forceActiveFocus();path.cursorPosition=path.text.length;phase=31
  }else if(phase===31){picker.contentItem.grabToImage(function(r){r.saveToFile("/tmp/ot3-project-folders-preview.png")});keys.keyClick(Qt.Key_R);check(picker.filteredFolders.length===2,"filter matches folder substrings");keys.keyClick(Qt.Key_E);check(picker.filteredFolders.length===1&&picker.filteredFolders[0].name==="Repo","typing filters without Enter or host roundtrip");phase=30
  }else if(phase===30){check(b.folderBrowse.parentPath==="/home/node","filter stays in parent folder");keys.keyClick(Qt.Key_Return);phase=4
  }else if(phase===4){check(keys.findChild(picker,"projectOpenFolder").enabled,"folder can be confirmed");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="add-project"&&b.sent.environment==="node"&&b.sent.workspaceRoot==="/home/node/Repo","native remote path, no local OS dialog or URL");b.managementRequest=b.sent;picker.back();check(picker.step===2,"pending command locks navigation");b.managementRequest=null;picker.back();check(picker.step===1,"back navigation");picker.openFolders();b.folderBrowse={environment:"local",parentPath:"/other",entries:[],busy:false};check(!keys.findChild(picker,"projectOpenFolder").enabled,"late result from other host cannot create project");b.managementFinished({environment:"node",result:{action:"add-project",projectId:"registered"}});check(picker.visible&&picker.openingProject,"registration waits for visible first chat");b.projectOpened("node","registered","model_unavailable");check(picker.visible&&!picker.openingProject,"chat failure stays visible without repeating registration");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="open-project"&&b.sent.projectId==="registered"&&b.sent.createThread,"retry creates chat in saved project without repeating registration");b.projectOpened("node","registered","");check(!picker.visible,"first chat success closes embedded modal");picker.begin();picker.chooseEnvironment("node");picker.chooseSource("github");keys.findChild(picker,"projectRepository").text="owner/repo";keys.mouseClick(keys.findChild(picker,"projectChooseDestination"));phase=5
  }else if(phase===5){picker.browsePath("/home/node");keys.mouseClick(keys.findChild(picker,"projectOpenFolder"));check(b.sent.action==="clone-project"&&b.sent.source==="github"&&b.sent.repository==="owner/repo"&&b.sent.workspaceRoot==="/home/node/repo","provider clone uses selected environment and destination child");picker.back();picker.back();picker.chooseSource("new");keys.findChild(picker,"projectName").text="Fresh";keys.mouseClick(keys.findChild(picker,"projectCreateNew"));check(b.sent.action==="create-project"&&b.sent.name==="Fresh","new repository uses native name-only operation");console.log("PROJECT_PICKER_QML_PASS: environment/source/folder steps, native remote paths, pending guards and embedded close");Qt.quit()}
 }}
}
