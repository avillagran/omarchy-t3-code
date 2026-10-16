import QtQuick
import QtQuick.Window
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtWebEngine
import "../js/DocumentTheme.js" as Theme
Window {
 id:root;visible:true;width:1100;height:800;minimumWidth:460;minimumHeight:320
 title:documentData.title+" · T3 Code Omarchy"
 readonly property var themeColors:documentData.theme.variables
 color:themeColors["--background"]
 function verifyDocument(){
  if(!web.item){console.error("DOCUMENT_VERIFY_FAIL no renderer");Qt.exit(1);return}
  web.item.runJavaScript("JSON.stringify({status:window.__t3DocumentStatus,sandbox:document.querySelector('iframe').getAttribute('sandbox')})",function(value){console.log("DOCUMENT_VERIFY "+value);Qt.quit()})
 }
 property bool sourceMode:!documentData.html
 ColumnLayout{
  anchors.fill:parent;spacing:0
  Rectangle{
   Layout.fillWidth:true;implicitHeight:48;color:root.themeColors["--card"]
   RowLayout{
    anchors.fill:parent;anchors.margins:8
    Text{Layout.fillWidth:true;text:documentData.title;textFormat:Text.PlainText;elide:Text.ElideMiddle;color:root.themeColors["--foreground"]}
    Controls.Button{palette.button:root.themeColors["--card"];palette.buttonText:root.themeColors["--foreground"];text:"‹/›";Accessible.name:documentData.labels?(root.sourceMode?documentData.labels.preview:documentData.labels.source):"‹/›";checkable:documentData.html;checked:root.sourceMode;onClicked:root.sourceMode=!root.sourceMode}
    Controls.Button{text:"−";enabled:web.item&&web.item.zoomFactor>0.5;onClicked:web.item.zoomFactor=Math.max(0.5,web.item.zoomFactor-0.25)}
    Controls.Button{text:web.item?Math.round(web.item.zoomFactor*100)+"%":"100%";onClicked:if(web.item)web.item.zoomFactor=1}
    Controls.Button{text:"+";enabled:web.item&&web.item.zoomFactor<2;onClicked:web.item.zoomFactor=Math.min(2,web.item.zoomFactor+0.25)}
    Controls.Button{text:"×";Accessible.name:documentData.labels?documentData.labels.close:"×";onClicked:root.close()}
   }
  }
  Loader{
   id:web;Layout.fillWidth:true;Layout.fillHeight:true;active:!root.sourceMode
   sourceComponent:Component{
    WebEngineView{
     objectName:"t3DocumentWeb";backgroundColor:root.color
     profile:WebEngineProfile{offTheRecord:true;httpCacheType:WebEngineProfile.MemoryHttpCache;httpCacheMaximumSize:8*1024*1024;persistentCookiesPolicy:WebEngineProfile.NoPersistentCookies;onDownloadRequested:function(download){download.cancel()}}
     settings.localContentCanAccessFileUrls:false
     settings.localContentCanAccessRemoteUrls:false
     settings.javascriptCanAccessClipboard:false
     settings.javascriptCanOpenWindows:false
     settings.fullScreenSupportEnabled:false
     onNewWindowRequested:function(request){}
     onPermissionRequested:function(permission){permission.deny()}
     onNavigationRequested:function(request){if(request.isMainFrame&&request.url.toString()!=="about:blank"&&!request.url.toString().startsWith("data:"))request.action=WebEngineNavigationRequest.IgnoreRequest}
     Component.onCompleted:loadHtml(Theme.page(documentData))
    }
   }
  }
  Controls.ScrollView{
   visible:root.sourceMode;Layout.fillWidth:true;Layout.fillHeight:true;clip:true
   Controls.TextArea {
    text:documentData.contents;textFormat:TextEdit.PlainText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap
    color:root.themeColors["--foreground"];font.family:"monospace";font.pixelSize:14
    background:Rectangle{color:root.color}
   }
  }
 }
 Shortcut{sequence:"Escape";onActivated:root.close()}
}
