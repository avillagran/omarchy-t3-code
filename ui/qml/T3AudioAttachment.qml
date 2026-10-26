import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import QtMultimedia
import qs.Commons
RowLayout {
 id:root
 required property var bridge
 required property var attachment
 property bool active:true
 readonly property var media:(bridge.videoStates||{})[bridge.videoKey(attachment)]||({})
 spacing:Style.space(8)
 onActiveChanged:if(!active)player.pause()
 Component.onDestruction:player.stop()
 MediaPlayer{id:player;audioOutput:AudioOutput{}
 source:root.active?root.media.url||"":""}
 Connections{target:root.bridge;function onVideoStatesChanged(){if(root.active&&root.media.url&&root.wantPlayback)player.play()}}
 property bool wantPlayback:false
 T3IconButton {
  objectName:"audioPlay";name:player.playbackState===MediaPlayer.PlayingState?"pause":"play";tooltipText:T3I18n.tr("Play audio");enabled:!root.media.loading
  onClicked:{if(player.playbackState===MediaPlayer.PlayingState){root.wantPlayback=false;player.pause()}else{root.wantPlayback=true;if(root.media.url)player.play();else root.bridge.requestVideo(root.attachment)}}
 }
 ColumnLayout{
  Layout.fillWidth:true;spacing:Style.space(2)
  Text{Layout.fillWidth:true;text:root.attachment.name||"";elide:Text.ElideRight;color:Color.popups.text;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Controls.Slider{objectName:"audioPosition";Layout.fillWidth:true;from:0;to:Math.max(1,player.duration);value:player.position;enabled:player.duration>0;onMoved:player.position=value}
  Text{Layout.fillWidth:true;visible:!!root.media.error||player.error!==MediaPlayer.NoError;text:root.media.error||player.errorString;wrapMode:Text.Wrap;color:Color.urgent;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
 }
}
