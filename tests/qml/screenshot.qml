import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var events:[]
 function check(ok,msg){if(!ok){console.error("T3_SCREENSHOT_QML_FAIL: "+msg);Qt.exit(1)}}
 function named(item,name){if(item.objectName===name)return item;var kids=item.children || [];for(var i=0;i<kids.length;i++){var found=named(kids[i],name);if(found)return found}return null}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onScreenshotCapture(active){root.events=root.events.concat([active])}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,button=root.named(panel.composerView,"t3ComposerScreenshot")
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;check(!!button&&button.visible,"button shown by default")
   // A one-pixel PNG written to the helper's {file} placeholder.
   b.rememberView({screenshotCommand:"printf '\\x89PNG\\r\\n\\x1a\\n\\0\\0\\0\\rIHDR\\0\\0\\0\\x01\\0\\0\\0\\x01\\x08\\x06\\0\\0\\0\\x1f\\x15\\xc4\\x89\\0\\0\\0\\rIDATx\\x9cc\\xf8\\x0f\\0\\x01\\x01\\x01\\0\\x18\\xdd\\x8d\\xb0\\0\\0\\0\\0IEND\\xaeB`\\x82' > {file}"})
   b.captureScreenshot();check(b.screenshot.busy,"capture marks busy");check(!panel.opened&&panel.screenshotHidden,"panel hides during capture");phase=2
  }else if(phase===2){if(b.screenshot.busy)return
   check(b.screenshot.error==="","capture succeeded "+b.screenshot.error);check(b.draft.attachments.length===1&&/^screenshot-.*\.png$/.test(b.draft.attachments[0].name),"screenshot attached "+JSON.stringify(b.draft.attachments));phase=3
  }else if(phase===3){check(panel.opened&&!panel.screenshotHidden,"panel reopened after capture");check(JSON.stringify(root.events)==="[true,false]","capture start/finish signals")
   b.rememberView({screenshotCommand:"exit 1",screenshotHidePanel:false});b.captureScreenshot();phase=4
  }else if(phase===4){if(b.screenshot.busy)return;check(b.draft.attachments.length===1&&!b.screenshot.error,"cancelled capture attaches nothing");check(panel.opened,"panel stays open without hiding")
   b.rememberView({screenshotCommand:"exit 7"});b.captureScreenshot();phase=5
  }else if(phase===5){if(b.screenshot.busy)return;check(b.screenshot.error==="The screenshot could not be taken.","failed command reports an error")
   b.rememberView({screenshotButton:false});phase=6
  }else if(phase===6){check(!button.visible,"preference hides the button");panel.close();console.log("T3_SCREENSHOT_QML_PASS: capture, attach, panel hiding, cancellation, failure and visibility preference");Qt.quit()}
 }}
}
