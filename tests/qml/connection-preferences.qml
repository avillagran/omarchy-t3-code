import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTION_PREFERENCES_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionSettings:({localEnabled:true,environments:[{id:"node",name:"NODE",enabled:true},{id:"disabled",name:"Hidden",enabled:false}]})
  property var connectionPreferences:({busy:false,error:"",primaryEnvironmentId:"primary",catalogRevision:"one",client:{revision:"a",loadBalancingEnabled:true,loadBalancingWeights:{primary:25,node:100}},github:{environments:[{environmentId:"primary",permission:"read",available:true},{environmentId:"node",permission:"off",available:true}]}})
  property var sent:null
  function manageConnectionPreferences(action,values){sent=Object.assign({action:action},values || {})}
 }
 FloatingWindow{visible:true;implicitWidth:1000;implicitHeight:900;Test.T3ConnectionPreferences{id:view;width:780;anchors.centerIn:parent;bridge:b}}
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){check(b.sent.action==="read","reads native shared preferences");check(view.enabledMachines.length===2&&view.rows.length===2,"enabled hosts including canonical local ID only");view.balancingExpanded=true;view.sharingExpanded=true;phase++
  }else if(phase===1){view.clientPatch({environmentId:"node",weight:0});check(b.sent.weight===0&&b.sent.environmentId==="node","manual-only weight");view.githubPatch("primary","read-write");check(b.sent.action==="github-update"&&b.sent.environmentId==="primary"&&b.sent.permission==="read-write","native primary sharing identity");check(view.balancingSummary().indexOf("NODE")>=0,"native weighted summary");console.log("CONNECTION_PREFERENCES_QML_PASS: enabled machines, canonical IDs, native balancing weights and GitHub permissions");Qt.quit()}
 }}
}
