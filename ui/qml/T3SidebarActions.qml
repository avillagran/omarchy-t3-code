import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import QtQuick.Dialogs
import qs.Commons
import qs.Ui

Item {
  id: root
  required property var bridge
  property var projects: []
  property var sourceThread:null
  property bool addingProject: false
  function newOnBranch(t){newThread();sourceThread=t;project.currentIndex=Math.max(0,projects.findIndex(function(p){return p.key===t.projectKey}))}
  function newThread() {
    sourceThread=null
    addingProject = bridge.managementRequest && bridge.managementRequest.action === "add-project"
    var index = projects.findIndex(function(p) { return p.key === root.bridge.selectedInfo.projectKey })
    project.currentIndex = Math.max(0,index)
    title.text = bridge.managementRequest ? (bridge.managementRequest.title || "") : T3I18n.tr("New chat")
    dialog.open()
  }
  function addProject() {
    if (bridge.managementRequest) { newThread(); return }
    folder.open()
  }
  Controls.Popup {
    id: dialog
    parent: Controls.Overlay.overlay
    anchors.centerIn: parent
    width: Style.space(460); padding: Style.space(20)
    modal: true
    background: Rectangle { color: Color.popups.background; radius: Style.space(12); border.width: 1; border.color: Color.accent }
    contentItem: ColumnLayout {
      spacing: Style.space(12)
      RowLayout {
        Text { Layout.fillWidth: true; text: root.addingProject ? T3I18n.tr("Add project") : T3I18n.tr("New chat"); color: Color.popups.text; font.pixelSize: Style.font.body; font.bold: true }
        T3IconButton { name: "x"; tooltipText: T3I18n.tr("Close"); onClicked: dialog.close() }
      }
      T3Select {
        id: project
        Layout.fillWidth: true; visible: !root.addingProject
        model: root.projects; textRole: "title"
        enabled: !root.bridge.managementBusy && !root.bridge.managementRequest
      }
      Controls.TextField {
        id: title
        Layout.fillWidth: true; visible: !root.addingProject
        color: Color.popups.text; placeholderText: T3I18n.tr("Thread title")
        enabled: !root.bridge.managementBusy && !root.bridge.managementRequest
        background: Rectangle { color: Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.06); radius: Style.space(7) }
      }
      Text { Layout.fillWidth: true; visible: !root.addingProject; text: T3I18n.tr("Opens in the project folder. Choose the model in the composer before sending."); color: Color.popups.text; opacity: 0.55; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
      Text { Layout.fillWidth: true; visible: !!root.bridge.managementError; text: T3I18n.error(root.bridge.managementError); color: Color.urgent; font.pixelSize: Style.font.caption; wrapMode: Text.Wrap }
      RowLayout {
        WorkIndicator { visible: root.bridge.managementBusy; active: visible && dialog.visible }
        Item { Layout.fillWidth: true }
        Button {
          text: root.bridge.managementRequest ? T3I18n.tr("Retry") : T3I18n.tr("Create thread")
          enabled: !root.bridge.managementBusy && (!!root.bridge.managementRequest || (!root.addingProject && project.currentIndex >= 0 && title.text.trim().length > 0))
          onClicked: {
            if (root.bridge.managementRequest) { root.bridge.manage({}); return }
            var p = root.projects[project.currentIndex]
            var template = root.bridge.state.threads.find(function(t) { return t.projectKey === p.key && t.modelSelection })
            var selection = p.defaultModelSelection || (template ? template.modelSelection : null) || root.bridge.draft.modelSelection
            if (!selection) { root.bridge.managementError = T3I18n.tr("Select a model in an existing chat first."); return }
            var branchSource=root.sourceThread&&root.sourceThread.projectKey===p.key?root.sourceThread:null
            root.bridge.manage({action: "new-thread", environment: p.environmentId, projectId: p.id, title: title.text.trim(), modelSelection: selection, runtimeMode: template ? template.runtimeMode : "approval-required",branch:branchSource?branchSource.branch:null,worktreePath:branchSource?branchSource.worktreePath:null})
          }
        }
      }
    }
  }
  FolderDialog {
    id: folder
    title: T3I18n.tr("Add folder as a T3 project")
    onAccepted: {
      root.addingProject = true
      root.bridge.manage({action:"add-project",environment:"local",url:String(selectedFolder)})
      dialog.open()
    }
  }
  Connections { target: root.bridge; function onManagementFinished(data) { if (!data.error) dialog.close() } }
}
