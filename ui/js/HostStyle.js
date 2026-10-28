.pragma library
// How a host (environment) is marked on threads: its machine icon, an optional
// colour and whether its name shows. Saved per environment in the view state
// as hostStyles[id] = {icon, color, showName}; empty means T3's own icon and no colour.
var ICONS = { server: "server", cloud: "cloud", linux: "linux", desktop: "monitor", laptop: "laptop", "mac-mini": "mac-mini", "mac-studio": "mac-studio" }
var KINDS = ["desktop", "laptop", "server", "cloud", "linux", "mac-mini", "mac-studio"]
// Swatches that read on both dark and light Omarchy themes.
var COLORS = ["#e06c75", "#e5a050", "#d8c35a", "#8fc46a", "#4fb8a8", "#5aa2e6", "#9a7be6", "#d877b8"]
function of(styles, id, catalogMachine) {
  var saved = (styles || {})[id] || {}
  var kind = ICONS[saved.icon] ? saved.icon : (catalogMachine || (id === "local" ? "desktop" : "server"))
  return { kind: kind, icon: ICONS[kind] || "server", color: COLORS.indexOf(saved.color) >= 0 ? saved.color : "", showName: !!saved.showName }
}
function update(styles, id, patch) {
  var all = Object.assign({}, styles || {})
  all[id] = Object.assign({}, all[id] || {}, patch)
  return all
}
