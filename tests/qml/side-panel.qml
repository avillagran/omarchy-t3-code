import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int waited:0
 function tab(id){var tabs=keys.findChild(panel,"t3SidePanelTabs");if(!tabs)return null;for(var i=0;i<tabs.children.length;i++)if(tabs.children[i].objectName==="t3SideTab_"+id)return tabs.children[i];return null}
 function check(ok,msg){if(!ok){console.error("SIDE_PANEL_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 function deviceData(){return {environment:"fixture",originKey:"o",viewId:"v",permissions:{configure:true,operate:true},settings:{enableDeviceSupport:true,enableAgentDeviceAccess:false,deviceOnboardingCompleted:true,deviceHosts:[]},state:{hosts:[{id:"local",kind:"local",label:"This machine",platforms:[]}],devices:[{hostId:"local",id:"emulator-5554",name:"Pixel 9",platform:"android",version:"Android 14.0",booted:true,physical:false}],sessions:[],hostStatus:"ready",hostStatuses:{},onboardingCompleted:true,agentAccessEnabled:false,revision:1,hubBasePath:"/api/device-hub"}}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1400,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;var button=keys.findChild(panel,"t3OpenSidePanel");check(button&&button.name==="panel-right","header opens a generic side panel");button.clicked();phase=2
  }else if(phase===2){check(panel.sideShown&&panel.sideTab==="browser"&&!!keys.findChild(panel,"t3BrowserWorkspace"),"side panel starts on the browser tab");check(!!root.tab("devices")&&!!root.tab("renders"),"side panel offers other tools");
   keys.mouseClick(root.tab("devices"));phase=3
  }else if(phase===3){check(panel.sideTab==="devices"&&!panel.browserShown,"devices tab replaces the browser");check(!keys.findChild(panel,"t3BrowserWorkspace"),"browser unloads when another tab shows");var dp=keys.findChild(panel,"t3DevicePanel");check(!!dp,"device panel loads");check(b.deviceView.targets.length===1&&b.deviceView.targets[0].environment==="fixture","devices tab watches the chat environment");
   var all={};all.fixture={busy:false,error:"",requestId:"",target:{environment:"fixture",projectId:null},data:deviceData()};b.deviceView=Object.assign({},b.deviceView,{sessions:all});phase=4
  }else if(phase===4){var list=keys.findChild(panel,"t3DeviceList");check(list&&list.count===1,"device list shows native devices");check(b.deviceOperations.fixture&&b.deviceOperations.fixture.action==="refresh","showing the tab lists devices like native T3");var detail=keys.findChild(list.itemAtIndex(0),"t3DeviceDetail");check(detail&&detail.text.indexOf("Android Android")<0&&detail.text.indexOf("Android 14.0")===0,"version is not repeated: "+(detail?detail.text:""));var open=list.itemAtIndex(0)?keys.findChild(list.itemAtIndex(0),"t3DeviceOpen_emulator-5554"):null;check(open&&open.enabled,"a device can be opened");keys.mouseClick(open);phase=5
  }else if(phase===5){var s=b.deviceScreen;check(s.target&&s.target.deviceId==="emulator-5554"&&s.target.platform==="android","screen targets the chosen device");check(s.requestId!=="","screen request is owned");
   b.receiveDeviceSurface({requestId:s.requestId,kind:"status",stage:"live",input:true});b.receiveDeviceSurface({requestId:s.requestId,kind:"frame",url:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png").toString(),revision:1,width:16,height:16});b.receiveDeviceSurface({requestId:"stale",kind:"error",error:"x"});phase=6;waited=0
  }else if(phase===6){var frame=keys.findChild(panel,"t3DeviceFrame");if(frame.status!==Image.Ready&&++waited<10)return;check(frame.status===Image.Ready,"device frames render");check(b.deviceScreen.stage==="live","stale requests are ignored");
   var cx=frame.width/2,cy=frame.height/2,p=frame.unit(cx,cy);check(p&&Math.abs(p.x-0.5)<0.01&&Math.abs(p.y-0.5)<0.01,"touch maps to normalized screen points "+JSON.stringify(p));check(frame.unit(-5,-5)===null,"touch outside the screen is ignored");
   check(!!keys.findChild(panel,"t3DeviceButtonBack").visible,"Android shows its back button");check(!!keys.findChild(panel,"t3DevicePowerOff"),"the screen offers power off");keys.mouseClick(keys.findChild(panel,"t3DeviceBack"));phase=7
  }else if(phase===7){check(b.deviceScreen.stage==="idle"&&!b.deviceFrame.url,"back stops viewing the device");check(keys.findChild(panel,"t3DeviceList").visible,"list returns");
   keys.mouseClick(root.tab("renders"));phase=8
  }else if(phase===8){check(panel.sideTab==="renders","renders tab");keys.mouseClick(keys.findChild(panel,"t3SidePanelClose"));phase=9
  }else if(phase===9){check(!panel.sideShown&&!panel.sidePanelOpen,"close hides the side panel");panel.close();console.log("SIDE_PANEL_QML_PASS: generic side panel, tabs, devices list, device screen frames, touch mapping, back and close");Qt.quit()}
 }}
}
