import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
// The real bar widget: right click opens and closes the quick menu, and the app's
// translated labels replace the English ones.
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_BAR_WIDGET_QML_FAIL: "+msg);Qt.exit(1)}}
 FloatingWindow{visible:true;implicitWidth:300;implicitHeight:100;Test.BarWidget{id:w;x:10;y:10}}
 TestCase{id:keys;when:false}
 Timer{interval:300;repeat:true;running:true;onTriggered:{
  var icon=keys.findChild(w,"t3BarIcon"),menu=keys.findChild(w,"t3BarMenu")
  if(phase===0){
   check(!!icon&&!!menu&&!menu.open,"menu starts closed")
   icon.pressed(Qt.RightButton);check(menu.open,"right click opens the menu");phase=1
  }else if(phase===1){
   icon.pressed(Qt.RightButton);check(!menu.open,"a second right click closes it")
   w.status={running:true,updatedAt:Date.now(),counts:{working:0,done:0,waiting:0,failed:0},animationMode:"light",menu:{animations:"Animaciones",full:"Completo",light:"Liviano",off:"Desactivadas",server:"Servidor",restart:"Reiniciar",stop:"Detener",app:"Aplicación",quit:"Salir"}};phase=2
  }else if(phase===2){
   check(w.menuLabels.light==="Liviano"&&w.menuLabels.quit==="Salir","the app's labels are used")
   w.status=({});phase=3
  }else if(phase===3){
   check(w.menuLabels.light==="Liviano","the last labels stay while the app is stopped")
   console.log("T3_BAR_WIDGET_QML_PASS: right click opens and closes the menu, app labels kept");Qt.quit()
  }
 }}
}
