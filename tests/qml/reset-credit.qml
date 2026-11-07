import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/NativeResetCredit.js" as Credit
import "UnderTest/ui/js/UsageLimits.js" as Usage
import "UnderTest/ui/js/NativeUsageLimits.js" as Native
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 Window {
  id:window;visible:true;width:640;height:480
  Client.T3ResetCreditConfirmation{id:confirmation;bridge:root.bridge;popupHost:window.contentItem}
  Client.QuotaStrip{id:strip;anchors.fill:parent;providers:[];creditBridge:root.bridge;collapsed:false}
 }
 // Compile the actual panel consumer as well as exercising the standalone modal.
 Component{id:panelType;Client.T3Panel{bridge:root.bridge}}
 Timer{interval:400;running:true;onTriggered:{try{tests.test_actual_backend_and_modal()}catch(error){console.error("RESET_CREDIT_QML_FAIL "+error+" "+error.stack);Qt.exit(1)}}}
 TestCase {
  id:tests;parent:window.contentItem;name:"ResetCredit";when:false
  function receipt(allowed){var pending=root.bridge.resetCreditAccessPending.env;check(!!pending);root.bridge.action({op:"provider-reset-credit-access",environment:"env",requestId:pending.requestId,result:{canManageProviders:allowed}})}
  function same(a,b,message){if(a!==b)throw Error(message+" actual="+a+" expected="+b)}
  function check(value,message){if(!value)throw Error(message)}
  function test_actual_backend_and_modal(){
   wait(300)
   var xhr=new XMLHttpRequest();xhr.open("GET",Qt.resolvedUrl("UnderTest/tests/fixtures/reset-credit-4301.json"),false);xhr.send();var fixtures=JSON.parse(xhr.responseText)
   fixtures.summaries.forEach(function(test){same(Credit.resetCreditsSummary(test.credits,test.now,test.compact),test.result,"complete native summary")})
   fixtures.accounts.forEach(function(test){same(JSON.stringify(Native.collectLimitAccounts(new Map(test.presentations))),JSON.stringify(test.result),"whole native account identity/credit/plan projection")})
   var account={key:"account",label:"Synthetic account",redeem:{environmentId:"env",input:{instanceId:"codex"}},resetCredits:{availableCount:2},windows:[]}
   strip.providers=[{provider:"codex",label:"Codex",accounts:[account],windows:[]}]
   same(strip.visibleProviders.length,1,"credits remain visible without a usage window")
   same(Credit.resetCreditsSummary({availableCount:1},0),"1 reset credit banked")
   same(Credit.resetCreditsSummary({availableCount:2,nextExpiresAt:"1970-01-01T01:00:00Z"},0),"2 reset credits banked · next expires in 1h 0m")
   root.bridge.requestResetCredit(account);receipt(false)
   same(root.bridge.resetCreditConfirmation,null,"denied scope never opens confirmation")
   root.bridge.requestResetCredit(account);receipt(true)
   tryVerify(function(){return !!root.bridge.resetCreditConfirmation})
   var dialog=findChild(confirmation,"t3ResetCreditConfirmation")
   tryCompare(dialog,"opened",true)
   mouseClick(findChild(confirmation,"resetCreditCancel"))
   tryCompare(dialog,"opened",false);same(root.bridge.resetCreditOperation.busy,false)
   root.bridge.requestResetCredit(account);receipt(true);tryCompare(dialog,"opened",true)
   root.bridge.resetCreditAccess={env:false}
   same(findChild(confirmation,"resetCreditConfirm").enabled,false,"revoked scope disables confirmation")
   root.bridge.confirmResetCredit();same(root.bridge.resetCreditOperation.busy,false);dialog.close()
   root.bridge.requestResetCredit(account);receipt(true);tryCompare(dialog,"opened",true)
   mouseClick(findChild(confirmation,"resetCreditConfirm"));check(root.bridge.resetCreditOperation.busy)
   var request=root.bridge.resetCreditOperation.requestId
   root.bridge.confirmResetCredit();same(root.bridge.resetCreditOperation.requestId,request,"double confirmation cannot issue another request")
   root.bridge.action({op:"provider-reset-credit",environment:"env",requestId:"stale",result:{outcome:"reset"}})
   check(root.bridge.resetCreditOperation.busy,"stale completion cannot release an in-flight redemption")
   root.bridge.action({op:"provider-reset-credit",environment:"env",requestId:request,result:{outcome:"reset",warning:"Synthetic warning"}})
   same(root.bridge.resetCreditOperation.status,"Synthetic warning");check(!root.bridge.resetCreditOperation.busy)
   for(var outcome of ["reset","nothingToReset","noCredit","alreadyRedeemed"]){
    root.bridge.keepResetCreditOperation({busy:true,status:"",requestId:outcome,account:account})
    root.bridge.action({op:"provider-reset-credit",environment:"env",requestId:outcome,result:{outcome:outcome}})
    check(!!root.bridge.resetCreditOperation.status);check(!root.bridge.resetCreditOperation.busy)
   }
   root.bridge.keepResetCreditOperation({busy:true,status:"",requestId:"unknown",account:account})
   root.bridge.action({op:"provider-reset-credit",environment:"env",requestId:"unknown",error:"rpc_outcome_unknown"})
   same(root.bridge.resetCreditOperation.status,"rpc_outcome_unknown");check(!root.bridge.resetCreditOperation.busy)
   console.log("RESET_CREDIT_QML_PASS: actual backend admission, modal cancel/confirm, revoked scope, one dispatch, stale receipts and outcomes")
   Qt.callLater(Qt.quit)
  }
 }
}
