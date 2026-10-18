import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property var fixtureState: null
 property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_INTERACTION_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections { target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixtureState)Test.T3Backend.state=root.fixtureState} }
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend;var editor=panel.composerView.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One",working:false,activeRunId:"fixture-run"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two",working:false}],projects:[],environments:[],working:0,pending:0};root.fixtureState=b.state;b.selectThread("fixture","one");b.transcript=[{id:"u1",role:"user",text:"Older prompt",createdAt:1},{id:"a1",role:"assistant",text:"Answer",createdAt:2},{id:"u2",role:"user",text:"Newest prompt",createdAt:3}];panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Scratch prompt");editor.forceActiveFocus();editor.cursorPosition=0;keys.keyClick(Qt.Key_Up);phase=2
  }else if(phase===2){check(b.draft.text==="Newest prompt","Up recalls sent prompt without sending");keys.keyClick(Qt.Key_Down);phase=3
  }else if(phase===3){check(b.draft.text==="Scratch prompt","Down restores unsent scratch");keys.keyClick(Qt.Key_A,Qt.MetaModifier);check(editor.selectedText==="Scratch prompt","Super+A selects all without OS conflict");keys.keyClick(Qt.Key_E,Qt.MetaModifier);phase=4
  }else if(phase===4){var sr=keys.findChild(panel,"sidebarReveal");if(sr&&sr.moving)return;check(panel.composerExpanded,"Super+E expands editor");check(editor.height>200,"expanded editor uses panel body");check(!panel.sidebarView.visible,"expanded editor owns the whole panel body");check(b.draft.text==="Scratch prompt","expanded editor shares draft");keys.keyClick(Qt.Key_Escape);phase=5
  }else if(phase===5){check(!panel.composerExpanded&&panel.opened,"Esc returns to chat without closing panel");panel.resizePanel(970,650);keys.keyClick(Qt.Key_Right,Qt.ControlModifier|Qt.AltModifier);phase=6
  }else if(phase===6){check(panel.panelWidth===1010&&b.viewState.panelWidth===1010,"keyboard resize persists dimensions");keys.keyClick(Qt.Key_K,Qt.ControlModifier);phase=7
  }else if(phase===7){check(panel.sidebarView.activeFocus,"Ctrl+K focuses chat list");keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Return);phase=8
  }else if(phase===8){check(b.selectedKey==="fixture/two","sidebar arrows and Enter select a chat");keys.keyClick(Qt.Key_PageUp,Qt.MetaModifier);phase=9
  }else if(phase===9){check(b.selectedKey==="fixture/one","Super+PgUp switches chat");check(b.draft.text==="Scratch prompt","switching chat preserves draft");b.flushDrafts();phase=10
  }else if(phase===10){check(b.draft.saveState==="saved","native disk save acknowledged");b.selectThread("fixture","two");var cache=Object.assign({},b.drafts);delete cache["fixture/one"];b.drafts=cache;b.selectThread("fixture","one");phase=11
  }else if(phase===11){if(b.draft.loading)return;check(b.draft.text==="Scratch prompt","draft recovered from Rust disk store after dropping UI cache");b.edit("");phase=12
  }else if(phase===12){var action=keys.findChild(panel.composerView,"t3PrimaryAction");check(action.actionMode==="stop"&&action.iconName==="stop-filled","empty running composer shows red Stop in primary action slot");b.edit("Queue this later");phase=13
  }else if(phase===13){var action=keys.findChild(panel.composerView,"t3PrimaryAction");check(action.actionMode==="queue"&&action.iconName==="list-plus","typing replaces Stop with original queue SVG");b.edit("");phase=14
  }else if(phase===14){var action=keys.findChild(panel.composerView,"t3PrimaryAction");check(action.actionMode==="stop","clearing input restores Stop");panel.close();console.log("T3_INTERACTION_QML_PASS: native keys, history, expanded editor, resize, sidebar and durable drafts");Qt.quit()}
 }}
}
