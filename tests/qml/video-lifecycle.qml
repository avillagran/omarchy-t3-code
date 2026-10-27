import QtQuick
import QtMultimedia
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 property var fixture:null
 property var video:({id:"clip",type:"file",mimeType:"video/mp4",name:"clip.mp4",sizeBytes:4096})
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 function check(ok,msg){if(!ok){console.error("VIDEO_LIFECYCLE_QML_FAIL: "+msg);Qt.exit(1)}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,loader=keys.findChild(panel,"t3VideoLoader")
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]}
   b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){
   if(b.draft.loading)return
   check(!loader.active&&!loader.item&&!keys.findChild(panel,"t3VideoOutput"),"text chat creates no player or video sink")
   var states={};states[b.videoKey(video)]={url:"file://"+Quickshell.env("OT3_VIDEO_FIXTURE"),loading:false};b.videoStates=states
   panel.openVideo(video);frames=0;phase=2
  }else if(phase===2){
   if(!loader.item||loader.item.player.duration<=0){check(++frames<35,"video loads: "+(loader.item?loader.item.player.errorString:"no viewer"));return}
   check(loader.active&&loader.item.player.playbackState===MediaPlayer.PlayingState,"opening starts playback")
   loader.item.player.pause();check(loader.item.player.playbackState===MediaPlayer.PausedState,"pause works")
   loader.item.player.position=500;loader.item.player.play();check(loader.item.player.playbackState===MediaPlayer.PlayingState,"seek and resume work")
   panel.imageViewer.close();frames=0;phase=3
  }else if(phase===3){
   if(++frames<2)return
   check(!loader.active&&!loader.item&&!keys.findChild(panel,"t3VideoOutput"),"closing destroys multimedia controls")
   panel.openVideo(video);frames=0;phase=4
  }else if(phase===4){
   if(!loader.item||loader.item.player.duration<=0){check(++frames<35,"video reopens");return}
   check(loader.item.player.playbackState===MediaPlayer.PlayingState,"reopened video plays")
   panel.close();console.log("VIDEO_LIFECYCLE_QML_PASS: no idle player, local video load, playback, pause, seek, teardown and reopen");Qt.quit()
  }
 }}
}
