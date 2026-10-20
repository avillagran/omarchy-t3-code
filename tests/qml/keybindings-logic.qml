import QtQuick
import Quickshell
import "UnderTest/ui/js/NativeKeybindings.js" as Original
ShellRoot {
 function check(ok,msg){if(!ok){console.error("KEYBINDINGS_LOGIC_QML_FAIL "+msg);Qt.exit(1)}}
 function run(){
  var k=Original.nativeKeys,defaults=k.mergeWithDefaultKeybindings([]),rows=k.buildKeybindingRows(defaults,"")
  check(defaults.length===82&&k.STATIC_KEYBINDING_COMMANDS.length===84,"actual pinned native defaults and command catalog")
  check(rows.length===82&&rows.every(function(r){return r.source==="Default"}),"native default source and row metadata")
  var groups=k.groupKeybindingRows(rows)
  check(groups[0].id==="navigation"&&groups[1].id==="threads"&&groups[2].id==="composer","native group order")
  var custom=k.compileResolvedKeybindingsConfig([{key:"ctrl+alt+enter",command:"composer.sendAndNewThread",when:"composerFocus && !terminalFocus"},{key:"mod+shift+k",command:"script.check.run"}]),merged=k.mergeWithDefaultKeybindings(custom),customRows=k.buildKeybindingRows(merged,"")
  check(customRows.some(function(r){return r.command==="composer.sendAndNewThread"&&r.key==="ctrl+alt+enter"&&r.source==="Custom"}),"native custom combination is preserved")
  check(customRows.filter(function(r){return r.command==="composer.sendAndNewThread"}).length===1,"native per-command overrides remove defaults")
  check(customRows.some(function(r){return r.command==="script.check.run"&&r.source==="Project"}),"native script commands join project group")
  check(k.filterKeybindingRows(k.buildKeybindingRows(merged),"Composer: Send and Start").length===1,"native command-label filtering")
  check(k.parseKeybindingShortcut("mod++").key==="+","literal plus key follows original parser")
  check(k.parseKeybindingShortcut("ctrl+ctrl+x").ctrlKey===true&&k.parseKeybindingShortcut("ctrl+ctrl+x").key==="x","repeated modifier is normalized as in native parser")
  check(!k.parseWhenExpressionDraft("terminalFocus && (").ok,"original parser rejects malformed expression")
  var ast=k.parseWhenExpressionDraft("!terminalFocus && (composerFocus || futureContext)")
  check(ast.ok&&k.unknownWhenVariables(ast.value)[0]==="futureContext","original unknown-variable reporting without dropping extensible conditions")
  check(k.parseWhenExpressionDraft("").ok&&k.whenAstToExpression(undefined)==="","empty condition matches original")
  check(k.buildWhenVariableOptions().indexOf("usagePageOpen")>=0,"actual condition options are complete")
  check(k.keybindingFromKeyboardEvent({key:"Enter",code:"Enter",ctrlKey:true,metaKey:false,altKey:true,shiftKey:false},"Linux")==="mod+alt+enter","native Linux recorder")
  check(k.keybindingFromKeyboardEvent({key:"Meta",code:"MetaLeft",ctrlKey:false,metaKey:true,altKey:false,shiftKey:false},"Linux")===null,"modifier-only press does not finish recording")
  check(k.keybindingFromKeyboardEvent({key:"!",code:"Digit1",ctrlKey:true,metaKey:false,altKey:false,shiftKey:true},"Linux")==="mod+shift+1","physical punctuation and shifted key follows native recorder")
  var conflicts=k.buildKeybindingRows(k.compileResolvedKeybindingsConfig([{key:"mod+k",command:"sidebar.toggle"},{key:"mod+k",command:"chat.new"}]),"")
  check(conflicts.every(function(r){return r.conflicts.length===1}),"native collision labels")
  check(k.buildKeybindingCommandOptions(merged).indexOf("script.check.run")>=0,"project commands extend native selector")
  console.log("KEYBINDINGS_LOGIC_QML_PASS: exact native defaults, groups, overrides, scripts, filtering, plus/modifier/physical key parsing, conditions and conflicts on actual Qt engine")
  Qt.quit()
 }
 Timer{interval:100;running:true;onTriggered:run()}
}
