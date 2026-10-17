import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root
 function check(ok,msg){if(!ok){console.error("SCRIPT_SHORTCUTS_QML_FAIL "+msg);Qt.exit(1);throw new Error(msg)}}
 function reply(b,writable,scripts,operation){var s=b.gitView.scope;b.action({op:"git-read",environment:s.environment,thread:s.thread,requestId:s.requestId,result:{scope:s,revision:"fresh-native",permissions:{terminalWrite:writable},scripts:scripts,operation:operation || {}}})}
 Timer{interval:160;repeat:true;running:true;onTriggered:{var b=Test.T3Backend;if(!b.viewStateLoaded)return
  b.state={primaryEnvironmentId:"fixture",environments:[],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",workspaceRoot:"/fixture"}],threads:[{id:"one",key:"fixture/one",environmentId:"fixture",projectId:"p",projectKey:"fixture/p"},{id:"two",key:"fixture/two",environmentId:"fixture",projectId:"p",projectKey:"fixture/p"}]};b.selectThread("fixture","one");b.patchDraft(b.selectedKey,{text:"Do not send this draft",attachments:[],loading:false,busy:false,attempt:null})
  check(!b.requestScriptShortcut("script.bad/path.run"),"original script ID predicate");check(b.requestScriptShortcut("script.check.run")&&b.scriptShortcutPending&&b.gitView.loading,"shortcut first requests fresh native scope");check(!b.requestScriptShortcut("script.check.run"),"no duplicate while waiting for read");
  reply(b,false,[{id:"check",command:"fixture-command"}]);check(!b.scriptShortcutPending&&!Object.keys(b.gitCommands).length&&b.gitView.operationError==="permission_denied","fresh terminal permission gates dispatch");
  check(b.requestScriptShortcut("script.check.run"),"new explicit shortcut can reread");reply(b,true,[]);check(!Object.keys(b.gitCommands).length&&b.gitView.operationError==="git_script_unavailable","removed native script cannot execute stale command");
  b.requestScriptShortcut("script.check.run");var stale=b.gitView.scope;b.selectThread("fixture","two");b.action({op:"git-read",environment:stale.environment,thread:stale.thread,requestId:stale.requestId,result:{scope:stale,revision:"old",permissions:{terminalWrite:true},scripts:[{id:"check"}]}});check(!b.scriptShortcutPending&&!Object.keys(b.gitCommands).length,"selection change cancels waiting shortcut");
  b.selectThread("fixture","one");b.requestScriptShortcut("script.check.run");reply(b,true,[{id:"check",command:"fixture-command"}],{state:"unknown",actionId:"old"});check(!Object.keys(b.gitCommands).length&&!b.requestScriptShortcut("script.check.run"),"unknown native operation cannot be duplicated");
  b.dismissGit();b.requestScriptShortcut("script.check.run");reply(b,true,[{id:"check",command:"fixture-command"}]);var commands=Object.keys(b.gitCommands).map(function(id){return b.gitCommands[id]});check(commands.length===1&&commands[0].op==="git-action","one dispatch through existing journaled native pipeline");var intent=commands[0].intent;check(intent.operation==="script"&&intent.scriptId==="check"&&intent.revision==="fresh-native"&&intent.confirmed&&intent.scope.thread==="one","exact native scope/revision/script intent");check(!b.scriptShortcutPending&&!b.requestScriptShortcut("script.check.run"),"admitted command is never automatically replayed");check(b.drafts["fixture/one"].text==="Do not send this draft"&&!b.drafts["fixture/one"].attempt,"script shortcut never sends composer draft");
  console.log("SCRIPT_SHORTCUTS_QML_PASS: fresh scoped read, native ID/permissions/membership, selection cancellation, unknown/pending suppression and one existing journaled dispatch");Qt.quit()
 }}
}
