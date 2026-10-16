import QtQuick
import Quickshell
import QtTest
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property int index:0;property bool grabbed:false
 property var codes:["en","es","pt","fr","de","it","nl","pl","ru","uk","ja","ko","zh","ar","tr","sv","da","no","fi","cs"]
 property var fixtureState:({threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"Example chat",project:"Example project"}],projects:[],environments:[],working:0,pending:0})
 function check(ok,msg){if(!ok){console.error("T3_I18N_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.phase>0)Test.T3Backend.state=root.fixtureState}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Test.PinnedKeyboardPanel{id:badgeWindow;anchorItem:null;bar:null;contentWidth:200;contentHeight:80;padding:0
  Rectangle{id:badgePreview;anchors.fill:parent;color:Color.popups.background
   Test.T3StatusBadges{id:badges;anchors.centerIn:parent;counts:({working:2,done:5,waiting:0})}
  }
 }
 Timer{interval:130;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend;var l=Test.T3I18n;var layout=keys.findChild(panel,"t3PanelLayout")
  if(phase===0){if(!b.viewStateLoaded||!l.catalog.en)return;Test.T3Palette.values={blue:"#488bff",green:"#43c78a",yellow:"#f0b85a"};b.state=root.fixtureState;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;phase=2
  }else if(phase===2){if(index>=codes.length){phase=5;return}b.rememberView({language:codes[index]});phase=3
  }else if(phase===3){check(l.language===codes[index],"reactive language selection "+codes[index]);check(l.tr("Close")===l.catalog[codes[index]]["Close"],"rendered catalog lookup "+codes[index]);check(l.tr("Settled ({count})",{count:7}).includes("7")&&!l.tr("Settled ({count})",{count:7}).includes("{count}"),"named interpolation "+codes[index]);var picker=keys.findChild(panel,"t3LanguagePicker");check(picker.model[picker.currentIndex].id===codes[index],"language selector follows locale");if(codes[index]==="ar"){check(l.rtl,"Arabic enables RTL");check(panel.sidebarView.mapToItem(layout,0,0).x>panel.conversationView.mapToItem(layout,0,0).x,"RTL mirrors actual sidebar layout");check(l.tr("Settled ({count})",{count:7}).includes("\u2068"),"mixed-direction values isolated")}
    if(["de","ar","ja"].indexOf(codes[index])>=0){grabbed=false;var code=codes[index];layout.grabToImage(function(image){check(image.saveToFile("/tmp/omarchy-t3-i18n-"+code+".png"),"render capture "+code);root.grabbed=true});phase=4}else{index++;phase=2}
  }else if(phase===4){if(!grabbed)return;index++;phase=2
  }else if(phase===5){b.rememberView({language:"fr-CA"});phase=6
  }else if(phase===6){check(l.tr("Close")==="Fermer","regional locale falls back to base");b.rememberView({language:"zz"});phase=7
  }else if(phase===7){check(l.tr("Close")==="Close","unsupported locale falls back to English");check(!keys.findChild(badges,"t3Badge_waiting").visible,"waiting badge hidden at zero");badges.counts={working:2,done:5,waiting:3};panel.close();badgeWindow.open=true;phase=8
  }else if(phase===8){check(keys.findChild(badges,"t3Badge_waiting").visible,"third badge only for waiting input");check(keys.findChild(badges,"t3Badge_working").y<keys.findChild(badges,"t3Badge_done").y,"working upper, done lower");grabbed=false;badgePreview.grabToImage(function(image){check(image.saveToFile("/tmp/omarchy-t3-badges.png"),"badge render capture");root.grabbed=true});phase=9
  }else if(phase===9){if(!grabbed)return;badgeWindow.open=false;console.log("T3_I18N_QML_PASS: 20 locales, reactive selector, placeholders, regional/English fallback, real RTL layout, render captures and status badges");Qt.quit()}
 }}
}
