import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("CODE_ACTIONS_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[{id:"fixture",state:"ready"}]};b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[],models:[]}};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Keep my draft");b.messagesAboutToChange();b.transcript=[{id:"command",role:"assistant",searchText:"Try this:\nbash experiments/tooltips/run-native",text:"Try this:\n```bash\nbash experiments/tooltips/run-native\n```",rendered:'<p>Try this:</p><table width="100%"><tr><td>bash</td><td align="right"><a href="ot3-code-run://0">Run in terminal</a> · <a href="ot3-code-copy://0">Copy code</a></td></tr></table><pre><code>bash experiments/tooltips/run-native\n</code></pre>',codeBlocks:[{index:0,language:"bash",code:"bash experiments/tooltips/run-native\n",command:"bash experiments/tooltips/run-native"}]}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return;var m=b.transcript[0],row=panel.conversationView.itemAtIndex(0),edit=keys.findChild(row,"chatMessageText");check(!!edit,"command message visible");check(edit.text.indexOf("ot3-code-run://0")>=0,"code block exposes run action");check(edit.text.indexOf(Test.T3I18n.tr("Run in terminal"))>=0,"run action translated");var pos=edit.getText(0,edit.length).indexOf(Test.T3I18n.tr("Run in terminal")),r=edit.positionToRectangle(pos);check(pos>=0,"run action has selectable layout position");keys.mouseClick(edit,r.x+4,r.y+r.height/2);check(b.terminalOperation.busy,"clicking rendered code action launches terminal");b.action({op:"terminal-open",requestId:b.terminalOperation.requestId,error:"terminal_launch_failed"});check(b.draft.text==="Keep my draft"&&panel.opened,"failed launch preserves prompt and panel");check(!b.codeAction(Object.assign({},m,{streaming:true}),"ot3-code-run://0"),"streaming cannot execute");check(!b.codeAction(m,"ot3-code-run://9"),"invalid block cannot execute");check(b.codeAction(m,"ot3-code-copy://0"),"copy code action routes without executing");b.catalogs={fixture:{canWrite:false,providers:[],models:[]}};check(panel.renderMessage(m).indexOf("ot3-code-run://")<0,"read-only presentation omits run");check(panel.renderMessage(m).indexOf("ot3-code-copy://")>=0,"read-only presentation keeps copy");panel.findOpen=true;panel.findQuery="bash";phase=3
  }else if(phase===3){var edit=keys.findChild(panel.conversationView.itemAtIndex(0),"chatMessageText");if(!edit.selectedText)return;check(edit.selectionStart===edit.getText(0,edit.length).lastIndexOf("bash"),"find selects command body instead of language/action toolbar");panel.close();console.log("CODE_ACTIONS_QML_PASS: actual link click, locale, draft, streaming/invalid blocks and read-only copy");Qt.quit()}
 }}
}
