import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" 1.0 as Test
ShellRoot {
  id: root
  property var backend: null
  readonly property var side: panelLoader.item
  readonly property var panel: side ? side.terminalItem : null
  Component.onCompleted: backend=Test.T3Backend
  property int phase: 0
  property string first: ""
  property string request: ""
  property var fixture: null
  function check(ok,msg){if(!ok){console.error("EMBEDDED_TERMINAL_QML_FAIL "+msg);Qt.exit(1)}}
  TestCase { id: keys; parent: panelLoader; when: false }
  FloatingWindow {
    visible: true; implicitWidth: 900; implicitHeight: 650
    Loader { id: panelLoader; anchors.fill: parent; active: !!root.backend; sourceComponent: Test.T3SidePanel { bridge: root.backend; tab: "terminal"; browserComponent: Component { Rectangle {} } } }
  }
  Connections { target: root.backend; ignoreUnknownSignals: true; function onSnapshotUpdated(){if(root.fixture)root.backend.state=root.fixture} }
  function pane(){return keys.findChild(panel,"t3TerminalPane_"+first)}
  function viewport(){var p=pane();return p&&keys.findChild(p,"t3TerminalViewportLoader").item}
  Timer { interval: 180; repeat: true; running: true; onTriggered: {
    var b=root.backend
    if(phase===0){
      if(!b||!panel||!b.viewStateLoaded)return
      fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[{id:"fixture",state:"ready"}]};b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[],models:[]}};b.selectThread("fixture","one");b.openTerminal();first=b.terminalTab().activeTerminalId;phase=1
    }else if(phase===1){
      if(!viewport())return
      check(viewport().engineRevision==="9f62873bf195e4d8a762d768a1405a5f2f7b1697","renderer uses the exact pinned Ghostty engine");check(pane().requestId.length>0,"native viewport attaches to the selected server session");request=pane().requestId
      b.receiveTerminalSurface({requestId:request,kind:"event",reset:true,event:{type:"snapshot",snapshot:{status:"running",label:"Fixture shell"}},data:"hello\u001b[31m red\u001b[0m\r\n"})
      check(viewport().rowText(0).indexOf("hello red")===0,"actual VT renderer consumes server output");check(b.terminalViews[request].label==="Fixture shell","native shell label is presented")
      viewport().selectRange(0,0,0,5);check(viewport().selectedText==="hello","real terminal text selection is available")
      b.receiveTerminalSurface({requestId:"stale",kind:"event",data:"WRONG"});check(viewport().rowText(0).indexOf("WRONG")<0,"stale scope output is ignored")
      side.tab="browser";phase=10
    }else if(phase===10){
      check(!!viewport()&&pane().requestId===request,"browser tab retains the same terminal frontend and stream");check(viewport().selectedText==="hello","browser tab preserves terminal selection");side.tab="terminal";phase=11
    }else if(phase===11){
      check(pane().requestId===request&&viewport().selectedText==="hello","return to terminal preserves renderer and selection");viewport().feed("\r\n"+"history\r\n".repeat(100));phase=12
    }else if(phase===12){
      var scroll=keys.findChild(viewport(),"t3TerminalScrollback")
      check(scroll&&scroll.visible&&scroll.maximumValue===viewport().scrollbackLines,"real scrollback exposes the native vertical scrollbar range");check(viewport().Accessible.name==="Terminal input"&&scroll.Accessible.name==="Terminal scrollback","terminal accessibility names match the native input and scrollbar");scroll.forceActiveFocus();keys.keyClick(Qt.Key_Home);check(viewport().scrollOffset===viewport().scrollbackLines&&scroll.value===0,"Home moves the scrollbar to oldest history");keys.keyClick(Qt.Key_Down);check(scroll.value===1,"ArrowDown advances one history row");keys.keyClick(Qt.Key_PageDown);check(scroll.value===1+viewport().rows,"PageDown advances one viewport");keys.keyClick(Qt.Key_End);check(viewport().scrollOffset===0&&scroll.value===scroll.maximumValue,"End restores latest terminal output");scroll.value=0;check(viewport().scrollOffset===viewport().scrollbackLines,"accessible value changes update the real terminal");keys.mouseClick(scroll,scroll.width/2,scroll.height-1,Qt.LeftButton,Qt.NoModifier);check(viewport().scrollOffset===0,"track pointer uses native clamped geometry");keys.findChild(panel,"t3TerminalSplit").clicked();phase=2
    }else if(phase===2){
      check(b.terminalTab().terminalIds.length===2,"split creates a distinct native terminal session");check(!!pane(),"existing split keeps its terminal identity")
      keys.findChild(panel,"t3TerminalNew").clicked();check(b.terminalLayout.tabs.length===2,"new terminal adds a tab");phase=3
    }else if(phase===3){
      b.activateTerminalTab(b.terminalLayout.tabs[0].id);phase=4
    }else if(phase===4){
      check(!!viewport(),"returning to a tab reattaches its native view");check(pane().requestId!==request,"reattachment has a fresh request scope")
      var active=b.terminalTab().activeTerminalId,requests=Object.keys(b.terminalViews).filter(function(k){return b.terminalViews[k].terminalId===active})
      check(requests.length===1,"each mounted pane owns exactly one stream")
      b.receiveTerminalSurface({requestId:requests[0],kind:"event",event:{type:"closed"}})
      check(b.terminalTab().terminalIds.length===1,"native closed event removes only its pane")
      console.log("EMBEDDED_TERMINAL_QML_PASS: native VT rendering, labels, selection across tool tabs, accessible scrollback keyboard/pointer/value actions, terminal tabs, splits, stream ownership and stale scopes");Qt.quit()
    }
  } }
  Timer { interval: 12000; running: true; onTriggered: {console.error("EMBEDDED_TERMINAL_QML_FAIL timeout phase="+root.phase);Qt.exit(1)} }
}
