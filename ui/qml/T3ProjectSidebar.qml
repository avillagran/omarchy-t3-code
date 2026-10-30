import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import Qt.labs.qmlmodels
import qs.Commons
import "../js/ProjectTree.js" as Tree

ListView {
    id:root
    objectName:"t3ProjectSidebar"
    required property var bridge
    required property Component threadDelegate
    property var threads:[]
    property var groups:[]
    property var nest:function(rows){return rows}
    readonly property var collapsed:bridge.viewState.projectTreeCollapsed || ({})
    readonly property var entries: {
        var rows=Tree.build(groups,threads,collapsed),out=[]
        // Reuse O3's subagent expansion and every existing thread context action.
        rows.forEach(function(row){if(row.kind!=="thread"){out.push(row);return}root.nest([row.thread]).forEach(function(t){out.push(Object.assign({},row,{key:t.key,thread:t}))})})
        return out
    }
    readonly property var visibleThreads:entries.filter(function(r){return r.kind==="thread"}).map(function(r){return r.thread})
    readonly property color fg:Color.popups.text
    readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.55)
    model:treeModel
    clip:true;reuseItems:true;interactive:false;activeFocusOnTab:true;keyNavigationEnabled:true;cacheBuffer:0
    T3Wheel{flickable:root}
    Controls.ScrollBar.vertical:Controls.ScrollBar{}
    ListModel{id:treeModel;dynamicRoles:true}
    function toggle(key){var next=Object.assign({},collapsed);if(next[key])delete next[key];else next[key]=true;bridge.rememberView({projectTreeCollapsed:next})}
    function activate(){var entry=entries[currentIndex];if(!entry)return;if(entry.kind==="thread")bridge.selectThread(entry.thread.environmentId,entry.thread.id);else toggle(entry.key)}
    function reveal(key){
        var all=Tree.build(groups,threads,{}),entry=all.find(function(r){return r.kind==="thread"&&r.key===key})
        if(!entry)return
        var next=Object.assign({},collapsed),p="project/"+entry.projectKey;delete next[p];delete next[p+"/state/"+entry.state];bridge.rememberView({projectTreeCollapsed:next})
        Qt.callLater(function(){var i=root.entries.findIndex(function(r){return r.key===key});if(i>=0){root.currentIndex=i;root.positionViewAtIndex(i,ListView.Contain)}})
    }
    function sync(){
        var old=treeModel.count&&currentIndex>=0&&currentIndex<treeModel.count?treeModel.get(currentIndex).row.key:""
        entries.forEach(function(entry,i){
            var at=-1;for(var j=i;j<treeModel.count;j++)if(treeModel.get(j).row.key===entry.key){at=j;break}
            var row=entry.kind==="thread"?entry.thread:entry
            if(at<0)treeModel.insert(i,{kind:entry.kind,row:row});else{if(at!==i)treeModel.move(at,i,1);treeModel.setProperty(i,"row",row)}
        })
        if(treeModel.count>entries.length)treeModel.remove(entries.length,treeModel.count-entries.length)
        var index=entries.findIndex(function(r){return r.key===old});if(index>=0)currentIndex=index
    }
    onEntriesChanged:sync()
    Component.onCompleted:sync()
    Keys.onReturnPressed:activate()
    Keys.onEnterPressed:activate()
    Keys.onSpacePressed:activate()
    Keys.onRightPressed:{var e=entries[currentIndex];if(e&&e.kind!=="thread"&&!e.expanded)toggle(e.key)}
    Keys.onLeftPressed:{var e=entries[currentIndex];if(e){if(e.kind!=="thread"&&e.expanded)toggle(e.key);else{var key=e.kind==="thread"?"project/"+e.projectKey+"/state/"+e.state:"project/"+e.projectKey;var i=entries.findIndex(function(r){return r.key===key});if(i>=0)currentIndex=i}}}
    delegate:DelegateChooser {
        role:"kind"
        DelegateChoice{roleValue:"thread";delegate:root.threadDelegate}
        DelegateChoice {
            delegate:Controls.AbstractButton {
                id:header
                required property var row
                required property int index
                objectName:"projectTreeHeader_"+row.key
                width:ListView.view.width;height:Style.space(row.kind==="project"?40:29)
                leftPadding:Style.space(row.kind==="project"?8:18);rightPadding:Style.space(8)
                Accessible.name:(row.kind==="project"?row.title:T3I18n.tr(row.title))+" · "+row.count
                onClicked:{root.currentIndex=index;root.forceActiveFocus();root.toggle(row.key)}
                background:Rectangle{radius:Style.space(6);color:header.hovered||root.activeFocus&&root.currentIndex===header.index?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06):"transparent";border.width:root.activeFocus&&root.currentIndex===header.index?1:0;border.color:Color.accent}
                contentItem:RowLayout {
                    spacing:Style.space(6)
                    T3Icon{name:header.row.expanded?"chevron-down":"chevron-right";implicitWidth:Style.space(12);tint:root.dim}
                    T3Icon{visible:header.row.kind==="project";name:"folder-git-2";implicitWidth:Style.space(14);tint:Color.accent}
                    Text{Layout.fillWidth:true;text:header.row.kind==="project"?header.row.title:T3I18n.tr(header.row.title);color:header.row.kind==="project"?root.fg:root.dim;font.pixelSize:header.row.kind==="project"?Style.font.body:Style.font.caption;font.weight:header.row.kind==="project"?Font.Medium:Font.Normal;elide:Text.ElideRight;textFormat:Text.PlainText}
                    Text{text:String(header.row.count);color:root.dim;font.family:"monospace";font.pixelSize:Style.font.caption}
                }
            }
        }
    }
    Text{anchors.centerIn:parent;width:parent.width-Style.space(20);visible:!root.count;text:T3I18n.tr("No threads here");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;horizontalAlignment:Text.AlignHCenter}
}
