import QtQuick
import Quickshell
import QtTest
import "UnderTest" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false;property var image:({type:"image",id:"fixture-image",name:"test.png",mimeType:"image/png",sizeBytes:2048})
 function check(ok,msg){if(!ok){console.error("T3_LIVE_MEDIA_QML_FAIL: "+msg);Qt.exit(1)}}
 function append(row){var b=Test.T3Backend;b.messagesAboutToChange();b.transcript=b.transcript.concat([row]);b.messagesUpdated()}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase>0&&panel.restoringConversation)return
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<20;i++)rows.push({id:"m"+i,role:"assistant",text:"Historical message "+i+"\nMore text to make the chat scroll",createdAt:1000});b.transcript=rows;panel.open();phase=1
  }else if(phase===1){check(c.atYEnd,"new chat at end");append({id:"incoming",role:"assistant",text:"Incoming without own send\n".repeat(20),createdAt:2000});phase=2
  }else if(phase===2){check(c.atYEnd,"incoming message follows tail without own send");b.imageStates={"fixture/fixture-image":{url:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png"),loading:false}};append({id:"image-message",role:"user",text:"An image",attachments:[root.image],createdAt:3000});phase=3
  }else if(phase===3){check(c.atYEnd,"attachment growth keeps tail visible");var row=c.itemAtIndex(c.count-1);var thumb=keys.findChild(row,"t3MessageThumbnail");check(!!thumb&&thumb.status===Image.Ready,"image thumbnail loads");keys.mouseClick(thumb.parent,thumb.width/2,thumb.height/2);phase=4
  }else if(phase===4){check(keys.findChild(panel,"t3ImagePopup").visible,"click opens image viewer");var full=keys.findChild(panel,"t3FullImage");check(full.status===Image.Ready&&full.width>300,"full-size image loads at larger size");keys.findChild(panel,"t3ImageContents").grabToImage(function(r){root.captured=r.saveToFile("/tmp/omarchy-t3-image-viewer.png")});phase=40
  }else if(phase===40){if(!captured)return;keys.findChild(panel,"t3ImagePopup").close();c.forceActiveFocus();keys.keyClick(Qt.Key_Home);phase=5
  }else if(phase===5){check(!c.atYEnd&&!panel.followingTail,"manual reading disengages follow");append({id:"another",role:"assistant",text:"New message while reading old ones",createdAt:4000});phase=6
  }else if(phase===6){check(!c.atYEnd,"incoming respects reading old messages");c.forceActiveFocus();keys.keyClick(Qt.Key_End);phase=7
  }else if(phase===7){check(c.atYEnd&&panel.followingTail,"End resumes following");append({id:"last",role:"assistant",text:"Final incoming message",createdAt:5000});phase=8
  }else if(phase===8){check(c.atYEnd,"tail continues after resuming");panel.close();console.log("T3_LIVE_MEDIA_QML_PASS: incoming tail, image growth, thumbnail/full viewer, manual reading and resume");Qt.quit()}
 }}
}
