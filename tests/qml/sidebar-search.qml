import QtQuick
import Quickshell
import QtTest
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 function check(ok,msg){if(!ok){console.error("T3_SIDEBAR_SEARCH_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 // query, active rows, settled rows
 readonly property var queries:[["",5,3],["zzz",0,0],["dictation",0,1],["dicta largo",0,1],["DICTATÍON",0,1],["dictaiton",0,1],["sidebar refactor",1,0],["omarchy",5,3]]
 Timer {interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   var titles=["Refactor the sidebar model","Audit the live transport","Port upstream palette ranking","Release 0.2.23","Explain the reconnect backoff","Choose the base branch","Fix dictation long clips","Android AAR build"]
   b.state={environments:[{id:"a",name:"omarchy"}],projects:[{key:"a/p",name:"omarchy-t3-code"}],threads:titles.map(function(t,i){return {key:"a/t"+i,id:"t"+i,environmentId:"a",projectKey:"a/p",project:"omarchy-t3-code",projectMonogram:"OT",title:t,settled:i>=5,working:i===1,updatedAt:1000+i}})}
   panel.open();phase=1
  } else if(phase<=root.queries.length){
   var q=root.queries[phase-1]
   if(frames===0)b.search(q[0])
   if(++frames<4)return
   frames=0
   var n=phase
   check(panel.activeRows.length===q[1],"\""+q[0]+"\" lists "+q[1]+" active, got "+panel.activeRows.length)
   check(panel.settledRows.length===q[2],"\""+q[0]+"\" lists "+q[2]+" settled, got "+panel.settledRows.length)
   check(!q[0]||panel.visibleSettledRows.length===q[2],"settled matches show while searching")
   var filler=keys.findChild(panel,"sidebarFiller"),footer=keys.findChild(panel,"t3SidebarFooter")
   check(footer.mapToItem(null,0,0).y+footer.height>panel.height*0.0+keys.findChild(panel,"t3PanelLayout").height-Style.space(80),"the footer stays at the bottom for \""+q[0]+"\"")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/ot3s/search-"+n+".png")})
   phase++
  } else {panel.close();console.log("T3_SIDEBAR_SEARCH_QML_PASS: accents, case, word order, typos and hosts match; shelves open and the footer stays put");Qt.quit()}
 }}
}
