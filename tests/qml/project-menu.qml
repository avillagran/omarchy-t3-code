import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/ProjectModel.js" as Projects
import "UnderTest/ui/js/SidebarModel.js" as Sidebar
ShellRoot {
 id:root;property bool done:false
 Timer{interval:400;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 QtObject{
  id:fixture
  property var state:({projects:[{id:"p",key:"remote/p",environmentId:"remote",title:"Project",workspaceRoot:"/work/repo",repositoryIdentity:{canonicalKey:"github.com/example/repo",rootPath:"/work/repo"}}],threads:[{id:"one",key:"remote/one",projectKey:"remote/p"}]})
  property var catalogs:({remote:{canWrite:true,models:[{instanceId:"codex",model:"test"}],providers:[],favorites:[],favoritesReady:true}})
  property var viewState:({})
  property var drafts:({})
  property var favoritePending:({})
  property string favoriteError:""
  property bool managementBusy:false
  property var managementRequest:null
  property var calls:[]
  property string selectedKey:"other/one"
  property int selected:0
  signal managementFinished(var data)
  function ensureCatalog(env){}
  function rememberView(v){viewState=Object.assign({},viewState,v)}
  function manage(v){managementRequest=Object.assign({requestId:"fixture-request"},v);calls=calls.concat([managementRequest])}
  function selectThread(env,id){selected++}
  function toggleModelFavoriteIn(env,provider,model){}
 }
 Window{
  id:win;visible:true;width:900;height:700;color:Color.popups.background
  Test.T3ThreadRow{id:row;width:320;thread:({id:"one",key:"remote/one",projectKey:"remote/p",project:"Project",environmentId:"remote",title:"Thread"});bridge:fixture;onProjectContextRequested:function(item,x,y){menu.showProject(thread.projectKey,item,x,y)}}
  Test.T3ProjectMenu{id:menu;bridge:fixture}
  TestCase{id:checks;when:false
   function runChecks(){
    var target=findChild(row,"projectNameContext");verify(!!target);mouseClick(target,target.width/2,target.height/2,Qt.RightButton);var ctx=findChild(menu,"t3ProjectContextMenu");verify(ctx.visible,"project context menu opens");compare(fixture.selected,0,"right click does not select another chat");compare(ctx.count,6,"native five options and separator");compare(menu.project.environmentId,"remote");
    ctx.close();menu.edit("rename");var input=findChild(menu,"projectNameInput");input.text="Changed";menu.save();compare(fixture.calls[0].action,"project-rename");compare(fixture.calls[0].environment,"remote");compare(fixture.calls[0].projectId,"p");compare(fixture.calls[0].title,"Changed");fixture.managementRequest=null;fixture.managementFinished({requestId:"fixture-request"});
    menu.edit("grouping");findChild(menu,"projectGroupingSelect").currentIndex=3;menu.save();compare(fixture.viewState.sidebarProjectGroupingOverrides["remote:/work/repo"],"separate");
    var second=Object.assign({},fixture.state.projects[0],{id:"p2",key:"local/p2",environmentId:"local"});compare(Projects.groups([fixture.state.projects[0],second],{}).length,1);compare(Projects.groups([fixture.state.projects[0],second],fixture.viewState).length,2);compare(Projects.mode(second,{nativeProjectGrouping:{mode:"repository_path",overrides:{"local:/work/repo":"separate"}}}),"separate");compare(Projects.mode(second,{nativeProjectGrouping:{mode:"repository_path",overrides:{"local:/work/repo":"separate"}},sidebarProjectGroupingOverrides:{"local:/work/repo":"inherit"}}),"repository_path");verify(Sidebar.matches({projectKey:"local/p2"},"all",["remote/p","local/p2"]));
    menu.edit("settings");var defaults=findChild(menu,"projectDefaultModel");defaults.bridge.chooseModel(0);compare(menu.modelSelection.model,"test");compare(Object.keys(fixture.drafts).length,0,"project model selection does not touch the chat draft");var modelPicker=findChild(menu,"projectDefaultEnvironment");modelPicker.currentIndex=2;menu.save();compare(fixture.calls[1].action,"project-settings");compare(fixture.calls[1].patch.defaultThreadEnvMode,"worktree");fixture.managementRequest=null;fixture.managementFinished({requestId:"fixture-request"});
    menu.edit("remove");var save=findChild(menu,"projectSave");verify(!save.enabled,"nonempty project requires explicit thread-delete acknowledgement");compare(fixture.calls.length,2,"opening removal performs no mutation");findChild(menu,"projectDeleteThreads").checked=true;menu.save();compare(fixture.calls[2].action,"project-remove");verify(fixture.calls[2].confirmed);verify(fixture.calls[2].force);fixture.managementRequest=null;fixture.managementFinished({requestId:"fixture-request"});
    console.log("PROJECT_MENU_QML_PASS: native actions, right-click scope, no selection, grouping, defaults and deletion confirmation");root.done=true
   }
  }
 }
}
