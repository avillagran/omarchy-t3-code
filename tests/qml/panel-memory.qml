import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property bool ready:false
 property var fixture:({environments:[],projects:[],threads:[]})
 property int sampleStep:0
 readonly property int cycles:Number(Quickshell.env("OT3_BENCH_CYCLES")||0)
 function fill(){
  var b=Test.T3Backend
  var count=Number(Quickshell.env("OT3_BENCH_MESSAGES")||0),paragraphs=Number(Quickshell.env("OT3_BENCH_PARAGRAPHS")||10)
  if(count){
   b.selectedEnvironment="fixture";b.selectedThread="0";b.selectedKey="fixture/0"
   b.patchDraft("fixture/0",{text:"Unsent draft",attachments:[],loading:false,busy:false,attempt:null})
   var messages=[]
   for(var n=0;n<count;n++){
    var text="",html=""
    for(var j=0;j<paragraphs;j++){var line="Paragraph "+j+" of message "+n+": "+"A deterministic long conversation with selectable text, code and links. ".repeat(5);text+=line+"\n\n";html+="<p>"+line+" <a href='https://example.com'>reference</a></p>"}
    messages.push({id:"m"+n,role:n%2?"assistant":"user",text:text,rendered:n%2?html:"",createdAt:1000+n})
   }
   b.transcript=messages;b.messagesUpdated()
  }
 }
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.ready)Test.T3Backend.state=root.fixture}}
 Timer{interval:100;running:!root.ready;repeat:true;onTriggered:{
  var b=Test.T3Backend;if(!b.viewStateLoaded)return
  fixture={environments:[],projects:[],threads:Array.from({length:Number(Quickshell.env("OT3_BENCH_ROWS")||24)},function(_,i){return {key:"fixture/"+i,id:String(i),environmentId:"fixture",title:"Thread "+i,project:"Example project",branch:"main",updatedAt:1000+i}})}
  b.rememberView({animationsEnabled:false,windowMode:"floating",windowWidth:1200,windowHeight:900,sidebarCompact:false});b.state=fixture;ready=true
  root.fill()
  panel.open();measure.start()
 }}
 Timer{id:measure;interval:3500;onTriggered:{
 if(root.cycles&&root.sampleStep>0&&Test.T3Backend.drafts["fixture/0"].text!=="Unsent draft"){console.error("MEMORY_QML_FAIL draft lost during history cycling");Qt.exit(1);return}
 if(Quickshell.env("OT3_BENCH_COLLECT")==="1"){console.log("MEMORY_PRE_GC");gc();console.log("MEMORY_POST_GC")}console.log("SIDEBAR_MEMORY_READY phase="+(root.sampleStep%2?"cleared":"filled"));
 if(root.sampleStep<root.cycles*2-1){root.sampleStep++;if(root.sampleStep%2)Test.T3Backend.clearSelection();else root.fill();measure.restart()}else finish.start()}}
 Timer{id:finish;interval:800;onTriggered:Qt.quit()}
}
