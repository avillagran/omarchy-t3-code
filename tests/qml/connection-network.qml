import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTION_NETWORK_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionNetwork:({busy:false,error:"",data:{revision:"one",canManage:true,mode:"local-only",tailscaleServeEnabled:false,tailscaleServePort:443,tailscale:{running:true,dnsName:"host.tail.ts.net"},endpoints:[]}})
  property var sent:null
  signal connectionNetworkFinished(string action,string error)
  function requestConnectionNetwork(action,values){sent=Object.assign({action:action},values || {})}
  function copyText(value){}
 }
 FloatingWindow{visible:true;implicitWidth:980;implicitHeight:800;Test.T3ConnectionNetwork{id:view;width:760;anchors.centerIn:parent;bridge:b}}
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){check(b.sent.action==="read","reads native launch settings");var toggle=keys.findChild(view,"connectionNetworkEnabled");toggle.checked=true;toggle.toggled();check(b.sent.action==="read","network change waits for restart confirmation");phase++
  }else if(phase===1){keys.findChild(view,"connectionNetworkConfirm").clicked();check(b.sent.action==="apply"&&b.sent.mode==="network-accessible"&&b.sent.confirmed,"confirmed native network change");b.connectionNetworkFinished("apply","");phase++
  }else if(phase===2){keys.findChild(view,"connectionTailscalePort").text="8443";var toggle=keys.findChild(view,"connectionTailscaleEnabled");toggle.checked=true;toggle.toggled();phase++
  }else if(phase===3){keys.findChild(view,"connectionNetworkConfirm").clicked();check(b.sent.action==="apply"&&b.sent.tailscaleServeEnabled&&b.sent.tailscaleServePort===8443,"native tailscale setting and custom HTTPS port");b.connectionNetworkFinished("apply","");b.connectionNetwork=Object.assign({},b.connectionNetwork,{data:Object.assign({},b.connectionNetwork.data,{canManage:false,local:{owner:"gui"}})});phase++
  }else if(phase===4){check(!keys.findChild(view,"connectionNetworkEnabled").enabled,"GUI server ownership protected");console.log("CONNECTION_NETWORK_QML_PASS: native snapshot, restart confirmation, tailnet HTTPS port and process ownership");Qt.quit()}
 }}
}
