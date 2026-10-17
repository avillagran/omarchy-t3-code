import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/MarkdownLinks.js" as Links
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false
 readonly property string source:"[GitHub](https://github.com/avillagran/ot3) · [Branch](https://github.com/avillagran/ot3/tree/main) · [README](./README.md) · [Website](https://example.org)"
 readonly property string rendered:'<p><a href="https://github.com/avillagran/ot3">GitHub</a> · <a href="https://github.com/avillagran/ot3/tree/main">Branch</a> · <a href="./README.md">README</a> · <a href="https://example.org">Website</a></p>'
 function check(ok,msg){if(!ok){console.error("MARKDOWN_LINKS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 TextMetrics{id:iconMetrics;font.family:"FiraCode Nerd Font";font.pixelSize:Style.font.body;text:"\uf09b"}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){
   if(!b.viewStateLoaded)return
   check(Links.glyph("https://github.com/a/b/tree/main")==="\uf09b","native GitHub brand also for branch destinations")
   check(Links.glyph("https://api.github.com/a")==="\uf09b","GitHub subdomain")
   check(Links.glyph("https://github.com.evil.invalid/a")==="\uf0ac","host boundary")
   check(Links.glyph("https://github.com@evil.invalid/a")==="\uf0ac","authority userinfo is not host")
   check(!Links.glyph("#section")&&!Links.glyph("ot3-code-copy://0")&&!Links.glyph("javascript:alert(1)"),"fragments, toolbars and unsupported links untouched")
   check(Links.glyph("./src/main.rs")!==Links.glyph("./README.md"),"file type icons")
   check(Links.decorate('&lt;a href="https://github.com"&gt;literal&lt;/a&gt;').indexOf("<span")<0,"literal HTML code stays literal")
   b.state={threads:[{key:"fixture/links",id:"links",environmentId:"fixture",title:"Link icons"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","links");b.transcript=[{id:"a",role:"assistant",text:root.source,rendered:root.rendered,searchText:"GitHub · Branch · README · Website"},{id:"u",role:"user",text:root.source}];panel.setWindowMode("floating");panel.open();phase=1
  }else if(phase===1){
   if(panel.restoringConversation)return
   var row=c.itemAtIndex(0),user=c.itemAtIndex(1);if(!row||!user)return
   var edit=row.textItem,plain=edit.getText(0,edit.length)
   check(/\uf09b\s+GitHub/.test(plain)&&/\uf09b\s+Branch/.test(plain),"icons precede rendered link labels with whitespace")
   check(/\uf48a\s+README/.test(plain)&&/\uf0ac\s+Website/.test(plain),"file and web icons render inline")
   var pos=plain.indexOf("\uf09b"),rect=edit.positionToRectangle(pos)
   check(edit.linkAt(rect.x+2,rect.y+rect.height/2)==="https://github.com/avillagran/ot3","icon belongs to clickable link")
   var labelRect=edit.positionToRectangle(plain.indexOf("GitHub"))
   check(labelRect.x-rect.x>=iconMetrics.tightBoundingRect.width+Style.space(3),"visible space clears glyph overhang: "+(labelRect.x-rect.x)+" / "+iconMetrics.tightBoundingRect.width)
   check(keys.findChild(row,"copyChatMessage").copyText===root.source,"copy retains source without decoration")
   check(user.textItem.getText(0,user.textItem.length)===root.source,"user input remains literal")
   panel.openFind();panel.findQuery="Branch";phase=2
  }else if(phase===2){
   if(panel.restoringConversation)return
   check(panel.findMatches.length===2,"search counts source labels only")
   var row=c.itemAtIndex(0);check(row.textItem.selectedText==="Branch","find selects label despite preceding icon")
   panel.findOpen=false;keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-markdown-links.png")});phase=3
  }else if(phase===3){if(!captured)return;panel.close();console.log("MARKDOWN_LINKS_QML_PASS: native GitHub brand, files/web, clickable icons, source copy and find selection");Qt.quit()}
 }}
}
