import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 property string target:""
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 function check(ok,msg){if(!ok){console.error("T3_MOSAIC_SIDEBAR_QML_FAIL: "+msg);Qt.exit(1)}}
 function mosaic(){return keys.findChild(panel,"t3ThreadMosaic")}
 function highlighted(){var board=keys.findChild(panel,"t3MosaicBoard");for(var i=0;i<board.children.length;i++){var c=board.children[i];if(c.thread&&c.thread.key===target)return c}return null}
 function clickTarget(){var v=panel.sidebarView;for(var i=0;i<v.count;i++){var r=v.itemAtIndex(i);if(r&&r.thread&&r.thread.key===target){keys.mouseClick(r,r.width/2,r.height/2);return true}}return false}
 Timer{interval:120;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,m=mosaic()
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={environments:[{id:"a",name:"Local"},{id:"node",name:"Node"}],projects:[],threads:[0,1,2,3,4,5].map(function(i){return {key:(i===5?"node":"a")+"/t"+i,id:"t"+i,environmentId:i===5?"node":"a",title:"Thread "+i,updatedAt:1000+i}})}
   b.rememberView({mosaicOpen:true,mosaicCount:2,sidebarCompact:false,animationsEnabled:false});panel.open();phase=1
  }else if(phase===1){
   if(!m||m.threads.length!==6)return
   target=m.threads[4].key;check(Math.floor(4/m.count)!==m.page,"target starts on another page");phase=2
  }else if(phase===2){
   if(!clickTarget()){check(++frames<30,"sidebar target available");return}
   frames=0;phase=3
  }else if(phase===3){
   if(++frames<5)return
   check(b.selectedKey===target,"sidebar selects the exact environment/thread");check(b.viewState.mosaicOpen,"sidebar keeps mosaic open");check(m.shown.some(function(t){return t.key===target}),"target page visible");check(highlighted()&&highlighted().revealed,"target emphasized with motion disabled: key="+m.highlightedKey+", target="+target+", tile="+highlighted())
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3-mosaic-sidebar.png")})
   frames=0;phase=4
  }else if(phase===4){
   if(++frames<16)return
   check(clickTarget(),"repeat click available");frames=0;phase=5
  }else if(phase===5){
   if(++frames<18)return
   check(m.highlightedKey===target,"repeat click renews emphasis");frames=0;phase=6
  }else if(phase===6){
   if(++frames<12)return
   check(m.highlightedKey==="","emphasis expires after a few seconds");check(b.selectedKey===target,"expiry preserves selection");
   m.revealThread(target);m.turn(-1);check(m.highlightedKey==="","manual pagination clears temporary emphasis")
   panel.close();console.log("T3_MOSAIC_SIDEBAR_QML_PASS: real sidebar clicks reveal another page, emphasize with motion off, renew and expire without closing mosaic");Qt.quit()
  }
 }}
}
