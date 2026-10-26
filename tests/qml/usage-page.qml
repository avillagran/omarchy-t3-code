import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("USAGE_PAGE_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;check(!keys.findChild(panel,"t3UsagePage"),"usage is created on demand");panel.triggerNativeShortcut("usage.open");var p=keys.findChild(panel,"t3UsagePage");if(!p){panel.toggleCommandPalette();var pal=keys.findChild(panel,"t3CommandPalette");pal.run(pal.groups[0].items.find(function(i){return i.id==="usage"}))}phase=2
  }else if(phase===2){var p=keys.findChild(panel,"t3UsagePage");if(!p)return;check(p.opened,"usage page opens");check(b.usage.period==="week","week is the default period");b.action({op:"usage-summary",requestId:b.usage.requestId,result:{cost:12.5,tokens:2500000,sessions:3,cacheSavings:1,unpricedRecords:2,pricing:"fresh",days:[{day:"2026-10-19",cost:2.5,tokens:500000},{day:"2026-10-20",cost:10,tokens:2000000}],models:[{provider:"claude",model:"opus",cost:10,tokens:2000000},{provider:"codex",model:"gpt",cost:2.5,tokens:500000}],sources:[]}});phase=3
  }else if(phase===3){var p=keys.findChild(panel,"t3UsagePage");check(p.summary.cost===12.5&&p.money(p.summary.cost)==="$12.50","cost KPI");check(p.count(p.summary.tokens)==="2.5M","tokens KPI");check(keys.findChild(p,"usageModels").count===2,"models listed");p.setPeriod("month");check(b.usage.period==="month"&&p.period==="month","period switches and reloads");keys.findChild(p,"usageMetricTokens").clicked();check(p.metric==="tokens","metric toggle");p.close();var loader=keys.findChild(panel,"usagePageLoader");check(loader.release(),"closed usage controls release");check(!loader.item,"usage unmounted");loader.show();p=keys.findChild(panel,"t3UsagePage");check(p.period==="month"&&p.metric==="tokens","usage choices survive unloading");p.close();check(!keys.findChild(panel,"t3AgentSessions"),"import controls start unloaded");panel.toggleCommandPalette();var pal=keys.findChild(panel,"t3CommandPalette");pal.run(pal.groups[0].items.find(function(i){return i.id==="agent-sessions"}));phase=4
  }else if(phase===4){var a=keys.findChild(panel,"t3AgentSessions");if(!a)return;check(b.agentSessions.action==="scan","import screen scans");b.action({op:"agent-sessions",requestId:b.agentSessions.requestId,result:{candidates:[{title:"proj",path:"/w/proj",sources:["codex"],threadCount:3,alreadyImported:false}]}});check(a.candidates.length===1,"candidates listed");b.agentSessionsAction({action:"import",path:"/w/proj",projectId:null});check(b.agentSessions.busy===b.agentSessions.busy&&b.agentSessions.action==="import","import sends");a.close();panel.close();console.log("USAGE_PAGE_QML_PASS: opens, KPIs, models, period and metric; agent session import");Qt.quit()}
 }}
}
