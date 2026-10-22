pragma Singleton
import QtQuick 6.11
import Quickshell
import Quickshell.Io
import qs.Commons

Singleton {
  id: root
  property var values: ({})
  readonly property color working: pick("blue", Color.accent)
  readonly property color done: pick("green", Color.accent)
  readonly property color waiting: pick("yellow", Color.accent)
  readonly property color failed: Color.urgent
  // T3's warning tone (Woke, Limited, Approval) and indigo for questions.
  readonly property color warning: pick("orange", pick("yellow", Color.accent))
  readonly property color input: pick("magenta", Color.accent)
  readonly property var projectColors: {
    var keys = ["blue", "green", "magenta", "cyan", "orange", "yellow", "red", "bright_blue", "bright_magenta"]
    var colors = []
    keys.forEach(function(k) { var c = String(root.pick(k, Color.accent)).toLowerCase(); if (colors.indexOf(c) < 0) colors.push(c) })
    return colors
  }
  function pick(name, fallback) { return values[name] || fallback }
  function load(raw) {
    var colors = {}; var lines = raw.split("\n")
    lines.forEach(function(line) { var m = line.match(/^\s*([a-zA-Z0-9_]+)\s*=\s*["'](#[0-9a-fA-F]{6})["']/); if (m) colors[m[1]] = m[2] })
    values = colors
  }
  // Row label colours for SidebarModel.detail(); the neutral "waiting" stays muted.
  function detailColor(detail) {
    return detail === "working" || detail === "goal" ? working : detail === "done" ? done : detail === "failed" ? failed
      : detail === "limited" || detail === "woke" || detail === "approval" ? warning : detail === "input" ? input
      : detail === "plan" ? waiting : Qt.rgba(Color.popups.text.r, Color.popups.text.g, Color.popups.text.b, 0.6)
  }
  function stateColor(state) { return state === "working" ? working : state === "done" ? done : state === "failed" ? failed : state === "waiting" ? waiting : Color.popups.text }
  function projectColor(key, projects) {
    var ids = (projects || []).map(function(p) { return p.key }).sort()
    var used = {}; var assignment = {}; var colors = projectColors
    ids.forEach(function(id) {
      var hash = 0; for (var i=0; i<id.length; i++) hash = ((hash * 31) + id.charCodeAt(i)) >>> 0
      var slot = hash % colors.length
      for (var j=0; j<colors.length && used[slot]; j++) slot = (slot+1) % colors.length
      assignment[id] = slot; used[slot] = true
    })
    return colors[assignment[key] === undefined ? 0 : assignment[key]]
  }
  FileView {
    id: themeFile
    path: Color.currentThemePath + "/colors.toml"
    watchChanges: true; printErrors: false
    onLoaded: root.load(text())
    onFileChanged: reload()
    onLoadFailed: root.load("")
  }
  // Theme switches replace the symlink; Color's IPC update also triggers a reload.
  Connections { target: Color; function onAccentChanged() { themeReload.restart() } function onBackgroundChanged() { themeReload.restart() } function onShellValuesChanged() { themeReload.restart() } }
  Timer { id: themeReload; interval: 100; onTriggered: themeFileReload() }
  function themeFileReload() { themeFile.reload() }
}
