import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int submenuRuns:0
 function check(ok,msg){if(!ok){console.error("COMMAND_PALETTE_QML_FAIL "+msg);Qt.exit(1)}}
 function thread(id,title,project,projectId){var stamp="2026-11-08T00:00:00.000Z";return {id:id,key:"fixture/"+id,environmentId:"fixture",title:title,project:project,projectId:projectId,updatedAt:1,searchShell:{id:id,environmentId:"fixture",title:title,branch:null,createdAt:stamp,updatedAt:stamp,latestUserAuthoredMessageAt:stamp,opens:null}}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[thread("one","One","Alpha","p"),thread("two","Refactor parser","Beta","q")],projects:[{id:"p",key:"fixture/p",title:"Alpha",environmentId:"fixture"},{id:"q",key:"fixture/q",title:"Beta",environmentId:"fixture"}],environments:[{id:"fixture",name:"Private",state:"ready"}]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;panel.toggleCommandPalette();phase=2
  }else if(phase===2){var p=keys.findChild(panel,"t3CommandPalette");if(!p||!p.opened||p.rankBusy)return;check(p&&p.opened,"palette opens");check(p.rows.some(function(r){return r.header&&r.label==="Actions"}),"actions group");check(p.rows.some(function(r){return !r.header&&r.item.id==="thread:fixture/two"}),"recent threads exclude the open one and list others");check(!p.rows.some(function(r){return !r.header&&r.item.id==="thread:fixture/one"}),"open thread is not listed")
   check(!p.rows.some(function(r){return r.header&&(r.label==="Projects"||r.label==="Settings")}),"search inventories do not appear in the empty root view");keys.findChild(p,"t3CommandPaletteSearch").text="refactor pars";phase=3
  }else if(phase===3){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.runnable.length===1&&p.runnable[0].item.id==="thread:fixture/two","multi-word search finds the thread: "+p.runnable.length);p.run(p.runnable[0].item);phase=4
  }else if(phase===4){check(b.selectedKey==="fixture/two","choosing a thread opens it");check(!keys.findChild(panel,"t3CommandPalette").opened,"palette closes after running");panel.toggleCommandPalette();phase=5
  }else if(phase===5){var p=keys.findChild(panel,"t3CommandPalette");keys.findChild(p,"t3CommandPaletteSearch").text="storage";phase=6
  }else if(phase===6){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.runnable.length>=1&&p.runnable[0].item.id==="settings:storage","settings pages are searchable");p.run(p.runnable[0].item);phase=7
  }else if(phase===7){check(panel.preferencesOpen,"settings page opens");panel.preferencesOpen=false;panel.toggleCommandPalette();keys.findChild(panel,"t3CommandPaletteSearch").text="tokenizer bug";phase=8
  }else if(phase===8){if(b.threadSearch.query!=="tokenizer bug"||!b.threadSearch.requestId)return;b.action({op:"thread-search",requestId:b.threadSearch.requestId,result:{query:"tokenizer bug",matches:[{threadId:"two",projectId:"q",environmentId:"fixture",source:"assistant",snippet:"…the tokenizer bug was in…",messageCreatedAt:null}]}});phase=9
  }else if(phase===9){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.runnable.length===1&&p.runnable[0].item.id==="match:fixture/two"&&p.runnable[0].item.title==="Refactor parser","server message matches list their thread: "+JSON.stringify(p.runnable.map(function(r){return r.item.id})));p.close();check(b.threadSearch.query==="","closing clears the search");panel.toggleCommandPalette();phase=10
  }else if(phase===10){var p=keys.findChild(panel,"t3CommandPalette");if(!p.opened)return
   p.groups=[{value:"actions",label:"Actions",items:[
    {id:"theme-menu",kind:"submenu",title:"Change theme",groups:[{value:"themes",label:"Themes",items:[
     {id:"theme:dracula",title:"Dracula",keepOpen:true,run:function(){root.submenuRuns++}},
     {id:"theme:solarized",title:"Solarized",run:function(){root.submenuRuns++}},
     {id:"theme:disabled",title:"Unavailable",disabled:true,run:function(){root.submenuRuns+=100}}
    ]}]},
    {id:"root-action",title:"Unrelated root action",run:function(){root.submenuRuns+=10}}
   ]}]
   keys.findChild(p,"t3CommandPaletteSearch").text="change theme";phase=11
  }else if(phase===11){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.runnable.length===1&&p.runnable[0].item.id==="theme-menu","naming a submenu keeps its parent row");p.run(p.runnable[0].item);phase=12
  }else if(phase===12){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.opened&&p.isInSubmenu&&p.query==="","submenu opens without closing the palette");check(p.rows.some(function(r){return !r.header&&r.item.id==="theme:disabled"})&&p.runnable.length===2,"disabled options render without taking a keyboard index");keys.findChild(p,"t3CommandPaletteSearch").text="unrelated root";phase=13
  }else if(phase===13){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.isInSubmenu&&p.runnable.length===1&&p.runnable[0].item.id==="root-action","empty submenu search falls back through the original all-options matcher");keys.findChild(p,"t3CommandPaletteSearch").text="";phase=14
  }else if(phase===14){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;keys.findChild(p,"t3CommandPaletteSearch").forceActiveFocus();keys.keyClick(Qt.Key_Backspace);phase=15
  }else if(phase===15){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(!p.isInSubmenu&&p.query==="change theme","Backspace restores the original parent query");keys.findChild(p,"t3CommandPaletteSearch").text="dracula";phase=16
  }else if(phase===16){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.runnable.length===1&&p.runnable[0].item.id==="theme:dracula"&&p.titleText(p.runnable[0].item).indexOf("Change theme:")>=0,"original nested suggestions include their parent title");p.run(p.runnable[0].item);phase=17
  }else if(phase===17){var p=keys.findChild(panel,"t3CommandPalette");check(p.opened&&submenuRuns===1,"keepOpen executes the child once without closing");p.run(p.originalItem("theme:disabled",false));check(submenuRuns===1,"disabled actions never execute");keys.findChild(p,"t3CommandPaletteSearch").text="x".repeat(1000);phase=18
  }else if(phase===18){var p=keys.findChild(panel,"t3CommandPalette");if(p.rankBusy)return;check(p.query.length===1000,"pastes are not truncated at the former 256-character UI limit");check(b.threadSearch.query.length===200,"server queries use the original 200-unit normalization");p.close();check(p.navigation.views.length===0,"closing clears the navigation stack");panel.close();console.log("COMMAND_PALETTE_QML_PASS: actions, recent threads, original search, settings, messages, submenus, fallback, parent query, nested suggestions, disabled actions, keepOpen and pasted query");Qt.quit()}
 }}
}
