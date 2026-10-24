pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons

// The shared tooltip preferences of Omarchy's unified tooltips (omarchy PR
// #15694, edited from the Control Panel): ~/.config/omarchy/tooltips.json.
// Missing settings follow the theme; an invalid edit keeps the last valid one.
QtObject {
  id: root
  property var preferences: ({})
  readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config")
  readonly property string shape: preferences.shape || "theme"
  readonly property string borderMode: preferences.borderMode || "theme"
  readonly property string borderColor: preferences.borderColor || "#ffffff"

  function load(raw) {
    if (!String(raw || "").trim()) { preferences = ({}); return }
    try {
      var next = JSON.parse(raw)
      if (!next || typeof next !== "object" || Array.isArray(next)) return
      if (next.shape !== undefined && ["theme", "square", "rounded"].indexOf(next.shape) < 0) return
      if (next.borderMode !== undefined && ["theme", "transparent", "custom"].indexOf(next.borderMode) < 0) return
      if (next.borderColor !== undefined && !/^#[0-9a-fA-F]{6}$/.test(next.borderColor)) return
      preferences = next
    } catch (error) {}
  }
  function radius(themeRadius) { return shape === "square" ? 0 : shape === "rounded" ? Style.space(8) : themeRadius }
  function borderSpec(themeSpec) {
    if (borderMode === "theme") return themeSpec
    return { color: borderMode === "transparent" ? "transparent" : borderColor, widths: themeSpec.widths, gradient: { colors: [], angle: 0, enabled: false } }
  }

  property FileView settings: FileView {
    path: root.configHome + "/omarchy/tooltips.json"
    watchChanges: true; printErrors: false
    onLoaded: root.load(text())
    onFileChanged: reload()
    onLoadFailed: root.load("")
  }
}
