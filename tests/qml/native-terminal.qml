import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" 1.0 as Test
ShellRoot {
  id: root
  property var backend: null
  readonly property var panel: panelLoader.item
  property int phase: 0
  property var fixture: null
  property string oldRequest: ""
  property string oldTerminal: ""
  Component.onCompleted: backend=Test.T3Backend
  function check(ok,msg){if(!ok){console.error("NATIVE_TERMINAL_QML_FAIL "+msg);Qt.exit(1)}}
  Loader { id: panelLoader; active: !!root.backend; sourceComponent: Test.T3Panel { bridge: root.backend } }
  TestCase { id: keys; when: false }
  Connections { target: root.backend; ignoreUnknownSignals: true; function onSnapshotUpdated(){if(root.fixture)root.backend.state=root.fixture} }
  Timer { interval: 200; running: true; repeat: true; onTriggered: {
    var b=root.backend
    if(phase===0){
      if(!b||!panel||!b.viewStateLoaded)return
      b.rememberView({windowMode:"floating",windowWidth:1400,windowHeight:900});fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two"}],projects:[],environments:[{id:"fixture",state:"ready"}]};b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[],models:[]}};b.selectThread("fixture","one");panel.open();phase=1
    }else if(phase===1){
      if(b.draft.loading)return
      b.edit("Preserve this conversation draft");var button=keys.findChild(panel,"t3OpenTerminal");check(button&&button.enabled,"header terminal action is available");button.clicked();check(panel.opened&&panel.sideTab==="terminal"&&panel.sideShown,"header opens embedded terminal inside the visible app");oldTerminal=b.terminalTab().activeTerminalId;phase=2
    }else if(phase===2){
      var pane=keys.findChild(panel.sidePanelItem.terminalItem,"t3TerminalPane_"+oldTerminal);if(!pane||!pane.requestId)return;oldRequest=pane.requestId
      var viewport=keys.findChild(pane,"t3TerminalViewportLoader").item;if(!viewport)return
      check(viewport.engineRevision==="9f62873bf195e4d8a762d768a1405a5f2f7b1697","header route loads the exact pinned Ghostty engine")
      b.receiveTerminalSurface({requestId:oldRequest,kind:"event",reset:true,event:{type:"snapshot",snapshot:{status:"running"}},data:"server shell\r\n"});check(viewport.rowText(0).indexOf("server shell")===0,"actual header route renders native PTY output")
      panel.nativeShortcutController.triggered("terminal.toggle");check(!panel.sideShown&&panel.opened,"terminal toggle hides the side panel and keeps the app visible");panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_J,Qt.ControlModifier);phase=3
    }else if(phase===3){
      check(panel.sideShown&&panel.sideTab==="terminal","CtrlJ reopens the embedded terminal");check(b.draft.text==="Preserve this conversation draft","terminal operations preserve the composer draft");check(b.terminalLayout.tabs.length===1,"reopening reuses its native terminal id")
      b.selectThread("fixture","two");b.openTerminal();phase=4
    }else if(phase===4){
      check(b.terminalTab()&&b.terminalTab().activeTerminalId!==oldTerminal,"each conversation has its own native terminal layout");check(!b.terminalViews[oldRequest],"switching conversation detaches its old view")
      b.receiveTerminalSurface({requestId:oldRequest,kind:"event",event:{type:"closed"}});check(b.terminalLayout.tabs.length===1,"late old-scope close cannot remove the new terminal")
      b.selectThread("fixture","one");phase=5
    }else if(phase===5){
      check(b.terminalTab().activeTerminalId===oldTerminal,"returning restores that conversation's terminal id");check(b.drafts["fixture/one"].text==="Preserve this conversation draft","conversation draft remains saved")
      var pane=keys.findChild(panel.sidePanelItem.terminalItem,"t3TerminalPane_"+oldTerminal);if(!pane)return
      var viewport=keys.findChild(pane,"t3TerminalViewportLoader").item;if(!viewport)return;viewport.forceActiveFocus();phase=6
    }else if(phase===6){
      check(panel.nativeShortcutContext.terminalFocus,"native shortcuts identify focus inside the terminal");keys.keyClick(Qt.Key_D,Qt.ControlModifier);phase=7
    }else if(phase===7){
      check(b.terminalTab().terminalIds.length===2,"CtrlD splits the embedded terminal instead of typing EOF")
      b.catalogs={fixture:{canWrite:false,providers:[],models:[]}};check(!b.canOpenTerminal&&!keys.findChild(panel.sidePanelItem.terminalItem,"t3TerminalNew").enabled,"read-only environment disables new shells")
      console.log("NATIVE_TERMINAL_QML_PASS: actual header route, VT output, CtrlJ toggle, terminal focus/CtrlD split, retained app/draft/session, per-thread layouts, stale events and read-only controls");Qt.quit()
    }
  } }
  Timer { interval: 14000; running: true; onTriggered:{console.error("NATIVE_TERMINAL_QML_FAIL timeout phase="+phase);Qt.exit(1)} }
}
