import QtQuick
import Quickshell
import QtTest
import "UnderTest" as Test
ShellRoot{
 id:root;property int phase:0;property real compactHeight:0
 function check(ok,msg){if(!ok){console.error("T3_EDITOR_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function write(value){var b=Test.T3Backend;b.edit(value);Qt.callLater(function(){panel.composerView.inputField.cursorPosition=value.length})}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend;var c=panel.composerView;var e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}]};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;e.forceActiveFocus();write("- first");phase=2
  }else if(phase===2){keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){check(b.draft.text==="- first\n- "&&!b.draft.attempt,"Enter continues bullet without sending");keys.keyClick(Qt.Key_Return);phase=4
  }else if(phase===4){check(b.draft.text==="- first\n","empty bullet exits list");write("  1. one");phase=5
  }else if(phase===5){keys.keyClick(Qt.Key_Return,Qt.ShiftModifier);phase=6
  }else if(phase===6){check(b.draft.text==="  1. one\n  2. ","numbered list increments with indentation");write("- [x] done");phase=7
  }else if(phase===7){keys.keyClick(Qt.Key_Return);phase=8
  }else if(phase===8){check(b.draft.text==="- [x] done\n- [ ] ","checkbox list continues unchecked");write("A long draft must not stretch the compact editor.\n".repeat(60));phase=9
  }else if(phase===9){compactHeight=c.height;check(compactHeight<350,"long input remains compact");keys.keyClick(Qt.Key_E,Qt.MetaModifier);phase=10
  }else if(phase===10){check(panel.composerExpanded&&c.height>compactHeight+100,"expanded editor fills body");keys.keyClick(Qt.Key_Escape);phase=11
  }else if(phase===11){check(!panel.composerExpanded&&panel.opened,"Esc returns to chat");check(Math.abs(c.height-compactHeight)<4,"compact geometry restored after long input");check(panel.conversationView.height>150&&panel.sidebarView.visible,"chat and sidebar retain space");keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/omarchy-t3-editor-return.png")});phase=12
  }else if(phase===12){panel.expandComposer();phase=120
  }else if(phase===120){panel.expandComposer();phase=13
  }else if(phase===13){check(Math.abs(c.height-compactHeight)<4,"repeated expansion has no height drift");check(b.draft.text.endsWith("compact editor.\n"),"draft preserved");var viewport=keys.findChild(c,"t3EditorViewport"),footer=keys.findChild(c,"t3ExpandEditor");check(footer.mapToItem(c,0,footer.height).y<=c.height+1,"footer stays inside composer");var caret=e.mapToItem(viewport,e.cursorRectangle.x,e.cursorRectangle.y);check(caret.y>=-1&&caret.y+e.cursorRectangle.height<=viewport.height+2,"caret stays inside clipped viewport");e.selectAll();e.remove(e.selectionStart,e.selectionEnd);keys.keyClick(Qt.Key_1);keys.keyClick(Qt.Key_Period);keys.keyClick(Qt.Key_Space);keys.keyClick(Qt.Key_A);phase=14
  }else if(phase===14){keys.keyClick(Qt.Key_Return);phase=15
  }else if(phase===15){check(b.draft.text==="1. a\n2. ","native typed list continues");panel.close();console.log("T3_EDITOR_QML_PASS: bullet/number/checklist Enter, empty exit, long draft, expand/back geometry and no prompt submission");Qt.quit()}
 }}
}
