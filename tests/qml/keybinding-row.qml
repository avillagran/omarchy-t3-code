import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeKeybindings.js" as Native
ShellRoot{
 id:root;property int phase:0;property var saved:null;property var removed:null;property bool captured:false
 readonly property var defaults:Native.nativeKeys.buildKeybindingRows(Native.nativeKeys.mergeWithDefaultKeybindings([]),"")
 function check(ok,msg){if(!ok){console.error("KEYBINDING_ROW_QML_FAIL "+msg);Qt.exit(1)}}
 FloatingWindow{id:win;visible:true;implicitWidth:1000;implicitHeight:740
  Rectangle{anchors.fill:parent;color:"#101418"}
  Test.T3KeybindingRow{id:row;width:parent.width-60;x:30;y:40;row:root.defaults.find(function(r){return r.command==="composer.sendAndNewThread"});allRows:root.defaults;writable:true;onSaveRequested:function(input){root.saved=input};onRemoveRequested:function(input){root.removed=input};onResetRequested:function(input){root.saved=input}}
 }
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:160;running:true;repeat:true;onTriggered:{
  var command="composer.sendAndNewThread",pill=keys.findChild(row,"keybindingPill_"+command),input=keys.findChild(row,"keybindingCapture_"+command)
  if(phase===0){check(row.valid&&!row.dirty,"native default row");pill.clicked();phase=1}
  else if(phase===1){check(input.activeFocus&&row.recording,"shortcut pill focuses capture input");keys.keyClick(Qt.Key_Escape);check(!row.recording&&!row.dirty,"Escape cancels original recording draft");pill.clicked();phase=2}
  else if(phase===2){keys.keyClick(Qt.Key_K,Qt.ControlModifier|Qt.ShiftModifier);check(row.keyDraft==="mod+shift+k"&&row.dirty&&!row.recording&&row.captureActive,"real Qt key capture uses native recorder");keys.findChild(row,"keybindingSave_"+command).clicked();check(saved&&saved.key==="mod+shift+k"&&saved.replace.command===command&&saved.replace.key===row.row.key,"save has exact original replacement target");row.whenDraft="composerFocus && (";check(!row.valid,"invalid condition disables save");saved=null;row.commit();check(!saved,"programmatic save cannot bypass invalid state");row.resetDraft();row.row=Native.nativeKeys.buildKeybindingRows(Native.nativeKeys.compileResolvedKeybindingsConfig([{command:command,key:"mod+shift+k",when:"composerFocus"}]),"")[0];phase=3}
  else if(phase===3){check(row.row.source==="Custom","custom native source");row.reset();check(saved&&saved.key===row.row.defaultKey&&saved.replace.key==="mod+shift+k","reset preserves native default/replacement semantics");keys.findChild(row,"keybindingRemove_"+command).triggered();check(removed&&removed.key==="mod+shift+k"&&removed.when==="composerFocus","remove captures exact native key and condition");keys.findChild(row,"keybindingWhen_"+command).clicked();check(keys.findChild(row,"keybindingConditionPopup").visible,"native condition tree opens inline");phase=31}
  else if(phase===31){keys.findChild(row,"keybindingConditionPopup").contentItem.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-keybinding-conditions.png")});phase=4}
  else if(phase===4){if(!captured)return;console.log("KEYBINDING_ROW_QML_PASS: original shortcut pill, actual Qt capture/cancel, exact replace/save/reset/remove and inline condition tree");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("KEYBINDING_ROW_QML_FAIL timeout");Qt.exit(1)}}
}
