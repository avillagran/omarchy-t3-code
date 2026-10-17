import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeKeybindings.js" as Native
import "UnderTest/ui/js/QtKeybindings.js" as Adapt
ShellRoot{
 id:root
 function check(ok,msg){if(!ok){console.error("KEYBINDINGS_EDITOR_QML_FAIL "+msg);Qt.exit(1)}}
 FloatingWindow{id:win;visible:true;implicitWidth:900;implicitHeight:800;Test.T3WhenEditor{id:editor;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:30;expression:"composerFocus && !terminalFocus"}}
 TestCase{id:keys;when:false}
 Timer{interval:180;running:true;onTriggered:{
  var k=Native.nativeKeys;check(editor.valid&&editor.node.type==="and","native condition tree loaded");check(keys.findChild(editor,"whenVariable_1"),"recursive condition selector");keys.findChild(editor,"whenRootGroup").clicked();check(editor.node.type==="and"&&editor.node.right.type==="or","original group builder adds opposite nested operator");editor.update({type:"identifier",name:"futureContext"});check(editor.valid&&editor.unknown[0]==="futureContext","unknown native variable preserved");var input=keys.findChild(editor,"whenExpressionInput");input.text="composerFocus && (";input.textEdited();check(!editor.valid&&editor.node&&editor.node.name==="futureContext","invalid expression keeps the last valid native tree while preserving the editable draft");editor.update(undefined);check(editor.valid&&!editor.node,"empty condition");keys.findChild(editor,"whenRootCondition").clicked();check(editor.node.name===k.DEFAULT_WHEN_VARIABLE,"native root default condition");
  var event={key:Qt.Key_Exclam,nativeScanCode:10,text:"!",modifiers:Qt.ControlModifier|Qt.ShiftModifier,isAutoRepeat:false};check(Adapt.capture(event)==="mod+shift+1","Qt XKB scan code preserves native physical Digit1 capture");event={key:Qt.Key_Backtab,nativeScanCode:23,text:"",modifiers:Qt.ShiftModifier};check(Adapt.capture(event)==="shift+tab","Qt Backtab adapter");check(Adapt.sequence(k.parseKeybindingShortcut("mod+alt+arrowleft"))==="Ctrl+Alt+Left","original formatter becomes Qt sequence");check(Adapt.when(k.parseKeybindingWhenExpression("composerFocus && !terminalFocus"),{composerFocus:true,terminalFocus:false}),"native condition AST matches Qt context");console.log("KEYBINDINGS_EDITOR_QML_PASS: real recursive native condition editor, text draft validity, root/group helpers and physical Qt key adaptation");Qt.quit()
 }}
}
