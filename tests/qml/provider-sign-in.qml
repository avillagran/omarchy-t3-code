import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/qml"
// Signed-out providers on a connected host show one card per host; "Sign in…"
// asks for that host's provider settings and "Not now" hides that set only.
ShellRoot {
 id:root;property int phase:0;property string requested:""
 function check(ok,msg){if(!ok){console.error("PROVIDER_SIGN_IN_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 FloatingWindow{visible:true;implicitWidth:520;implicitHeight:400
  Test.T3ProviderSignInNotice{id:notice;width:480;bridge:Test.T3Backend;onSignInRequested:function(environment){root.requested=environment}}
 }
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return
   b.rememberView({signInDismissed:{}})
   b.state={threads:[],projects:[],environments:[{id:"local",name:"mbp-m2",state:"ready"},{id:"node",name:"NODE",state:"ready"},{id:"down",name:"down",state:"connecting"}]}
   b.catalogs={local:{providers:[{instanceId:"codex",name:"Codex",signIn:null}]},node:{providers:[{instanceId:"codex",name:"Codex",signIn:"cli"},{instanceId:"claudeAgent",driverKind:"claudeAgent",name:"Claude",signIn:"cli"},{instanceId:"cursor",name:"Cursor",signIn:"t3"},{instanceId:"opencode",name:"OpenCode",signIn:null}]},down:{providers:[{instanceId:"codex",name:"Codex",signIn:"cli"}]}}
   phase=1
  }else if(phase===1){
   check(b.providerSignIns.length===1&&b.providerSignIns[0].environment==="node","only the connected host with signed-out providers: "+JSON.stringify(b.providerSignIns))
   check(notice.visible&&keys.findChild(notice,"t3ProviderSignInNames_node").text.indexOf("Codex, Claude, Cursor")===0,"names the signed-out providers")
   keys.findChild(notice,"t3ProviderSignInOpen_node_cursor").clicked();check(root.requested==="node","a provider T3 signs in opens that host's providers")
   var claude=keys.findChild(notice,"t3ProviderSignInOpen_node_claudeAgent");claude.clicked()
   var login=b.providerLogin("node","claudeAgent");check(login&&!login.error&&claude.text.indexOf("Claude")>=0&&claude.text!==T3I18n.tr("Sign in to {provider}…",{provider:"Claude"}),"a CLI login waits in its button: "+claude.text)
   var c=Object.assign({},b.catalogs);c.node={providers:c.node.providers.map(function(p){return p.instanceId==="claudeAgent"?Object.assign({},p,{signIn:null}):p})};b.catalogs=c
   check(!b.providerLogin("node","claudeAgent")&&b.providerSignIns[0].providers.length===2,"signing in ends the wait and drops that provider")
   keys.findChild(notice,"t3ProviderSignInLater_node").clicked();phase=2
  }else if(phase===2){
   check(b.providerSignIns.length===0&&!notice.visible,"not now hides that set")
   var c=Object.assign({},b.catalogs);c.node={providers:c.node.providers.concat([{instanceId:"opencode",name:"OpenCode",signIn:"cli"}]).filter(function(p,i,a){return a.findIndex(function(q){return q.instanceId===p.instanceId})===i}).map(function(p){return p.instanceId==="opencode"?Object.assign({},p,{signIn:"cli"}):p})};b.catalogs=c;phase=3
  }else if(phase===3){
   check(b.providerSignIns.length===1&&b.providerSignIns[0].providers.length===3,"another provider signing out shows it again")
   var d=Object.assign({},b.catalogs);d.node={providers:d.node.providers.map(function(p){return Object.assign({},p,{signIn:null})})};b.catalogs=d;phase=4
  }else if(phase===4){
   check(b.providerSignIns.length===0&&!notice.visible,"signing in removes the card")
   b.rememberView({signInDismissed:{}})
   console.log("PROVIDER_SIGN_IN_QML_PASS: per host, connected only, names, T3 sign-in opens settings, CLI login waits and ends when signed in, not now per set, reappears, clears after sign-in");Qt.quit()
  }
 }}
}
