import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/NativeLicenses.js" as Original
ColumnLayout {
 id:root
 objectName:"t3Licenses"
 required property var bridge
 property string query:""
 property bool searchOpen:false
 property string openEntryKey:""
 readonly property var view:bridge.licenseView
 readonly property var entries:view.data?view.data.entries:[]
 readonly property var filtered:Original.nativeLicenses.filterThirdPartyLicenseEntries(entries,query)
 readonly property var notice:bridge.licenseNotice
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.57)
 Text{id:noticeWidth;visible:false;text:"0".repeat(76);font.family:"monospace";font.pixelSize:Style.font.caption}
 spacing:Style.space(10)
 Component.onCompleted:bridge.requestLicenses(false)
 Component.onDestruction:bridge.dismissLicenseNotice()
 onViewChanged:if(view.data&&!view.busy&&!view.error&&openEntryKey&&entries.some(function(e){return e.entryKey===root.openEntryKey}))bridge.requestLicenseNotice(openEntryKey)
 function toggle(entry){openEntryKey=openEntryKey===entry.entryKey?"":entry.entryKey;if(openEntryKey)bridge.requestLicenseNotice(openEntryKey);else bridge.dismissLicenseNotice()}
 RowLayout{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(8)
  Text{Layout.fillWidth:true;text:T3I18n.tr("Third-party notices");color:root.dim;font.pixelSize:Style.font.caption}
  Text{visible:!!root.view.data;text:root.filtered.length===root.entries.length?T3I18n.tr("{count} notices",{count:root.entries.length}):T3I18n.tr("{shown} of {total}",{shown:root.filtered.length,total:root.entries.length});color:root.dim;font.pixelSize:Style.font.caption}
  Controls.TextField{id:search;objectName:"licenseSearch";visible:root.searchOpen;Layout.preferredWidth:Style.space(180);text:root.query;onTextEdited:root.query=text;placeholderText:T3I18n.tr("Search licenses");color:root.fg;placeholderTextColor:root.dim;font.pixelSize:Style.font.caption;selectByMouse:true
   background:Rectangle{color:"transparent";radius:Style.space(5);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12)}
   onActiveFocusChanged:if(!activeFocus&&!text)root.searchOpen=false
   Keys.onEscapePressed:{root.query="";root.searchOpen=false}
  }
  T3IconButton{objectName:"licenseSearchToggle";visible:!root.searchOpen&&!!root.view.data;name:"search";tooltipText:T3I18n.tr("Search licenses");onClicked:{root.searchOpen=true;Qt.callLater(function(){search.forceActiveFocus()})}}
 }
 Rectangle{Layout.fillWidth:true;implicitHeight:rows.implicitHeight;radius:Style.space(10);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.02);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)
  ColumnLayout{id:rows;width:parent.width;spacing:0
   Text{visible:root.view.busy;Layout.fillWidth:true;Layout.margins:Style.space(16);text:T3I18n.tr("Loading open-source notices…");color:root.dim;font.pixelSize:Style.font.caption}
   ColumnLayout{visible:!!root.view.error;Layout.fillWidth:true;Layout.margins:Style.space(16);spacing:Style.space(8)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Open-source notices are unavailable");color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
    Text{Layout.fillWidth:true;text:T3I18n.error(root.view.error);color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    T3PickerButton{objectName:"licenseRetry";text:T3I18n.tr("Retry");primary:false;onClicked:root.bridge.requestLicenses(true)}
   }
   Text{visible:!!root.view.data&&!root.filtered.length;Layout.fillWidth:true;Layout.margins:Style.space(28);horizontalAlignment:Text.AlignHCenter;text:T3I18n.tr("No licenses match that search.");color:root.dim;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   Repeater{model:root.view.busy||root.view.error?[]:root.filtered
    ColumnLayout{id:licenseRow;required property var modelData;required property int index;Layout.fillWidth:true;spacing:0
     readonly property bool expanded:root.openEntryKey===modelData.entryKey
     readonly property bool current:root.notice.entryKey===modelData.entryKey&&root.view.data&&root.notice.manifestId===root.view.data.manifestId
     RowLayout{Layout.fillWidth:true;spacing:Style.space(6)
      Controls.AbstractButton{id:trigger;objectName:"licenseRow_"+licenseRow.index;Layout.fillWidth:true;implicitHeight:Style.space(36);hoverEnabled:true;leftPadding:Style.space(14);rightPadding:Style.space(6);onClicked:root.toggle(licenseRow.modelData);Accessible.name:licenseRow.modelData.name;Accessible.description:licenseRow.modelData.license
       background:Rectangle{color:trigger.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent";radius:Style.space(5)}
       contentItem:RowLayout{spacing:Style.space(9)
        T3Icon{name:"chevron-right";rotation:licenseRow.expanded?90:0;implicitWidth:Style.space(13);tint:root.dim}
        RowLayout{id:identity;Layout.fillWidth:true;spacing:Style.space(8)
         Text{Layout.maximumWidth:Math.max(0,identity.width-(version.visible?Math.min(version.implicitWidth,Style.space(130))+Style.space(8):0));text:licenseRow.modelData.name;color:root.fg;font.pixelSize:Style.font.caption;font.weight:Font.Medium;elide:Text.ElideRight;textFormat:Text.PlainText}
         Text{id:version;visible:!!licenseRow.modelData.version;Layout.maximumWidth:Style.space(130);text:licenseRow.modelData.version || "";color:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
         Item{Layout.fillWidth:true}
        }
        Text{Layout.maximumWidth:root.width*0.42;text:licenseRow.modelData.license+" · "+Original.nativeLicenses.formatLicenseBundles(licenseRow.modelData.bundles);color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
       }
      }
      T3IconButton{objectName:"licenseSource_"+licenseRow.index;visible:!!licenseRow.modelData.sourceUrl;name:"external-link";implicitWidth:Style.space(28);Layout.rightMargin:Style.space(10);tooltipText:T3I18n.tr("View project source for {name}",{name:licenseRow.modelData.name});onClicked:Qt.openUrlExternally(licenseRow.modelData.sourceUrl)}
     }
     Loader{Layout.fillWidth:true;Layout.leftMargin:Style.space(38);Layout.rightMargin:Style.space(20);Layout.bottomMargin:active?Style.space(16):0;active:licenseRow.expanded;visible:active
      sourceComponent:Component{ColumnLayout{width:parent.width;spacing:Style.space(6)
       Text{visible:!licenseRow.current||root.notice.busy;Layout.fillWidth:true;text:T3I18n.tr("Loading open-source notices…");color:root.dim;font.pixelSize:Style.font.caption}
       Text{visible:licenseRow.current&&!!root.notice.error;Layout.fillWidth:true;text:T3I18n.error(root.notice.error);color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
       T3PickerButton{visible:licenseRow.current&&!!root.notice.error;text:T3I18n.tr("Retry");primary:false;onClicked:root.bridge.requestLicenseNotice(licenseRow.modelData.entryKey)}
       Controls.TextArea{objectName:"licenseNotice_"+licenseRow.index;visible:licenseRow.current&&!root.notice.busy&&!root.notice.error;Layout.fillWidth:true;Layout.maximumWidth:noticeWidth.implicitWidth;Layout.alignment:Qt.AlignLeft;readOnly:true;selectByMouse:true;text:visible?root.notice.noticeText:"";textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;color:root.fg;font.family:"monospace";font.pixelSize:Style.font.caption;padding:0;background:Item{}}
      }}
     }
    }
   }
  }
 }
}
