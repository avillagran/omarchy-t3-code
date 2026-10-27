import QtQuick 6.11
import QtMultimedia
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Loaded only while a video viewer is open. A text-only chat need not start
// FFmpeg, audio devices or a video sink during initial UI construction.
Item {
  id:root
  property url source:""
  required property color foreground
  required property color muted
  property alias player:videoPlayer
  Component.onDestruction:videoPlayer.stop()
  VideoOutput{id:videoOutput;objectName:"t3VideoOutput";anchors.fill:parent;fillMode:VideoOutput.PreserveAspectFit}
  MediaPlayer{
    id:videoPlayer;videoOutput:videoOutput
    audioOutput:AudioOutput{}
    source:root.source
    onSourceChanged:if(source!="")play()
  }
  MouseArea{anchors.fill:parent;onClicked:videoPlayer.playbackState===MediaPlayer.PlayingState?videoPlayer.pause():videoPlayer.play()}
  RowLayout{anchors.left:parent.left;anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:Style.space(8);spacing:Style.space(8);visible:videoPlayer.duration>0
    T3IconButton{name:videoPlayer.playbackState===MediaPlayer.PlayingState?"pause":"play";implicitWidth:Style.space(22);implicitHeight:implicitWidth;onClicked:videoPlayer.playbackState===MediaPlayer.PlayingState?videoPlayer.pause():videoPlayer.play()}
    Controls.Slider{Layout.fillWidth:true;from:0;to:Math.max(1,videoPlayer.duration);value:videoPlayer.position;onMoved:videoPlayer.position=value}
    Text{text:Math.floor(videoPlayer.position/60000)+":"+("0"+Math.floor(videoPlayer.position/1000%60)).slice(-2)+" / "+Math.floor(videoPlayer.duration/60000)+":"+("0"+Math.floor(videoPlayer.duration/1000%60)).slice(-2);color:root.muted;font.pixelSize:Style.font.caption}
  }
}
