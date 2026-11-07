import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 readonly property bool replay:Quickshell.env("O3_NATIVE_ENTRYPOINT_GUI_REPLAY")==="1"
 property var frame:({})
 property var compositorRect:null
 property bool editing:false
 property bool clicked:false
 property bool preedit:false
 property bool complete:false
 property bool stateReported:false
 property bool pointerReady:false
 property bool peerReady:false
 property bool peerActive:false
 property string pointerStep:""
 function check(value,message){if(!value){console.error("NATIVE_ENTRYPOINT_GUI_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function windowRect(node){if(node.name===window.title)return node.rect;var children=(node.nodes||[]).concat(node.floating_nodes||[]);for(var i=0;i<children.length;i++){var found=windowRect(children[i]);if(found)return found}return null}
 function viewRects(node){var rows=[];if(node.pid)rows.push({rect:node.rect,titleMatches:node.name===window.title,pidPresent:true});var children=(node.nodes||[]).concat(node.floating_nodes||[]);for(var i=0;i<children.length;i++)rows=rows.concat(viewRects(children[i]));return rows}
 function input(event){worker.write(JSON.stringify({op:"input",event:event})+"\n")}
 FloatingWindow {
  id:window;title:"Owned native entrypoint input";visible:true;implicitWidth:700;implicitHeight:500
  Item {
   id:host;anchors.fill:parent
   Test.T3BrowserPage {
    id:page;anchors.fill:parent;page:root.frame;interactive:root.editing&&!root.replay
    onInput:function(id,event){console.log("ENTRYPOINT_GUI_INPUT "+event.type);root.check(id==="native-entrypoint-view","actual input view");if(event.type==="composition"&&event.preedit){root.check(event.preedit==="にほん"&&event.selectionStart===3&&event.selectionEnd===3&&event.replacementStart===undefined,"exact compositor preedit");commit.restart()}root.input(event)}
    
    onInputMethodRejected:function(reason){root.check(false,"Qt input rejected "+reason)}
   }
  }
 }
 Process {
  id:worker;command:["python3","/proof/native-electron-entrypoint.py"];environment:({O3_NATIVE_ENTRYPOINT_GUI:"1"});stdinEnabled:true;running:true
  onStarted:worker.write(JSON.stringify({op:"ready"})+"\n")
  stdout:SplitParser{onRead:function(line){var row=JSON.parse(line);if(row.type==="frame")root.frame=row.page;else if(row.type==="ime-ready"){console.log("ENTRYPOINT_GUI_IME_READY");root.editing=true;focus.running=true}else if(row.type==="complete"){root.check(row.textMatches===true&&(root.preedit||root.replay),"real native editing outcome");root.complete=true;if(pointer.running)pointer.write("quit\n");if(peer.running)peer.write("quit\n")}else root.check(false,"unknown private worker record")}}
  stderr:SplitParser{onRead:function(line){console.log("ENTRYPOINT_NATIVE_METADATA "+line)}}
  onExited:function(code){root.check(code===0&&root.complete,"native worker completed");Qt.quit()}
 }
 Process{id:focus;command:["/opt/sway-runtime/usr/bin/swaymsg",'[title="Owned native entrypoint input"]',"focus"];environment:({LD_LIBRARY_PATH:"/opt/sway-runtime/usr/lib"});onExited:function(code){root.check(code===0,"real GUI focus");keyboard.running=true;layout.running=true}}
 Process{id:layout;command:["/opt/sway-runtime/usr/bin/swaymsg","-r","-t","get_tree"];environment:({LD_LIBRARY_PATH:"/opt/sway-runtime/usr/lib"});stdout:StdioCollector{onStreamFinished:{root.check(text.length<65536,"bounded private compositor metadata");var tree=JSON.parse(text),rect=root.windowRect(tree);root.check(rect!==null,"actual private GUI surface rectangle");root.compositorRect=rect;console.log("ENTRYPOINT_GUI_COMPOSITOR_RECT "+JSON.stringify(rect))}}}
 Process{id:keyboard;command:["wtype","-k","Shift_L"];onExited:function(code){root.check(code===0,"real GUI seat serial")}}
 Process {
  id:pointer;command:["/gui-baseline/pointer"];environment:({O3_PRIVATE_INPUT_FIXTURE:"1"});stdinEnabled:true;running:root.editing&&!root.replay
  stdout:SplitParser{onRead:function(line){if(line==="ready"){root.pointerReady=true;console.log("ENTRYPOINT_GUI_POINTER_READY");return}root.check(line==="ok","real pointer acknowledgement");if(root.pointerStep==="move"){root.pointerStep="press";pointer.write("press\n")}else if(root.pointerStep==="press"){root.pointerStep="release";pointer.write("release\n")}else if(root.pointerStep==="release"){root.pointerStep="";root.clicked=true}else root.check(false,"pointer phase")}}
  onExited:function(code){root.check(code===0&&root.complete,"pointer completed")}
 }
 Process {
  id:peer;command:["/gui-baseline/input-method-peer"];stdinEnabled:true;running:root.editing&&!root.replay
  stdout:SplitParser{onRead:function(line){if(line==="ready")root.peerReady=true;else if(line.indexOf("active ")===0)root.peerActive=true;else if(line.indexOf("inactive ")===0)root.peerActive=false;else root.check(line==="sent preedit"||line==="sent commit","real input-method record")}}
  onExited:function(code){root.check(code===0&&root.complete,"IME peer completed")}
 }
 Timer{id:commit;interval:20;onTriggered:peer.write("commit\n")}
 Timer{interval:40;running:true;repeat:true;onTriggered:{
  if(root.editing&&page.ready&&!root.stateReported){root.stateReported=true;console.log("ENTRYPOINT_GUI_ADMISSION_STATE active="+host.Window.active+" pointer="+root.pointerReady+" peer="+root.peerReady+" peerActive="+root.peerActive+" dimensions="+page.width+"x"+page.height)}
  if(root.editing&&root.compositorRect&&root.pointerReady&&page.ready&&host.Window.active&&!root.clicked&&!root.pointerStep){var scale=Math.min(page.width/root.frame.width,page.height/root.frame.height),point=Qt.point(root.compositorRect.x+(page.width-root.frame.width*scale)/2+50*scale,root.compositorRect.y+(page.height-root.frame.height*scale)/2+40*scale);console.log("ENTRYPOINT_GUI_POINTER_POINT "+Math.round(point.x)+","+Math.round(point.y));root.pointerStep="move";pointer.write("move "+Math.round(point.x)+" "+Math.round(point.y)+"\n")}
  if(root.clicked&&root.peerReady&&root.peerActive&&page.nativeInputItem&&page.nativeInputItem.activeFocus&&host.Window.active&&!root.preedit){root.preedit=true;peer.write("preedit\n")}
 }}
 Timer{interval:70000;running:true;onTriggered:root.check(false,"real GUI input deadline")}
}
