import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property var summary:({fromTurnCount:0,toTurnCount:1,files:[{path:"src/a.rs"},{path:"src/b.rs"}]})
 property var view:({loading:false,error:"",data:{files:[
  {path:"src/a.rs",oldPath:"src/a.rs",kind:"modified",additions:1,deletions:1,rows:[{kind:"hunk",text:"@@ -1 +1 @@"},{kind:"deletion",text:"-old"+" wider output".repeat(40),oldLine:1},{kind:"addition",text:"+new",newLine:1}],splitRows:[{kind:"hunk",left:{kind:"hunk",text:"@@ -1 +1 @@"},right:{kind:"hunk",text:"@@ -1 +1 @@"}},{kind:"change",left:{kind:"deletion",text:"-old"+" wider output".repeat(40),oldLine:1},right:{kind:"addition",text:"+new",newLine:1}}]},
  {path:"src/b.rs",oldPath:"src/b.rs",kind:"modified",binary:true,missing:true,rows:[],splitRows:[]}
 ],additions:1,deletions:1}})
 property int phase:0
 function check(value,message){if(!value){console.error("DIFF_REVIEW_QML_FAIL "+message);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3DiffReview{id:review;view:root.view;summary:root.summary;initialPath:"src/a.rs";width:900;height:620}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  if(phase===0){check(review.selectedIndex===0,"initial selection");check(keys.findChild(review,"t3DiffFiles")!==null,"file tree");review.diffLayout="split";check(review.split&&review.visibleRows.length===2,"real split rows");review.moveFile(1);phase=1}
  else if(phase===1){check(review.selectedPath==="src/b.rs","next file selection");check(review.file.binary&&review.file.missing,"missing binary metadata");review.selectFile("src/a.rs");review.foldHunk(0);check(review.visibleRows.length===1,"hunk collapse");review.foldHunk(0);review.width=490;phase=2}
  else if(phase===2){check(review.narrow&&!review.split,"responsive unified view");check(review.visibleRows.length===3,"full unified hunk expansion");review.wordWrap=false;check(review.noWrapWidth>review.width,"unwrapped output supports horizontal range");console.log("DIFF_REVIEW_QML_PASS: file selection, split, hunk collapse, narrow layout");Qt.quit()}
 }}
}
