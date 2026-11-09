import QtQuick
import Quickshell
import "UnderTest/ui/js/BarMenuModel.js" as M
// The bar icon's quick menu: rows by app state and the command each one runs.
ShellRoot{
 function check(v,m){if(!v){console.error("T3_BAR_MENU_QML_FAIL: "+m);Qt.exit(1);throw Error(m)}}
 Timer{interval:1;running:true;onTriggered:{
  var run=M.entries({animations:"Animaciones",quit:"Salir"},true),names=run.map(function(r){return r.header||r.label})
  check(names.join("|")==="Animaciones|Full|Light|Off|Server|Restart|Stop|Application|Restart|Salir","running menu rows and translated labels: "+names.join("|"))
  var stopped=M.entries({},false).map(function(r){return r.header||r.label})
  check(stopped.join("|")==="Server|Restart|Stop|Application|Restart","a stopped app has no animations or quit: "+stopped.join("|"))
  function cmd(i){return JSON.stringify(M.command(run[i]))}
  check(M.command(run[0])===null,"headers do nothing")
  check(cmd(1)==='{"launcher":["call","drive","animationMode","full"]}'&&cmd(2).indexOf('"light"')>0&&cmd(3).indexOf('"off"')>0,"animation modes go to the app")
  check(cmd(5)==='{"server":"restart"}'&&cmd(6)==='{"server":"stop"}',"server rows")
  check(cmd(8)==='{"launcher":["restart"]}'&&cmd(9)==='{"launcher":["quit"]}',"app rows")
  check(M.command({mode:"fast"}).launcher[3]==="fast"&&M.command({server:"format"})===null&&M.command({app:"rm"})===null,"unknown servers and apps are refused")
  check(M.checked(run[2],true,"light")&&!M.checked(run[1],true,"light")&&!M.checked(run[2],false,"light")&&!M.checked(run[5],true,"light"),"only the current animation mode is checked")
  console.log("T3_BAR_MENU_QML_PASS: rows by state, translated labels, commands, current mode");Qt.quit()
 }}
}
