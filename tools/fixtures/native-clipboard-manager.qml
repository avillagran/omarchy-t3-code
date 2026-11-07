import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property bool producerReady:false
 property bool ownerReady:false
 property bool begun:false
 property string focusNext:""
 property string keyMode:""
 property int prompts:0
 property int choices:0
 property int baselineReads:0
 property string clipboardPhase:""
 property bool finished:false
 property int customAttempts:0
 property var pendingStage:null
 function check(value,message){if(!value){console.error("NATIVE_MANAGER_MODAL_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function send(value){worker.write(JSON.stringify(value)+"\n")}
 function offer(value){producer.write(JSON.stringify(value)+"\n")}
 function focus(title,after){check(!focusControl.running&&!keyboard.running,"one physical focus operation");focusNext=after;focusControl.command=["/opt/sway-runtime/usr/bin/swaymsg",'[title="'+title+'"]',"focus"];focusControl.running=true}
 function begin(){if(producerReady&&ownerReady&&!begun){begun=true;clipboardPhase="denied";focus("Owned synthetic clipboard peer","producer-denied")}}
 function focusDone(){
  var action=focusNext;focusNext=""
  if(action==="custom-inspect"){offer({op:"formats",id:30});return}
  if(action==="custom-publish"){offer({op:"custom-binary",id:33});return}
  if(action.indexOf("producer-")===0){clipboardPhase=action.slice(9);offer({op:"instrumented-text",text:clipboardPhase==="denied"?"native denied offer":clipboardPhase==="one"?"native permission allowed one":"native permission allowed two",id:10})}
  else if(action==="gui-denied")send({op:"start"})
  else if(action==="gui-one")send({op:"next-allow"})
  else if(action==="gui-two")send({op:"next-fresh"})
  else if(action==="gui-custom")send({op:"read-custom"})
  else check(false,"unexpected focus action")
 }
 function producerLine(line){var value=JSON.parse(line)
  if(value.id===undefined&&value.ready){producerReady=true;begin();return}
  if(value.id===10){check(value.active===true,"actual producer focus");offer({op:"stats",id:11});return}
  if(value.id===11){baselineReads=value.reads;focus(window.title,"gui-"+clipboardPhase);return}
  if(value.id===12){check(value.reads===baselineReads,"native Block must fetch no system contents");send({op:"next-block"});return}
  if(value.id===13){check(value.reads===baselineReads,"remembered Block must fetch no system contents");focus("Owned synthetic clipboard peer","producer-one");return}
  if(value.id===14){check(value.reads>baselineReads,"Allow reads actual OS bytes");focus("Owned synthetic clipboard peer","producer-two");return}
  if(value.id===15){check(value.reads>baselineReads,"remembered Allow reads fresh actual OS bytes");send({op:"next-custom"});return}
  if(value.id===30){if(value.formats.indexOf('application/web;type="custom/formatmap"')<0||value.formats.indexOf('application/web;type="custom/format0"')<0){root.customAttempts++;root.check(root.customAttempts<100,"actual native custom format metadata "+JSON.stringify(value.formats));customDelay.restart();return}offer({op:"raw",format:'application/web;type="custom/format0"',id:31});return}
  if(value.id===31){root.check(value.bytes==="0001027f80ff","actual custom native egress bytes");focus("Owned synthetic clipboard peer","custom-publish");return}
  if(value.id===33){root.check(value.active===true,"actual custom source focus");focus(window.title,"gui-custom");return}
  check(false,"unexpected producer record")
 }
 function receive(line){var event=JSON.parse(line)
  if(event.type==="browser-surface"){
   var data=event.data
   if(data.kind==="permission-request"){
    prompts++;check(prompts<=2,"no extra prompts for remembered origins");check(data.nativeOptions&&data.nativeOptions.defaultId===1&&data.nativeOptions.cancelId===1,"actual native default/cancel policy");check(data.nativeOptions.buttons[0]==="Allow"&&data.nativeOptions.buttons[1]==="Block","actual native button choices");brokerBridge.browserPermission=data
   }else if(data.kind==="permission-closed"){if(brokerBridge.browserPermission&&brokerBridge.browserPermission.requestId===data.requestId)brokerBridge.browserPermission=null}
   else if(data.kind==="error")check(false,"actual Manager error "+data.error)
   return
  }
  check(event.type==="stage","typed worker record")
  if(event.stage==="admitted")return
  if(event.stage==="owner-ready"){ownerReady=true;begin()}
  else if(event.stage==="blocked-one"){check(prompts===1&&choices===1,"actual default Block settled");offer({op:"stats",id:12})}
  else if(event.stage==="blocked-cached"){check(prompts===1&&choices===1,"per-origin Block remembered");offer({op:"stats",id:13})}
  else if(event.stage==="allowed-one"){check(prompts===2&&choices===2,"actual explicit Allow settled");offer({op:"stats",id:14})}
  else if(event.stage==="allowed-cached"){check(prompts===2&&choices===2,"per-origin Allow remembered");offer({op:"stats",id:15})}
  else if(event.stage==="custom-written"){root.check(prompts===2,"cached native Allow governs custom write");focus("Owned synthetic clipboard peer","custom-inspect")}
  else if(event.stage==="custom-read"){root.check(prompts===2,"cached native Allow governs fresh custom read");send({op:"stop"})}
  else if(event.stage==="complete"){finished=true;offer({op:"quit"})}
  else check(false,"unexpected worker stage "+event.stage)
 }
 QtObject {
  id:brokerBridge
  property var browserPermission:null
  function answerBrowserPermission(allow){var prompt=browserPermission;root.check(prompt!==null,"current native request exists");browserPermission=null;root.choices++;root.send({op:"browser-permission",requestId:prompt.requestId,allow:allow===true})}
 }
 FloatingWindow {
  id:window
  title:"Owned native clipboard permissions"
  visible:true;implicitWidth:700;implicitHeight:500
  Item{id:host;anchors.fill:parent;Test.T3BrowserPermission{id:prompt;bridge:brokerBridge;popupHost:host}}
 }
 Loader{id:platform;active:true;source:"/fixture/T3BrowserClipboardPlatform.qml"}
 Process {
  id:worker
  command:["/fixture/native-manager-probe"]
  environment:platform.item?({O3_GUI_CLIPBOARD_ENDPOINT:platform.item.endpoint}):({})
  stdinEnabled:true
  running:platform.status===Loader.Ready&&platform.item.available
  onStarted:root.check(platform.item.bindWorker(worker.processId).length>0,"actual manager worker kernel binding")
  stdout:SplitParser{onRead:function(line){root.receive(line)}}
  onExited:function(code){root.check(code===0&&root.finished,"manager completed normally")}
 }
 Process{id:producer;command:["/fixture/clipboard-owner"];environment:({O3_TEST_WAYLAND:"1"});stdinEnabled:true;running:true;stdout:SplitParser{onRead:function(line){root.producerLine(line)}}onExited:function(code){root.check(code===0&&root.finished,"producer completed normally")}}
 Process{id:focusControl;environment:({LD_LIBRARY_PATH:"/opt/sway-runtime/usr/lib"});onExited:function(code){root.check(code===0,"physical private compositor focus");root.keyMode="pulse";keyboard.command=["/usr/bin/wtype","-k","Shift_L"];keyboard.running=true}}
 Process{id:keyboard;onExited:function(code){root.check(code===0,"physical private compositor keyboard");if(root.keyMode==="pulse")root.focusDone()}}
 property string sentRequest:""
 Timer{interval:40;running:true;repeat:true;onTriggered:{
  if(prompt.opened&&brokerBridge.browserPermission&&root.sentRequest!==brokerBridge.browserPermission.requestId&&!keyboard.running&&!focusControl.running){
   root.check(host.Window.active&&prompt.defaultFocusItem.activeFocus,"real GUI seat and native default Block focus")
   root.sentRequest=brokerBridge.browserPermission.requestId
   root.send({op:"browser-permission",requestId:"stale-unrelated-request",allow:true})
   root.keyMode="choice";keyboard.command=root.prompts===1?["/usr/bin/wtype","-k","Return"]:["/usr/bin/wtype","-M","shift","-k","Tab","-m","shift","-k","Return"];keyboard.running=true
  }
  if(root.finished&&!worker.running&&!producer.running&&!platform.item.workerAdmitted){console.log("NATIVE_CLIPBOARD_MANAGER_MODAL_PASS actual-owned-server actual-Manager physical-default-Block explicit-Allow remembered-origins fresh-data zero-denied-content-fetches exact-custom-MIME");Qt.quit()}
 }}
 Timer{id:customDelay;interval:20;onTriggered:root.offer({op:"formats",id:30})}
 Timer{interval:90000;running:true;onTriggered:root.check(false,"native manager proof deadline")}
}
