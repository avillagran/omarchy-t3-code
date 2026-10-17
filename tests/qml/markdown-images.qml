import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property var imageLayout:null;property var image:({id:"markdown-image",type:"image",name:"Panel actualizado",source:"/home/other/screen.png"})
 function check(ok,msg){if(!ok){console.error("MARKDOWN_IMAGES_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:300;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"Images"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");b.imageStates={"fixture/one/markdown-image":{url:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png"),loading:false,width:480,height:320}};b.transcript=[{id:"m",role:"assistant",text:"Before image\nPanel actualizado\nAfter image",rendered:'<p>Before image</p><p><a href="ot3-image-open://0">Panel actualizado</a></p><p>After image</p>',markdownImages:[root.image],attachments:[],createdAt:1000}];panel.open();phase=1}
  else if(phase===1){if(panel.restoringConversation)return;var item=c.itemAtIndex(0);if(!item)return;var edit=item.textItem;check(edit.text.indexOf('<img')>=0,"native rich text embeds the resolved image");check(edit.text.indexOf("Before image")<edit.text.indexOf('<img')&&edit.text.indexOf('<img')<edit.text.indexOf("After image"),"image stays between its surrounding paragraphs");check(edit.height>320,"actual Qt document reserves image height");var imageHeight=Number(edit.text.match(/<img[^>]*height="([0-9]+)"/)[1]);root.imageLayout={documentHeight:edit.height,imageHeight:imageHeight,extra:edit.height-imageHeight};check(edit.height-imageHeight<140,"image paragraph does not multiply the image height with prose leading");check(edit.text.indexOf('src="/home/other/')<0,"host filesystem paths never reach the Qt image loader");edit.linkActivated("ot3-image-open://0");phase=2}
  else if(phase===2){check(keys.findChild(panel,"t3ImagePopup").visible,"inline image expands in native viewer");check(keys.findChild(panel,"t3FullImage").status===Image.Ready,"expanded image decodes");keys.findChild(panel,"t3ImagePopup").close();check(b.imageKey(root.image)==="fixture/one/markdown-image","workspace cache is scoped by environment and thread");console.log("MARKDOWN_IMAGES_QML_PASS: in-position native image, proportional size, private resolved URL and full-size viewer "+JSON.stringify(root.imageLayout));panel.close();Qt.callLater(Qt.quit)}
 }}
}
