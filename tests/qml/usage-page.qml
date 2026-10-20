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
  }else if(phase===1){if(b.draft.loading)return;panel.triggerNativeShortcut("usage.open");var p=keys.findChild(panel,"t3UsagePage");if(!p.opened){panel.toggleCommandPalette();var pal=keys.findChild(panel,"t3CommandPalette");pal.run(pal.groups[0].items.find(function(i){return i.id==="usage"}))}phase=2
  }else if(phase===2){var p=keys.findChild(panel,"t3UsagePage");check(p.opened,"usage page opens");check(b.usage.period==="week","week is the default period");b.action({op:"usage-summary",requestId:b.usage.requestId,result:{cost:12.5,tokens:2500000,sessions:3,cacheSavings:1,unpricedRecords:2,pricing:"fresh",days:[{day:"2026-10-19",cost:2.5,tokens:500000},{day:"2026-10-20",cost:10,tokens:2000000}],models:[{provider:"claude",model:"opus",cost:10,tokens:2000000},{provider:"codex",model:"gpt",cost:2.5,tokens:500000}],sources:[]}});phase=3
  }else if(phase===3){var p=keys.findChild(panel,"t3UsagePage");check(p.summary.cost===12.5&&p.money(p.summary.cost)==="$12.50","cost KPI");check(p.count(p.summary.tokens)==="2.5M","tokens KPI");check(keys.findChild(p,"usageModels").count===2,"models listed");p.setPeriod("month");check(b.usage.period==="month"&&p.period==="month","period switches and reloads");keys.findChild(p,"usageMetricTokens").clicked();check(p.metric==="tokens","metric toggle");p.close();panel.close();console.log("USAGE_PAGE_QML_PASS: opens, KPIs, models, period and metric");Qt.quit()}
 }}
}
