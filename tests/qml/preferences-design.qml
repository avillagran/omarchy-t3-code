import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool captured:false;property bool narrowPreview:false
 function check(ok,msg){if(!ok){console.error("PREFERENCES_DESIGN_QML_FAIL "+msg);Qt.exit(1)}}
 function capture(name){captured=false;view.grabToImage(function(r){check(r.saveToFile("/tmp/t3-preferences-"+name+".png"),"save screenshot");root.captured=true})}
 QtObject{id:b
  property string selectedEnvironment:"local"
  property var viewState:({usageExpanded:false,sidebarCompact:true})
  property var state:({environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"remote",name:"NODE",state:"ready"}]})
  property var catalogs:({local:{providers:[{instanceId:"codex",name:"Codex",ready:true},{instanceId:"claude",name:"Claude",ready:false}],models:[{instanceId:"codex",model:"gpt-6.1-sol",displayName:"GPT-6.1-Sol"}],favorites:[{provider:"codex",model:"gpt-6.1-sol"}]}})
  property var settingsResult:({busy:false,error:"",environment:"local",fields:[
   {key:"enableProviderUpdateChecks",label:"Check provider updates",kind:"bool",value:true},
   {key:"continueThreadsAfterServerUpdate",label:"Resume after server updates",kind:"bool",value:true},
   {key:"sidebarAutoSettleAfterDays",label:"Auto-settle after inactivity (days)",kind:"days",value:3,minimum:1,maximum:90},
   {key:"sidebarAutoSettleOnMerge",label:"Settle merged pull requests",kind:"bool",value:true},
   {key:"snoozeLimitedThreads",label:"Snooze rate-limited threads",kind:"bool",value:false},
   {key:"responseStreamingMode",label:"Response streaming",kind:"choice",value:"paragraph",choices:[{id:"turn",name:"Turn"},{id:"paragraph",name:"Paragraph"}]},
   {key:"backgroundActivityProfile",label:"Background activity",kind:"choice",value:"balanced",choices:[{id:"balanced",name:"Balanced"},{id:"performance",name:"Performance"},{id:"battery-saver",name:"Battery saver"}]},
   {key:"branchNamingMode",label:"Branch naming",section:"source-control",kind:"choice",value:"semantic",choices:[{id:"static",name:"Static"},{id:"semantic",name:"Semantic"},{id:"custom",name:"Custom"}]},
   {key:"branchNamePrefix",label:"Branch name prefix",section:"source-control",kind:"text",value:"t3",maximumLength:256,multiline:false},
   {key:"branchNameInstructions",label:"Branch naming instructions",section:"source-control",kind:"text",value:"Use short, descriptive names for each change.",maximumLength:16384,multiline:true},
   {key:"defaultAutoPull",label:"Pull changes automatically",section:"source-control",kind:"bool",value:false},
   {key:"newWorktreesStartFromOrigin",label:"Start new worktrees from origin",section:"source-control",kind:"bool",value:true}
  ]})
  function serverVersion(environment){return ""}
  function requestSettings(environment,key,value){}
  function rememberView(value){viewState=Object.assign({},viewState,value)}
  function toggleModelFavoriteIn(environment,provider,model){}
 }
 FloatingWindow {id:win;visible:true;implicitWidth:1180;implicitHeight:780
  Test.T3Preferences{id:view;width:root.narrowPreview?720:win.width;height:root.narrowPreview?700:win.height;bridge:b;shortcuts:"Super+Enter · Send\nSuper+E · Edit"}
 }
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(phase===0){check(keys.findChild(view,"preference_enableProviderUpdateChecks")!==null,"grouped native field renders");capture("general");phase=1
  }else if(phase===1&&captured){view.page="source-control";phase=2
  }else if(phase===2){capture("source-control");phase=3
  }else if(phase===3&&captured){root.narrowPreview=true;phase=4
  }else if(phase===4){var field=keys.findChild(view,"preferenceText_branchNamePrefix");check(view.compact&&view.width===720,"narrow viewport exercised independently of window tiling");check(field.width>200,"narrow text field uses available width");check(field.mapToItem(view,field.width,0).x<=view.width,"narrow field stays inside window");capture("narrow");phase=5
  }else if(phase===5&&captured){view.query="updates";phase=6
  }else if(phase===6){check(view.groups.length===1&&view.groups[0].title==="Behavior","search finds update fields in the native Behavior section");view.query="does-not-exist";check(!view.groups.length,"search empty state");view.query="";view.page="appearance";phase=7
  }else if(phase===7){check(keys.findChild(view,"preferenceUsage").visible,"appearance switch visible");capture("appearance");phase=8
  }else if(phase===8&&captured){console.log("PREFERENCES_DESIGN_QML_PASS: grouped cards, cross-category search, responsive fields and screenshots");Qt.quit()}
 }}
}
