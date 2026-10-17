import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var events:[]
 function check(v,m){if(!v){console.error("BROWSER_PAGE_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 FloatingWindow {
  id:win;visible:true;implicitWidth:600;implicitHeight:360
  TextInput{id:address;objectName:"fixtureAddress";width:200;height:30;text:"Address"}
  Test.T3BrowserPage{id:page;y:40;width:600;height:300;interactive:true;page:({requestId:"one",url:Qt.resolvedUrl("UnderTest/ui/assets/icons/globe.svg").toString(),revision:1,width:640,height:400});onInput:function(id,event){root.events=root.events.concat([{id:id,event:event}])}}
 }
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:80;running:true;repeat:true;onTriggered:{
  if(phase===0){if(!page.ready)return;var mouse=keys.findChild(page,"t3BrowserPageInput");keys.mouseClick(mouse,300,150,Qt.LeftButton);var press=events.filter(function(e){return e.event.type==="press"});check(press.length===1&&press[0].id==="one"&&Math.abs(press[0].event.x-320)<1&&Math.abs(press[0].event.y-200)<1,"real Qt pointer maps painted image into renderer pixels");var count=press.length;keys.mouseClick(mouse,20,150,Qt.LeftButton);check(events.filter(function(e){return e.event.type==="press"}).length===count,"letterbox clicks never enter page");check(page.point(20,150)===null,"letterbox outside content");address.forceActiveFocus();page.page=Object.assign({},page.page,{requestId:"two",revision:2});phase=1;}
  else if(phase===1){if(!page.ready)return;check(address.activeFocus,"new frame retains address-bar focus");check(!events.some(function(e){return e.id==="two"}),"frame replacement does not emit stale input");page.interactive=false;var count=events.length;keys.mouseClick(keys.findChild(page,"t3BrowserPageInput"),300,150,Qt.LeftButton);check(events.length===count,"read-only surface rejects pointer input");page.visible=false;page.send({type:"key",key:65,text:"a"});check(events.length===count,"hidden surface rejects input");console.log("BROWSER_PAGE_QML_PASS: native image surface, actual pointer mapping, letterbox exclusion, focus retention, read-only and hidden admission");Qt.quit();}
 }}
 Timer{interval:4500;running:true;onTriggered:{console.error("BROWSER_PAGE_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
