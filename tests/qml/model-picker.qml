import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool captured:false
 function check(ok,msg){if(!ok){console.error("T3_MODEL_PICKER_QML_FAIL: "+msg);Qt.exit(1)}}
 QtObject{
  id:fixture
  property string selectedEnvironment:"fixture"
  property var favoritePending:({})
  property string favoriteError:""
  property int chosen:0
  property int refreshes:0
  property var selection:({instanceId:"personal",model:"same"})
  property var catalog:({favoritesReady:true,favorites:[{provider:"work",model:"same"}],providers:[{instanceId:"personal",name:"Codex Personal",driverKind:"codex",ready:true},{instanceId:"work",name:"Codex Work",driverKind:"codex",ready:true},{instanceId:"offline",name:"Unavailable",driverKind:"claude",ready:false}],models:[{instanceId:"personal",providerName:"Codex Personal",driverKind:"codex",model:"same",displayName:"Same model",available:true},{instanceId:"personal",providerName:"Codex Personal",driverKind:"codex",model:"second",displayName:"Second model",available:true},{instanceId:"personal",providerName:"Codex Personal",driverKind:"codex",model:"old",displayName:"Old model",available:true,isLegacy:true},{instanceId:"work",providerName:"Codex Work",driverKind:"codex",model:"same",displayName:"Same model",available:true},{instanceId:"offline",model:"unavailable",available:false}]})
  function refreshCatalog(){refreshes++}
  function chooseModel(index){selection={instanceId:catalog.models[index].instanceId,model:catalog.models[index].model};chosen++}
  function toggleModelFavorite(provider,model){var f=catalog.favorites.slice(),i=f.findIndex(function(a){return a.provider===provider&&a.model===model});if(i<0)f.push({provider:provider,model:model});else f.splice(i,1);catalog=Object.assign({},catalog,{favorites:f})}
 }
 Window{id:win;width:600;height:620;visible:true;color:Color.popups.background
  TestCase{id:keys;when:false}
  Test.T3ModelPicker{id:picker;x:55;y:500;width:270;bridge:fixture;selection:fixture.selection}
 }
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var results=keys.findChild(picker,"t3ModelResults"),search=keys.findChild(picker,"t3ModelSearch"),rail=keys.findChild(picker,"t3ModelProviderRail")
  if(phase===0){keys.mouseClick(picker,100,15);phase=1
  }else if(phase===1){check(picker.popup.visible&&picker.provider==="favorites"&&results.count===1,"shared favorites are initial tab");check(picker.rows[0].model.instanceId==="work","favorite uses instance identity");check(rail.count===4&&!rail.itemAtIndex(3).enabled,"unavailable provider stays disabled");check(fixture.refreshes===1,"open refreshes native catalog");picker.popup.contentItem.grabToImage(function(r){root.captured=r.saveToFile("/tmp/t3-model-picker.png")});phase=100
  }else if(phase===100){if(!captured)return;keys.mouseClick(keys.findChild(results.itemAtIndex(0),"t3ModelFavorite_0"),10,10);phase=2
  }else if(phase===2){check(picker.popup.visible&&fixture.chosen===0&&results.count===0,"star changes favorite without selecting/closing");keys.mouseClick(rail.itemAtIndex(1),20,18);phase=3
  }else if(phase===3){check(picker.provider==="personal"&&results.count===2,"provider filter and folded legacy");keys.mouseClick(keys.findChild(results.itemAtIndex(0),"t3ModelFavorite_0"),10,10);picker.query="work same";phase=4
  }else if(phase===4){check(results.count===1&&picker.rows[0].model.instanceId==="work","search spans providers without confusing same slugs");search.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=5
  }else if(phase===5){check(!picker.popup.visible&&fixture.selection.instanceId==="work"&&fixture.chosen===1,"Enter selects exact native instance");keys.mouseClick(picker,100,15);phase=6
  }else if(phase===6){check(picker.provider==="favorites"&&picker.rows[0].model.instanceId==="personal","updated favorites reused on reopen");search.forceActiveFocus();keys.keyClick(Qt.Key_Left);check(rail.activeFocus,"left from empty search enters rail");keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Right);phase=7
  }else if(phase===7){check(picker.provider==="personal"&&search.activeFocus,"rail keyboard selects provider and returns search");picker.legacyExpanded=true;check(results.count===3,"legacy expands");picker.query="second";phase=8
  }else if(phase===8){check(results.count===1,"search filters results");search.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=9
  }else if(phase===9){check(fixture.selection.model==="second"&&fixture.selection.instanceId==="personal","filtered index maps to original catalog");if(!captured)return;win.visible=false;console.log("T3_MODEL_PICKER_QML_PASS: native favorites, provider rail, search, keyboard, legacy, exact instance and capture");Qt.quit()}
 }}
}
