.pragma library

// The quick menu of the bar icon (right click). The bar loads no translations, so
// the app publishes the labels; these helpers only shape rows and commands.

var ENGLISH = { animations: "Animations", full: "Full", light: "Light", off: "Off", server: "Server", restart: "Restart", stop: "Stop", app: "Application", quit: "Quit" }

// Rows for the menu. Animations and Quit act on the running app, so they are
// left out while it is stopped; Restart then starts it again.
function entries(labels, running) {
  var l = Object.assign({}, ENGLISH, labels || {})
  var rows = []
  if (running) rows.push({ header: l.animations }, { label: l.full, mode: "full" }, { label: l.light, mode: "light" }, { label: l.off, mode: "off" })
  rows.push({ header: l.server }, { label: l.restart, server: "restart" }, { label: l.stop, server: "stop" })
  rows.push({ header: l.app }, { label: l.restart, app: "restart" })
  if (running) rows.push({ label: l.quit, app: "quit" })
  return rows
}

// What choosing a row runs: {launcher: [args for bin/o3]} or {server: action}.
function command(row) {
  if (!row || row.header) return null
  if (row.mode) return { launcher: ["call", "drive", "animationMode", row.mode] }
  if (row.server === "restart" || row.server === "stop") return { server: row.server }
  if (row.app === "restart" || row.app === "quit") return { launcher: [row.app] }
  return null
}

function checked(row, running, mode) { return !!row && !!row.mode && running && mode === row.mode }
