import QtQuick
import Quickshell
import "UnderTest/ui/js/NativeBrowserImport.js" as Original
ShellRoot{
 function check(v,m){if(!v){console.error("BROWSER_IMPORT_LOGIC_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 Timer{interval:1;running:true;onTriggered:{
  var n=Original.nativeImport,source={id:"firefox",name:"Firefox",profiles:[{directory:"a",name:"A"}]};
  check(n.resolveEnvironmentOptionLabel({isPrimary:true,environmentId:"local",runtimeLabel:"Local"})==="This device","original primary environment caption");check(n.resolveEnvironmentOptionLabel({isPrimary:true,environmentId:"local",runtimeLabel:"My machine"})==="My machine","custom runtime labels remain user text");
  check(n.initialWizardStep(source).step==="configure","available source opens configuration");check(n.initialWizardStep({profiles:[],unavailable:"browserRunning"}).step==="quit","running source has quit/recheck screen");check(n.initialWizardStep({profiles:[]}).reason==="unknownSourceProfile","empty source blocked");
  check(n.initialTargetSelection(true,[]).kind==="new","prefer new target when cap allows");check(n.initialTargetSelection(false,[{id:"work"}]).profileId==="work","at cap prefer first existing target");check(!n.resolveWizardTarget({kind:"existing",profileId:"gone"},"new",[]),"removed destination rejected");
  check(!n.canCloseWizard({step:"importing"})&&n.canCloseWizard({step:"configure"}),"original import dialog close rules");check(n.outcomeToStep({kind:"blocked",reason:"browserRunning"}).step==="quit","reopened source returns to quit");check(n.outcomeToStep({kind:"imported",imported:2,skipped:1,skippedDomains:[],targetName:"Firefox"}).step==="done","import result goes to done");
  check(n.refreshedSourceProfileDirectory("a",source)==="a"&&n.refreshedSourceProfileDirectory("gone",source)==="a","refresh retains or resolves source profile");check(n.isRetryableReason("keychainItemMissing")&&!n.isRetryableReason("unsupportedPlatform"),"retry semantics reused");check(n.formatSkippedDomains(["a","b","c","d","e"])==="a, b, c and 2 more","original skipped-host summary");check(n.failureCopy.profileNotSaved.indexOf("couldn't be saved")>=0,"original failure copy reused");
  console.log("BROWSER_IMPORT_LOGIC_QML_PASS: original targets, wizard transitions, close/retry rules, source refresh and failure copy");Qt.quit();
 }
 }
}
